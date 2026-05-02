# `*.dockside.test` Wildcard DNS + Reverse Proxy Design

Design doc for Dockside ticket #14. Captures research, decisions, scope, and a phased delivery plan. **No code is written yet.** This document exists to align before implementation, since the feature touches networking, OS integration, and the trust store on three platforms.

## Goal

Open `http://nginx.dockside.test` (or `https://`) in a browser and reach the running container named `nginx`. Same URL pattern works for any container, any user, on macOS, Linux, and Windows, with no manual `/etc/hosts` editing and no per-container port memorization.

## Why not `*.dockside.local`?

`.local` is **reserved by RFC 6762 for multicast DNS**. Bonjour-aware code on macOS (Safari, Finder, anything that calls `getaddrinfo` against the Apple resolver) MUST multicast `.local` queries to `224.0.0.251`, bypassing `/etc/resolver/`. OrbStack ships `*.orb.local` and tracks active user complaints about exactly this (issue #2214: OAuth redirects break, Safari intermittently fails, mDNS responders win the race). RFC 6761 specifically reserves `.test` for development; `.dev` is delegated by Google with HSTS preload (works fine for non-HTTPS-strict tools, but may force HTTPS in browsers). RFC 8375 reserves `home.arpa.` for home networks.

**Decision: ship `*.dockside.test` as the default**, allow users to change the suffix in Settings. Reject `.local` with a warning if entered.

## Why DNS alone is not enough

| Runtime | Bridge IP host-routable? | Approach |
|---|---|---|
| Docker on Linux (native) | Yes | DNS → container bridge IP works |
| Docker Desktop (macOS) | No (LinuxKit VM) | DNS → 127.0.0.1, reverse proxy → published port |
| Docker Desktop (Windows) | No (Hyper-V VM) | DNS → 127.0.0.1, reverse proxy → published port |
| Colima default | No (Lima VM) | Same as Docker Desktop |
| Colima `--network-address` | Yes | DNS → bridge IP |
| OrbStack with bridge enabled | Yes | DNS → bridge IP |
| Podman rootless | Mostly no | Same as Docker Desktop |

A pure DNS-to-bridge-IP path works only on Linux native and a couple of opt-in modes elsewhere. Splitting UX by platform is unacceptable for a desktop tool — users move between machines. So we need a reverse proxy that listens on `127.0.0.1:80` and `127.0.0.1:443` and forwards to whichever backend is actually reachable.

**Decision: ship DNS + reverse proxy together.** DNS always answers `127.0.0.1` for `*.dockside.test`; the proxy figures out the right backend per container.

## Architecture

```
┌─────────────┐        ┌────────────────────┐        ┌───────────────┐
│ Browser     │───DNS──│ Local resolver     │        │ Dockside app  │
│             │  query │ (OS DNS stub)      │───────▶│  hickory-dns  │
└─────────────┘        └────────────────────┘ 15353  │  server       │
       │                                              │  (UDP+TCP)    │
       │ HTTP/HTTPS to 127.0.0.1                      └───────────────┘
       ▼                                                       │
┌─────────────────────────────────────┐                        │ docker
│ Reverse proxy (axum + hyper)        │                        │ events
│ :80 / :443                          │                        ▼
│ • parse Host header                 │              ┌───────────────────┐
│ • lookup name in route map          │◀─────────────│ Container watcher │
│ • forward to backend                │   route map  │ (bollard stream)  │
└─────────────────────────────────────┘              └───────────────────┘
                  │
                  ▼ (one of)
        bridge IP : container port    (Linux / OrbStack / Colima w/ bridge)
        127.0.0.1 : host port         (Docker Desktop / Colima default)
```

### Components inside Dockside

1. **DNS server** — `hickory-server` 0.26.x, `default-features = false` to drop DoT/DoH/DoQ/DNSSEC. Listens on `127.0.0.1:15353` (UDP+TCP). Single `RequestHandler` shared across both sockets. State is `Arc<RwLock<RouteMap>>`; mutated by the container watcher.
2. **Container watcher** — already partly exists in `src/services/watchers/docker_events.rs`. Subscribe to `/events` filtered for `container=create,start,stop,die,destroy`. On every event, rebuild the route map from `docker ps` + per-container `inspect`. Same map fuels the proxy.
3. **Reverse proxy** — `axum` 0.7 + `hyper` 1.x. Listens on `127.0.0.1:443` (TLS) and `127.0.0.1:80` (HTTP, optional plain mode + redirect-to-HTTPS). Splits Host header → looks up in route map → proxies bidirectionally including WebSockets and streaming.
4. **Local CA** — `rcgen` to generate a Dockside root CA on first run, persisted under `~/.config/dockside/ca/`. Leaves minted in-memory per-domain on demand. Root CA installation step done by an OS-specific helper (see "Trust store" below).
5. **OS integration helper** — separate concern. Writes resolver config files / runs `resolvectl` / `Add-DnsClientNrptRule`. Always run with elevated privileges as a one-shot from a Settings UI button — never from app startup.

### Route map shape

```rust
struct Route {
  container_id: String,
  container_name: String,    // canonical lookup key (lowercased)
  aliases: Vec<String>,      // network aliases + label `dockside.alias=`
  backend: Backend,
  https_only: bool,          // label `dockside.https=true`
}

enum Backend {
  /// Linux native or OrbStack/Colima bridge mode.
  Bridge { ip: IpAddr, port: u16 },
  /// Docker Desktop / Colima default — published port on host loopback.
  HostPort { port: u16 },
  /// No reachable target; proxy returns 502 with a helpful page.
  Unreachable { reason: &'static str },
}
```

### Backend selection algorithm

Per container:

1. If a label `dockside.backend=host` is present, force `HostPort` mode. If `dockside.backend=bridge`, force `Bridge` mode.
2. Determine target port:
   - Label `dockside.port=<n>` wins.
   - Else, lowest numeric `Config.ExposedPorts` (Traefik default behavior).
   - Else, lowest port in `NetworkSettings.Ports` keys.
   - Else: `Unreachable { reason: "no exposed or published port" }`.
3. Determine reachability:
   - Detect runtime once per Dockside session: `docker info` — if `OperatingSystem` contains "Docker Desktop" or `KernelVersion` contains "linuxkit", we're VM-based. If `Architecture` matches host and `OperatingSystem` is normal, we're native Linux.
   - Native Linux → `Bridge { ip, port }` using the first non-empty `IPAddress` across `Networks`.
   - VM-based → `HostPort { port: published_port }` if published; otherwise `Unreachable`.
   - User can override per-container with `dockside.backend` label.

### Naming + collisions

- Lookup key: container name lowercased.
- If two containers have aliases that collide, the most recently created wins (last writer); UI surfaces a warning.
- Compose-style: `<service>.<project>.dockside.test` is added as an extra alias automatically when `com.docker.compose.project` and `com.docker.compose.service` labels are present.
- Wildcards never match `dockside.test` itself — the apex returns NXDOMAIN.

## OS resolver integration

A one-shot, opt-in install flow exposed via a Settings page button **"Enable `*.dockside.test` resolution"**. The button shells out a privileged helper that performs the platform-specific step. Uninstall flow removes whatever was added.

### macOS

```
sudo install -m 644 /dev/stdin /etc/resolver/dockside.test <<EOF
nameserver 127.0.0.1
port 15353
search_order 1
EOF
```

`scutil --dns` should show `dockside.test` as a resolver. No reboot needed.

Caveats: per-domain resolver works in BSD-socket and `dig`. Browsers + curl are fine. Apps using NetService/Bonjour skip it — but that only matters for `.local`, which we are explicitly avoiding.

### Linux — systemd-resolved (preferred)

```
resolvectl dns lo 127.0.0.1:15353
resolvectl domain lo '~dockside.test'
```

`~` makes it a routing-only domain. Persist via a `~/.config/systemd/network/dockside.network` drop-in, or write a `/etc/systemd/network/10-dockside.network` file with helper privileges. Verify with `resolvectl status lo`.

### Linux — NetworkManager + dnsmasq fallback

```
echo 'server=/dockside.test/127.0.0.1#15353' \
  | sudo tee /etc/NetworkManager/dnsmasq.d/dockside.conf
sudo nmcli general reload
```

Requires `dns=dnsmasq` in `NetworkManager.conf`. We detect resolved first, fall back to NM/dnsmasq if found, otherwise show clear instructions.

### Windows — NRPT

```powershell
Add-DnsClientNrptRule -Namespace ".dockside.test" -NameServers "127.0.0.1"
```

Limitation: NRPT cannot specify a port. The DNS server must listen on UDP/TCP 53. Two options:

A. On Windows only, bind the DNS server to `127.0.0.1:53` directly. This requires the Dockside helper to ensure the Windows DNS Client service does not already hold port 53 — typically it doesn't. Service install runs with admin elevation.
B. Run a tiny port-redirector (Windows-only) that forwards `127.0.0.1:53` → `127.0.0.1:15353`, e.g. via `netsh interface portproxy` (supports v4tov4 UDP since Server 2019).

Option A is simpler and standard for tools like Acrylic DNS Proxy. Pick A.

## TLS and the local trust store

### Certificate generation (`rcgen`)

- First app launch: generate Ed25519 root CA, save private key + cert under `~/.config/dockside/ca/` (mode 0600). Root CA validity: 10 years.
- Reverse proxy holds the CA; mints leaf certs on demand keyed by SNI Host header. Leaves are RSA-2048 or P-256 depending on browser compat preference, validity 90 days, regenerated on expiry. Never written to disk.
- Cert cache keyed by canonical hostname; LRU max 256 entries; new entries on demand.

### Installing the root CA

OS-specific, requires elevation, exposed via a Settings button **"Install Dockside HTTPS root certificate"**:

- macOS: `security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain ~/.config/dockside/ca/root.crt`
- Linux: copy to `/usr/local/share/ca-certificates/dockside.crt` + `update-ca-certificates`. Firefox/Chromium use the NSS store separately (`certutil -A -n ...`); Settings page shows the manual command if NSS detected.
- Windows: `certutil -addstore -f ROOT %APPDATA%\Dockside\ca\root.crt`. Edge/Chrome use the system store; Firefox uses its own.
- Uninstall flow reverses each.

A clear inline disclaimer in the Settings UI explains: this CA is on disk, anyone with the private key can MITM your traffic, do not enable on shared/managed machines. Disable by default. Same trust pattern as mkcert / OrbStack 1.1.

### HTTP/2 support

`hyper` 1.x supports HTTP/2 server-side. Decision: support HTTP/1.1 and HTTP/2 both upstream and downstream. WebSocket upgrade goes through the HTTP/1.1 path. HTTP/3 (QUIC) deferred — not worth the dependency footprint for v1.

## Container watcher integration

Reuse `src/services/watchers/docker_events.rs`. Add a `DnsRouteWatcher` task that:

1. On startup, calls `docker.list_containers(true)` and seeds the route map.
2. Subscribes to `/events` filtered by `type=container` actions: `create`, `start`, `stop`, `die`, `destroy`, `rename`, `update`.
3. On each event, calls `docker.inspect_container(id)` for the affected container only. Updates the map. Emits a `RouteMapChanged` event so any UI listing routes (Settings page, Dashboard tile) refreshes.
4. Holds an `Arc<RwLock<RouteMap>>` shared with both DNS handler and proxy. Reads are read-locked; writes are short.

Note: there is currently a known gap that `set_namespace` does not refresh `events` (per CLAUDE-side rule #5). This is a separate concern. The DNS watcher uses container events, not k8s events; not blocked.

## Settings UI

New Settings panel section "Dockside Local DNS":

- **Enable `*.dockside.test` resolution** toggle. When flipped on the first time, runs the OS install helper with elevation prompt; then starts the DNS server. Writes `dns_enabled: bool` and `dns_suffix: String` to settings.
- **Suffix** text input, default `dockside.test`, with validation: rejects `.local`, `.localhost`, anything containing slashes or whitespace, and warns if HSTS-preloaded TLDs (`.dev`, `.app`) are entered.
- **DNS port** numeric, default 15353, advanced setting hidden behind a disclosure.
- **Install Dockside HTTPS root certificate** button. Greyed when CA already trusted.
- **Reverse proxy** toggle. On = bind 80/443 (or 8080/8443 if 80/443 unavailable). Shows status indicator + last error.
- **Per-container labels reference** card with copyable examples for `dockside.port`, `dockside.alias`, `dockside.backend`, `dockside.https`.

## Phased delivery

Each phase is independent. Each leaves the app working without the feature if the user disables it. Each is one focused commit on top of the existing branch.

### Phase A — Local DNS server, opt-in (1 session)
- Add `hickory-server` 0.26 with default-features off.
- New module `src/services/dns/server.rs` and `src/services/dns/route_map.rs`.
- Container watcher task that seeds + mutates `RouteMap`. No proxy yet.
- Settings UI toggle + suffix input. OS-install helper for macOS only (simplest).
- Validate by `dig @127.0.0.1 -p 15353 nginx.dockside.test` returning `127.0.0.1`.

### Phase B — Reverse proxy on `:80`, plain HTTP (1 session)
- `src/services/proxy/server.rs` with `axum`/`hyper`.
- WebSocket upgrade.
- Backend selection algorithm.
- 502 page when target unreachable, 404 when name unknown.
- Validate by `curl http://nginx.dockside.test/` reaching nginx.

### Phase C — TLS + local CA (1 session)
- `rcgen` root CA, leaf cert minting.
- HTTPS listener on `:443`.
- Settings UI for CA install/uninstall (macOS first).
- Validate by `curl --cacert ~/.config/dockside/ca/root.crt https://nginx.dockside.test/` working, then by browser after CA install.

### Phase D — Linux + Windows OS integration (1 session)
- systemd-resolved + NM/dnsmasq detection and helper scripts.
- Windows port-53 binding mode + NRPT install.
- Linux + Windows trust store helpers.

### Phase E — UX polish (interleaved)
- Dashboard tile listing all live `<name>.dockside.test → backend` mappings, click to open.
- Compose-aware aliases.
- Per-container labels reference and override editor in container detail.
- Health probe column in the route table.

## Risks

| Risk | Mitigation |
|---|---|
| Port 80 / 443 already in use (existing local web server) | Detect on bind; offer 8080/8443 fallback; show clear status |
| User installs root CA, machine compromised, key reused for MITM | Documented warning; private key 0600; opt-in; uninstall flow |
| systemd-resolved not running on the user's distro | Detect → fall back to NM/dnsmasq → fall back to manual instructions |
| macOS `/etc/resolver/` requires sudo every time the file is rewritten | Only write once; survive across DNS suffix changes by adding/removing files instead of editing in place |
| User changes suffix mid-session | Restart DNS server, rewrite OS files, leave proxy alone (uses Host header) |
| Container has no published port AND we're on Docker Desktop | `Unreachable { reason }`; proxy 502 with explanatory page |
| Compose service rebuild causes brief routing dead time | Watcher reacts to `start` events; downtime is sub-second |
| Antivirus / EDR flags the local CA | Document in README; some users will need to whitelist |
| Mobile devices on same network want to use it too | Out of scope for v1; bind to `127.0.0.1` only |

## Out of scope (v1)

- Remote machines on the LAN reaching `*.dockside.test` (would require LAN DNS, not loopback)
- IPv6 backends (`::1` works for the proxy itself but container IPv6 routing on Docker Desktop is broken)
- HTTP/3 / QUIC
- Plugin system for custom routing rules
- Sharing the route map with `kubectl port-forward` style flows for k8s services (could come later as `*.svc.dockside.test`)
- A CLI mode (`dockside dns ...`) — possible but separate

## Open questions

1. **Suffix default**: `.dockside.test` (RFC-blessed, safest) vs `.dockside` (shorter, but unreserved and could theoretically be delegated by ICANN someday) vs `.dockside.dev` (HSTS preload). Recommend `.dockside.test`.
2. **HTTPS-only mode**: should plain HTTP redirect to HTTPS by default, or serve both? Recommend "redirect to HTTPS unless container has `dockside.http_only=true` label", since HTTPS is preferred and free.
3. **Privilege model for the helper**: spawn a separate small binary that does only the installs (auditable), or shell out via `osascript -e 'do shell script ... with administrator privileges'` / `pkexec` / RunAs? Recommend a separate `dockside-helper` binary with a narrow command surface — easier to review, easier to sign on macOS.
4. **Auto-start of DNS server**: on every Dockside launch, or only when user opts in? Recommend opt-in plus `dns_autostart: bool` setting (default true once enabled).
5. **Migration from any existing `/etc/hosts` workflow**: do we want a tool that reads the user's existing `/etc/hosts` entries for `*.dockside.local` and offers to migrate? Probably yes for adoption.

## Recommendation

Ship phases A → B → C as the v1 milestone over three commits. Phase D after that brings Linux + Windows to feature parity. Phase E is interleaved. Total time estimate: 4-5 focused sessions for full cross-platform coverage; v1 (macOS-only, plain HTTP) ships in 2 sessions.

Sources cited inline below; full bibliography on request.
- RFC 6761 (Special-Use Domain Names — `.test`, `.localhost`)
- RFC 6762 (Multicast DNS — `.local` reservation)
- RFC 8375 (`home.arpa` — homenet)
- hickory-dns 0.26 release notes (May 2026)
- OrbStack docs: container domains, container networking, OrbStack 1.1 HTTPS blog post
- OrbStack issue #2214 (non-`.local` TLD support)
- Docker Desktop networking how-tos (DNS limits in VM)
- Colima `--network-address` (issue #220)
- Apple `man 5 resolver` and `scutil --dns`
- systemd-resolved `resolvectl` and `~domain` syntax
- Microsoft `Add-DnsClientNrptRule` PowerShell reference
- mkcert (FiloSottile) — local CA install pattern
- Traefik docker-provider routing rules
