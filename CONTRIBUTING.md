# Contributing

Use Windows, Node.js 22.12+ and Rust stable with the MSVC toolchain. Follow README.md for system prerequisites.

Before opening a pull request:

```powershell
npm ci
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml
python scripts/check_repository.py
```

Use synthetic fixtures and local mock HTTP services in tests. Do not require a real NAS, private SSH alias or cloud account. The registry integration test is ignored by default and runs separately on a disposable Windows runner.

For compatibility reports, include NAS OS/tool versions, filesystem type and redacted output. Keep real addresses, account names, volume names, cookies and tokens out of logs, screenshots and fixtures. Documentation addresses are appropriate examples; synthetic private and loopback addresses are appropriate when testing network classification.

Preserve third-party licenses and notices. Keep dependency changes separate from unrelated fixes and update lockfiles deliberately. Do not commit installers, executables, runtime data or development captures.

Describe each PR's behavior change and verification. Credential/network changes should have regression tests. Label untested platform compatibility explicitly. Security reports follow SECURITY.md.
