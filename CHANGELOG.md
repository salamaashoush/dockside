# Changelog

All notable changes to Dockside will be documented in this file.

## [0.3.1] - 2026-05-03

### Packaging

- Switch the release pipeline to `cargo-packager`. Linux now ships
  `.deb` (Debian/Ubuntu), `.AppImage` (universal), and a pacman
  `PKGBUILD` + source tarball (Arch/CachyOS/Manjaro). macOS still
  ships `.app` (zipped) plus a `.dmg`.
- The privileged `dockside-helper` ships next to the main binary
  in every artifact. `.deb` / pacman drop a polkit policy at
  `/usr/share/polkit-1/actions/dev.dockside.helper.policy` so
  packaged users skip the bootstrap-copy prompt entirely.
- Multi-size hicolor app icons (16 / 32 / 48 / 64 / 128 / 256 /
  512 / 1024 + svg) so KDE and GNOME menus render the icon at
  every size.

### Linux DNS / port redirect

- Replace `setcap cap_net_bind_service` on the binary with an
  `nftables` `ip nat output` redirect on loopback :80 / :443 →
  :47080 / :47443. Persisted via a `dockside-port-redirect`
  oneshot systemd unit so the redirect comes back after reboot.
  setcap was wiped on every `cargo build` because file caps live
  on the inode, not the path.
- Proxy listener now degrades gracefully on `EACCES`: drops the
  previous runtime, falls back to the unprivileged port, and
  surfaces a "Restart Dockside to bind 80/443" banner in the
  Settings panel.

### Helper / app

- Bump helper version 0.3.0 → 0.3.1 so the in-app version check
  surfaces a clear "Re-run setup" hint when the on-disk helper
  is older than the running app.
- `helper_path()` is now polkit-policy-driven: parse the policy's
  `exec.path` annotation and call exactly that binary so pkexec
  hits the cached `auth_admin_keep` path.
- `bootstrap()` always pkexecs the in-tree helper instead of the
  stale system copy — otherwise the upgrade path was broken on
  every iterative build.
- `is_bootstrapped()` recognises packaged installs by checking
  every canonical helper location, not just `/usr/local/libexec`.
- `get_themes_dir()` learns the package layout
  (`/usr/lib/dockside/themes`, `/usr/share/dockside/themes`,
  `<exe>/../lib/dockside/themes`). Without this, `.deb` /
  pacman / AppImage installs silently fell back to the default
  theme.

### Tray

- Drop the system tray on Linux for now. gpui 0.2's Linux backend
  has no `Window::hide()` and stops the run loop on the last
  window close, so a tray-resident app needs a gpui fork. macOS
  keeps the tray with a stable SNI id `dev.dockside.app`.

## [0.3.0] - 2026-05-02

### Bug Fixes

- Improve container process discovery for minimal containers by @salamaashoush
- **machines**: Always expose Machines view; show Host on every platform by @salamaashoush
- **machines**: Host stats now populate; load free / df / ps locally by @salamaashoush
- **scan**: Structured install hint with copy buttons; jump to Vulns tab by @salamaashoush
- **scan**: Center the Vulnerabilities install/error panel by @salamaashoush
- **compose**: Chdir + -f config from container labels by @salamaashoush
- **compose**: Kill watch child on output dialog close by @salamaashoush

### Documentation

- README — drop Windows/Intel mac, point to scripts/install.sh + install-deps.sh, list real release assets by @salamaashoush

### Features

- Add cross-platform abstraction module by @salamaashoush
- Add platform-specific UI adaptations by @salamaashoush
- Add host management, Docker daemon config, and UI improvements by @salamaashoush
- Terminal grid + TUI fixes, k8s/colima as optional features, settings UX by @salamaashoush
- **terminal**: Mouse text selection with scroll-aware highlight by @salamaashoush
- **containers,images**: Live log streaming + image layer inspector by @salamaashoush
- **containers**: Per-container Stats tab + health/exit/mounts on Info by @salamaashoush
- **images**: Tag + push, streaming pull progress by @salamaashoush
- **create,prune**: Resource limits + labels, system df breakdown by @salamaashoush
- **terminal,containers**: Unified log viewer via libghostty TerminalSource by @salamaashoush
- **pods**: Unify k8s log viewer through libghostty TerminalSource by @salamaashoush
- **images**: Build dialog + streaming bollard build_image by @salamaashoush
- **logs,build,create**: Batch chunks, build output viewer, healthcheck by @salamaashoush
- **images**: Vulnerability scan via Trivy by @salamaashoush
- **images**: Registry browser via Docker Hub search by @salamaashoush
- **charts,search,scan**: Smooth sparklines, find bar, platform errors by @salamaashoush
- **build,scan**: Hadolint Dockerfile lint with platform-aware install hint by @salamaashoush
- **prune**: BuildKit build cache pruning by @salamaashoush
- **images**: Save/load image tarball by @salamaashoush
- **containers**: Add Author field to commit dialog by @salamaashoush
- **activity**: Per-row CPU sparkline + status breakdown badges by @salamaashoush
- **compose**: Docker compose watch with profile picker by @salamaashoush
- **build**: Debounced auto-lint on Dockerfile field edits by @salamaashoush
- **activity**: Hover tooltips on sparklines by @salamaashoush
- **build**: Browse button + native folder picker for context_dir by @salamaashoush
- **containers**: Hover tooltips on Stats tab sparklines by @salamaashoush
- **compose**: Inline YAML viewer by @salamaashoush
- **images**: Scan all toolbar button by @salamaashoush
- **volumes**: Used By section on detail tab by @salamaashoush
- **settings**: More options + auto-save text inputs + theme tracker by @salamaashoush
- **settings**: Wire kubeconfig path, default platform, terminal font, colima defaults by @salamaashoush

### Miscellaneous Tasks

- Add multi-platform builds for Linux and Windows by @salamaashoush
- Add Linux-specific GTK dependency by @salamaashoush
- Drop snapshot log path and trim unused fields by @salamaashoush
- Clippy clean by @salamaashoush
- Drop Windows targets — release ships macOS arm64 + Linux x86_64 only by @salamaashoush
- Install dependencies on every supported platform by @salamaashoush

### Refactor

- Update Docker client for cross-platform runtime support by @salamaashoush
- Update utilities and services for cross-platform support by @salamaashoush
- Wire cross-platform abstraction into UI and clean up dead code by @salamaashoush
- **terminal**: Replace alacritty_terminal with libghostty-vt by @salamaashoush

### Testing

- Hadolint parser, compose label extraction, build-cache prune by @salamaashoush

### Ui

- Standardize action UX across views by @salamaashoush
- Collapse detail toolbars to Ellipsis dropdown menu by @salamaashoush
- Collapse every list toolbar into a single Ellipsis dropdown by @salamaashoush
- **activity**: Pill-style status chips in title bar by @salamaashoush
- **images**: Inline Scan button on empty Vulnerabilities tab by @salamaashoush
- **settings**: Full revamp — sidebar nav + cards + auto-save by @salamaashoush
- **settings**: Drop second header + cards; flat rows + sidebar footer actions by @salamaashoush

## [0.2.0] - 2026-01-08

### Bug Fixes

- Prevent focus stealing from inputs across all views by @salamaashoush

### Features

- Add resource watchers for real-time updates by @salamaashoush
- Add command palette and global keybindings by @salamaashoush
- Add global search functionality by @salamaashoush
- Add loading and spinning icon components by @salamaashoush
- Expand Colima VM configuration support by @salamaashoush
- Enhance Kubernetes client and service handlers by @salamaashoush
- Add watcher integration to docker state by @salamaashoush
- Integrate watchers, command palette, and global search by @salamaashoush
- Improve list views with search and status filtering by @salamaashoush
- Add shared dialog utilities and module exports by @salamaashoush
- Improve machine UI with Config tab, command palette, and actions by @salamaashoush
- Add Kill icon to differentiate from Stop action by @salamaashoush
- Use Lucide icons for Container, Image, and Compose by @salamaashoush
- Add Colima settings section with cache management and template editor by @salamaashoush
- Add native macOS menu bar and About dialog by @salamaashoush
- Add file explorer context menu and open in editor support
- Add dynamic terminal resize and improved layout
- Add reusable ProcessView component with kill process support

### Miscellaneous Tasks

- Add empty changelog

### Refactor

- Unify machine create and edit dialogs by @salamaashoush
- Improve views with type-safe tabs and state preservation by @salamaashoush
- Standardize icons to use AppIcon for all resources by @salamaashoush
- Comprehensive icon consistency across entire UI by @salamaashoush

### Testing

- Add comprehensive test coverage (207 tests) by @salamaashoush

