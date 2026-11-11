#!/usr/bin/env python3
"""Set the notary secrets on ngjordan/office, in the `release` environment.

The notary service takes either an Apple ID with an app-specific password, or an
App Store Connect API key. The API key is better: it does not expire and needs
no interactive 2FA, so a CI job can notarize unattended.

This reads the key from the local key store, seals it with the ENVIRONMENT's
public key (libsodium SealedBox, the format GitHub accepts) and sets three
secrets:

    APPLE_API_KEY        the base64 of the .p8 private key, no newlines
    APPLE_API_KEY_ID     the key id
    APPLE_API_ISSUER_ID  the issuer id

An environment secret only reaches a job that declares `environment: release`,
and the environment is restricted to the default branch, so a fork's pull_request
cannot get it. The environment has its own public key, which is not the
repository's: sealing with the repository's key is rejected.

Nothing is printed except the names and the HTTP status. The key never leaves
this machine in plaintext.

    python3 set_notary_secrets.py                 # ngjordan/office, release
    python3 set_notary_secrets.py --check         # only list what is set
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import re
import sys
import urllib.request
from pathlib import Path

KEY_DIR = Path(os.environ.get("APPLE_KEY_DIR", Path.home() / ".hermes/keys/apple"))
SECRET_NAMES = ["APPLE_API_KEY", "APPLE_API_KEY_ID", "APPLE_API_ISSUER_ID"]


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


def read_key() -> dict[str, str]:
    if not KEY_DIR.is_dir():
        die(f"{KEY_DIR} does not exist")
    key_id = (KEY_DIR / "key_id").read_text().strip()
    issuer = (KEY_DIR / "issuer_id").read_text().strip()
    pem = KEY_DIR / f"AuthKey_{key_id}.p8"
    if not pem.exists():
        die(f"{pem} does not exist")
    return {
        # No newlines: the value goes straight into a base64 decode in the workflow.
        "APPLE_API_KEY": base64.b64encode(pem.read_bytes()).decode(),
        "APPLE_API_KEY_ID": key_id,
        "APPLE_API_ISSUER_ID": issuer,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default="ngjordan/office")
    ap.add_argument("--env", default="release")
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

    values = read_key()
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
    print(f"\n  all three set: {not missing}" + (f"  missing={missing}" if missing else ""))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
