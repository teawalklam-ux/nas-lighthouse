# Security and privacy

## Scope and permissions

NAS Lighthouse is a local desktop client. The SSH collector reads system metrics and Docker metadata without installing an agent or writing business files. The optional cloud credential feature is different: it **writes to your AList / OpenList storage configuration**.

Use a dedicated SSH account where possible. Docker socket access is a high-privilege capability, not a read-only permission. The search/storage integration obtains an administrator token from the configured container; enable it only for a backend you trust. This is not a hardened multi-user gateway.

## Data and credentials

- SSH keys stay with OpenSSH; the app uses non-interactive authentication and normal host-key verification.
- Backend administrator tokens are held in memory, scoped to endpoint, SSH alias, container and backend. Changes invalidate the cache. They are not written to `config.json`.
- Cloud Cookie/refresh token input is sent to your backend, which persists it. The client does not persist this input. Closing the dialog clears its React state; guaranteed memory zeroization is not claimed.
- Credential updates fetch the existing storage by ID, verify driver/path and replace only the credential field. No storage is created, enabled or deleted. Avoid simultaneous storage edits from another client: this operation is not a server-side transaction.
- HTTP endpoints transmit credentials in plaintext. Prefer HTTPS with a valid trusted certificate or a trusted SSH tunnel. Authenticated API requests do not follow redirects.
- Configuration contains hosts, aliases, service URLs and preferences. User-supplied URLs can themselves contain secrets. Full configuration export is a private backup, **not a sanitized diagnostic report**.
- Events can contain service names and backend status/error messages. Inspect and redact before sharing. Do not upload runtime configuration, events, browser Cookie exports, private screenshots or crash dumps.

## Network requests

- The client connects to configured SSH/HTTP endpoints, checks service TCP ports and opens selected links in your browser.
- Icons are requested only from the service origin. Redirects and third-party fallback are disabled; TLS verification stays enabled. These requests bypass system HTTP proxies.
- NAS outbound checks use configurable TCP 443 targets, initially `1.1.1.1` and `8.8.8.8`. They can be disabled. A failed probe does not prove all internet destinations are unreachable.
- Help links open third-party websites when clicked. This project does not operate those sites or receive their OAuth callbacks.

## Independent installations

The app does not automatically read older settings or remove older startup entries. Backup import is explicit and does not enable startup. Use `NAS_LIGHTHOUSE_CONFIG_DIR` to isolate development/demonstrations from real configuration.

## Vulnerability reporting

Use this repository's **Security → Report a vulnerability** feature for a private report. Do not publish credentials or exploit details in issues. If private reporting is unavailable, ask for a private contact in an issue without including sensitive details.

Include affected commit/version, platform, impact and reproduction with synthetic data. This early project has no guaranteed response SLA; only the latest development version is actively maintained.
