"""Check the Git index for accidental local artifacts and common secret formats.

This is a release guard, not a complete secret scanner. It only prints paths,
line numbers and rule names, never matching credential values.
"""
from pathlib import Path, PurePosixPath
import json
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
RULES = {
    "private key": re.compile(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----"),
    "GitHub token": re.compile(r"\b(?:gh[pousr]_[A-Za-z0-9_]{30,}|github_pat_[A-Za-z0-9_]{40,})\b"),
    "AWS access key": re.compile(r"\bAKIA[A-Z0-9]{16}\b"),
    "JWT": re.compile(r"\beyJ[A-Za-z0-9_-]{15,}\.[A-Za-z0-9_-]{15,}\.[A-Za-z0-9_-]{15,}"),
    "secret constant": re.compile(r'(?i)\b(?:\w*(?:CLIENT_SECRET|API_KEY|ACCESS_TOKEN))\s*(?::[^=\n]+)?=\s*["\'][A-Za-z0-9_./+\-=]{24,}["\']'),
}
FORBIDDEN_PARTS = {"node_modules", "target", "dist", "release", "credentials", "secrets", ".nas-lighthouse"}
FORBIDDEN_NAMES = {"handoff.md", "overlay-verify.png", "events.json", "id_rsa", "id_ed25519"}
FORBIDDEN_SUFFIXES = {".exe", ".dll", ".pdb", ".msi", ".msix", ".bak", ".zip", ".7z", ".pem", ".key", ".p12", ".pfx", ".db"}


def main():
    result = subprocess.run(["git", "ls-files", "--stage", "-z"], cwd=ROOT, check=True, capture_output=True)
    entries = [entry for entry in result.stdout.split(b"\0") if entry]
    if not entries:
        raise SystemExit("No staged/tracked files; stage the intended source tree first.")
    errors = []
    paths = set()
    for entry in entries:
        metadata, raw_path = entry.split(b"\t", 1)
        mode, object_id, stage = metadata.decode().split()
        path = raw_path.decode("utf-8")
        paths.add(path)
        parts = PurePosixPath(path)
        if mode not in {"100644", "100755"} or stage != "0":
            errors.append(f"{path}: unexpected index mode/stage")
            continue
        if (set(parts.parts) & FORBIDDEN_PARTS or parts.name in FORBIDDEN_NAMES
                or parts.suffix.lower() in FORBIDDEN_SUFFIXES
                or any(part.startswith(".tmp-") for part in parts.parts)
                or (parts.name.startswith(".env") and parts.name != ".env.example")
                or (parts.name.endswith("-config.json") and parts.parts[0] != "examples")):
            errors.append(f"{path}: local artifact must not be committed")
        data = subprocess.check_output(["git", "cat-file", "blob", object_id], cwd=ROOT)
        if len(data) > 2 * 1024 * 1024:
            errors.append(f"{path}: unexpected large source artifact")
        if b"\0" in data[:8192]:
            continue
        try:
            text = data.decode("utf-8-sig")
        except UnicodeDecodeError:
            continue
        for number, line in enumerate(text.splitlines(), 1):
            for label, pattern in RULES.items():
                if pattern.search(line):
                    errors.append(f"{path}:{number}: {label}")
    for required in ["LICENSE", "README.md", "SECURITY.md", "CONTRIBUTING.md", "THIRD-PARTY-NOTICES.md", "src/vendor/bloub/LICENSE", "src/vendor/bloub/NOTICE.txt"]:
        if required not in paths:
            errors.append(f"{required}: required notice/document missing from index")
    package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
    npm_lock = json.loads((ROOT / "package-lock.json").read_text(encoding="utf-8"))
    manifest = tomllib.loads((ROOT / "src-tauri/Cargo.toml").read_text(encoding="utf-8"))
    cargo_lock = tomllib.loads((ROOT / "src-tauri/Cargo.lock").read_text(encoding="utf-8"))
    if package["name"] != "nas-lighthouse" or npm_lock["packages"][""]["name"] != package["name"]:
        errors.append("npm package/lock name mismatch")
    if manifest["package"]["name"] not in {p["name"] for p in cargo_lock["package"] if "source" not in p}:
        errors.append("Cargo package/lock name mismatch")
    if errors:
        print("\n".join(errors))
        return 1
    print(f"Repository guard passed: {len(entries)} indexed files; no blocked artifact or common secret pattern.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
