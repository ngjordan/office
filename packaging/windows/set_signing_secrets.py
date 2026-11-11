#!/usr/bin/env python3
"""Set the Windows code-signing secrets in the repository's `release` environment.

A CA-issued code-signing certificate arrives as a PKCS#12 file. GitHub stores
it as a secret: the base64 of the .pfx, and the password that opens it.

    WINDOWS_CERTIFICATE           the base64 of the .pfx, no newlines
    WINDOWS_CERTIFICATE_PASSWORD  the .pfx password

The signing script (packaging/windows/sign.ps1) prefers these when they are
present and falls back to the Azure Artifact Signing path, so this does not
conflict with that.

The values are sealed with the ENVIRONMENT's public key (libsodium SealedBox,
the format GitHub accepts) - the environment has its own key, and a value
sealed with the repository's key is rejected.

    python3 set_signing_secrets.py --pfx ~/iridium-office.pfx --password ...
    python3 set_signing_secrets.py --check

Nothing is printed except the names and the HTTP status. The certificate never
leaves the machine in plaintext.
"""

from __future__ import annotations

import argparse
import base64
import getpass
import json
import os
import re
import sys
import urllib.request
from pathlib import Path

SECRET_NAMES = ["WINDOWS_CERTIFICATE", "WINDOWS_CERTIFICATE_PASSWORD"]


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def token() -> str:
    creds = Path.home() / ".git-credentials"
    if not creds.exists():
        die("no ~/.git-credentials")
    m = re.search(r"https://[^:]+:([^@]+)@github\.com", creds.read_text())
    if not m:
        die("no GitHub token in ~/.git-credentials")
    return m.group(1)


def gh(url: str, tok: str, method: str = "GET", body: bytes | None = None) -> tuple[int, bytes]:
    req = urllib.request.Request(
        url, data=body, method=method,
        headers={"Authorization": f"Bearer {tok}", "User-Agent": "hermes",
                 "Accept": "application/vnd.github+json", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def seal(public_key_b64: str, plaintext: str) -> str:
    """SealedBox-encrypt, the format GitHub requires.

    crypto_box_beforenm/aforenm produces a different container and GitHub
    rejects it, so this uses SealedBox explicitly.
    """
    from nacl.public import PublicKey, SealedBox

    key = PublicKey(base64.b64decode(public_key_b64))
    return base64.b64encode(SealedBox(key).encrypt(plaintext.encode())).decode()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default="ngjordan/office")
    ap.add_argument("--env", default="release")
    ap.add_argument("--pfx", help="the PKCS#12 file to store")
    ap.add_argument("--password", help="the .pfx password (prompted if omitted)")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    tok = token()
    base = f"https://api.github.com/repos/{args.repo}/environments/{args.env}"
    status, body = gh(f"{base}/secrets/public-key", tok)
    if status != 200:
        die(f"could not fetch the environment public key: HTTP {status} {body[:200]!r}")
    pk = json.loads(body)
    key_id, public_key = pk["key_id"], pk["key"]

    if args.check:
        st, resp = gh(f"{base}/secrets", tok)
        names = {s["name"] for s in json.loads(resp).get("secrets", [])} if st == 200 else set()
        for n in SECRET_NAMES:
            print(f"  {'set' if n in names else 'MISSING'}  {n}")
        return 0

    if not args.pfx:
        die("--pfx is required")
    pfx = Path(args.pfx).expanduser()
    if not pfx.is_file():
        die(f"{pfx} is not a file")
    password = args.password or getpass.getpass("  the .pfx password: ")

    values = {
        # No newlines: the value goes straight into a base64 decode in the workflow.
        "WINDOWS_CERTIFICATE": base64.b64encode(pfx.read_bytes()).decode(),
        "WINDOWS_CERTIFICATE_PASSWORD": password,
    }
    for name in SECRET_NAMES:
        payload = {"encrypted_value": seal(public_key, values[name]), "key_id": key_id}
        st, resp = gh(f"{base}/secrets/{name}", tok, method="PUT",
                      body=json.dumps(payload).encode())
        print(f"  {'OK ' if st in (201, 204) else 'FAIL'} {name}  (HTTP {st})")
        if st not in (201, 204):
            print(resp[:300].decode(errors="replace"))

    st, resp = gh(f"{base}/secrets", tok)
    names = {s["name"] for s in json.loads(resp).get("secrets", [])} if st == 200 else set()
    missing = [n for n in SECRET_NAMES if n not in names]
    print(f"\n  both set: {not missing}" + (f"  missing={missing}" if missing else ""))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
