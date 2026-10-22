# Third-party notices

The root MIT license covers original NAS Lighthouse code. Dependencies and vendored assets retain their own terms. This overview is not a relicensing of those components or a complete license inventory for every possible binary build.

## Vendored bloub engine

`src/vendor/bloub/` comes from [jeremy-prt/bloub](https://github.com/jeremy-prt/bloub).

Copyright (c) 2026 Jérémy Perret. MIT License. The full original [LICENSE](src/vendor/bloub/LICENSE) and [NOTICE](src/vendor/bloub/NOTICE.txt) are retained. The SVG engine serves as the application status mascot.

## Frontend dependencies

| Component | License / source |
| --- | --- |
| React / React DOM | MIT; [facebook/react](https://github.com/facebook/react) |
| GSAP 3.15.0 | GreenSock Standard “no charge” license; [upstream](https://github.com/greensock/GSAP), [terms](https://gsap.com/standard-license/) |
| @gsap/react 2.1.2 | GSAP Standard license; [greensock/react](https://github.com/greensock/react) |
| Lucide React | ISC; [lucide-icons/lucide](https://github.com/lucide-icons/lucide) |
| Tauri API / opener | MIT or Apache-2.0; [tauri-apps](https://github.com/tauri-apps) |
| Build tooling | See installed package licenses; package-lock.json records exact versions |

GSAP and its React integration are **not MIT dependencies**. Retain upstream copyright/license headers when distributing bundled JavaScript and check the terms for your use. Their source is not vendored into this repository.

## Rust and platform components

Cargo.toml declares direct dependencies and Cargo.lock pins resolved versions. Tauri, its plugins, Tokio, Serde, reqwest/rustls, Windows bindings and transitive dependencies retain their own licenses. Before distributing an executable, generate an inventory for that locked build and include required license texts. This repository does not ship a prebuilt installer.

Microsoft WebView2, Windows runtime components, OpenSSH, Everything and AList/OpenList are external prerequisites, not bundled copies. Names identify interoperability, not vendor affiliation.
