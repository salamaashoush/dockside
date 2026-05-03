# Changelog

All notable changes to Dockside will be documented in this file.

## [0.4.0] - 2026-05-03

### Bug Fixes

- **clippy**: Satisfy newer pedantic lints (rust 1.95) by @salamaashoush
- **k8s**: Make resource list bodies scrollable by @salamaashoush
- **ui**: Unify detail-view tab bar — drop redundant padding and borders by @salamaashoush
- **ui**: No-op re-select + inline menu buttons by @salamaashoush
- **tables**: Keep column widths consistent across rows by @salamaashoush
- **tables**: Responsive — min total width + horizontal scroll by @salamaashoush
- **tables**: Sticky header + responsive scrolling by @salamaashoush
- **settings**: DNS panel layout — sectioned, status card, route list, test guide by @salamaashoush
- **tls**: Install rustls ring provider at startup before any TLS code by @salamaashoush
- **proxy**: Default ports 8080/8443 (unprivileged) instead of 80/443 by @salamaashoush
- **helper**: Arch/Fedora trust store via update-ca-trust; dashboard DNS tile in System by @salamaashoush
- **helper**: Use absolute paths under pkexec sanitised PATH; log invocations by @salamaashoush
- **proxy**: Default ports 47080/47443 (avoid common 8080/8443 collisions) by @salamaashoush
- **proxy**: Only auto-redirect HTTP→HTTPS when local CA is trusted by @salamaashoush
- **helper**: Chmod 0644 on CA file before update-ca-trust extract by @salamaashoush
- **helper**: Refuse stale system helper; surface clear refresh hint by @salamaashoush

### Documentation

- K8s roadmap — multi-context, cluster CRUD, node detail, add-node flows by @salamaashoush
- Design for *.dockside.test wildcard DNS + reverse proxy by @salamaashoush
- **dns**: Lock decisions — .test TLD, bundled proxy, helper binary by @salamaashoush
- **changelog**: Add v0.3.1 entry by @salamaashoush

### Features

- **containers**: Clickable port chips open in browser by @salamaashoush
- **networks**: Connect/disconnect container at runtime by @salamaashoush
- **containers**: Multi-select with bulk start/stop/restart/delete by @salamaashoush
- **machines**: Krunkit VM type for GPU-accelerated AI workloads by @salamaashoush
- **models**: AI Models view wrapping `colima model` (macOS aarch64) by @salamaashoush
- **k8s**: Secrets + ConfigMaps views + native k3s/kubeadm detection by @salamaashoush
- **k8s**: StatefulSets + DaemonSets views with rolling restart by @salamaashoush
- **k8s**: Jobs + CronJobs views, sidebar grouping, default ns 'all' by @salamaashoush
- **k8s**: Ingress + PVC views, Networking/Storage sidebar groups by @salamaashoush
- **k8s**: YAML edit + apply + rollback + reload toolbar by @salamaashoush
- **k8s**: Cluster overview (Nodes/Events/Namespaces) + YAML actions dropdown by @salamaashoush
- **dashboard**: Top-level dashboard with counts, system, favorites, activity by @salamaashoush
- **k8s**: StatefulSet list+detail split with Info/Pods/YAML tabs by @salamaashoush
- **dashboard**: Unified card layout — fixed-size tiles, single grid by @salamaashoush
- **k8s**: DaemonSet list+detail split with Info/Pods/YAML tabs by @salamaashoush
- **k8s**: Job list+detail split with Info/Pods/YAML tabs by @salamaashoush
- **k8s**: CronJob list+detail split with Info/Recent Jobs/YAML tabs by @salamaashoush
- **k8s**: Storage parent group view, consistent with other groups by @salamaashoush
- **k8s**: Ingress list+detail split with Info/YAML tabs by @salamaashoush
- **k8s**: PVC list+detail split with Info/YAML tabs by @salamaashoush
- **k8s**: Secret + ConfigMap list+detail split with Info/Data/YAML/Events by @salamaashoush
- **k8s**: Node cordon/uncordon/drain + Secret/ConfigMap create forms by @salamaashoush
- **volumes**: Backup, restore, and clone via throw-away alpine container by @salamaashoush
- **containers**: Docker cp upload/download via row menu prompts by @salamaashoush
- **dns**: Foundation — bridge_ip on ContainerInfo, AppSettings DNS fields by @salamaashoush
- **dns**: *.dockside.test resolver + reverse proxy + local CA + helper + Settings UI by @salamaashoush
- **dns**: Port inputs in Settings, container Domain row, Dashboard route card; fix CA install before HTTPS spin-up by @salamaashoush
- **dns**: One-click bootstrap — copies helper to /usr/local/libexec, writes polkit rule, installs resolver+CA in one auth prompt; smarter resolver detection (systemd-resolved vs NM dnsmasq) by @salamaashoush
- **tls**: Also install CA into per-user NSS dbs (Chromium / Firefox) by @salamaashoush
- **dns**: "Drop port from URL" — setcap on Linux, pf redirect on macOS, switches settings to 80/443 by @salamaashoush
- **packaging**: Cargo-packager pipeline + nftables redirect + Linux helper UX by @salamaashoush

### Miscellaneous Tasks

- **logging**: Silence noisy third-party trace events by default by @salamaashoush

### Styling

- Cargo fmt + fix typo (unparseable→unparsable) by @salamaashoush

### Ui

- Consolidate namespace dropdown + bulk-actions dropdown by @salamaashoush
- Remove redundant detail-view action dropdowns by @salamaashoush
- Namespace dropdown — outlined trigger, right-aligned by @salamaashoush
- Tint group-view header with tab_bar bg by @salamaashoush
- **dashboard**: Drop redundant Local DNS routes card; tile in System covers it by @salamaashoush
- **settings**: Drop How-to-test panel from DNS section by @salamaashoush
- **settings**: Show CA install status chip; hide Remove unless installed by @salamaashoush
- Container row Open-in-browser action; Settings DNS panel auto-refresh on container events by @salamaashoush

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

