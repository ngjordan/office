#!/usr/bin/env python3
"""Mint an own-CA (private trust) code-signing certificate for Windows.

This is the "run your own CA" route: a self-signed root that signs a code-signing
leaf, which is then stored as the release environment's WINDOWS_CERTIFICATE
secret and used by sign.ps1 exactly like a certificate from a public CA.

What this buys you: no vendor, no identity validation, no per-seat fee, and the
root can be pushed to the Trusted Root store on the machines you control.
What it does not buy you: public trust. Windows treats a certificate whose root
is not in a root program as untrusted, so SmartScreen still warns, and a public
download still shows "Unknown publisher". Use Azure Artifact Signing (Public
Trust) or a real CA if the file is going to strangers.

Only the leaf is uploaded to CI. The root stays offline (see --root-offline),
because anyone holding the root key can mint a certificate for any publisher
name, which is the same reason public CAs keep their roots offline.

    # once: create the root, kept offline
    python3 make_own_ca.py root --out-dir ~/iridium-ca

    # per release: leaf signed by the root, then stored in the environment
    python3 make_own_ca.py leaf --ca-dir ~/iridium-ca --out iridium-office.pfx

    python3 set_signing_secrets.py --pfx iridium-office.pfx --password ...

Requires openssl on PATH. Everything is stdlib + the openssl binary.
"""

from __future__ import annotations

import argparse
import base64
import shutil
import subprocess
import sys
from pathlib import Path

OPENSSL = shutil.which("openssl")
if OPENSSL is None:
    raise SystemExit("error: openssl is not on PATH")


def die(msg: str) -> None:
    print(f"error: {msg}", file=sys.stderr)
    raise SystemExit(1)


def run(args: list[str], **kw) -> str:
    p = subprocess.run([OPENSSL, *args], capture_output=True, text=True, **kw)
    if p.returncode != 0:
        die(f"openssl {' '.join(args[:2])} failed:\n{p.stderr}")
    return p.stdout.strip()


def run_info(args: list[str]) -> str:
    """Like run(), but tolerates a non-zero exit and returns stdout+stderr.

    `openssl pkcs12 -info` prints the algorithm names it found and then fails
    when it cannot decrypt the bag, because OpenSSL 3 does not load the legacy
    provider by default. The header is what we need to check.
    """
    p = subprocess.run([OPENSSL, *args], capture_output=True, text=True)
    return (p.stdout or "") + (p.stderr or "")


# The root's extensions. openssl req has no -extfile, so these go in as -addext.
ROOT_ADDEXT = [
    "basicConstraints=critical,CA:TRUE,pathlen:0",
    "keyUsage=critical,keyCertSign,cRLSign,digitalSignature",
    "subjectKeyIdentifier=hash",
]

# The leaf's extensions, in an extfile for `openssl x509 -req -extfile`.
# Code-signing EKU (1.3.6.1.5.5.7.3.3) plus the Microsoft individual-code-signing
# OID, because Windows SmartScreen checks for the latter on modern binaries.
LEAF_EXT = """
[v3_leaf]
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=codeSigning,1.3.6.1.4.1.311.2.1.21
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid,issuer
"""


def make_root(out_dir: Path, days: int, subject: str) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)
    key = out_dir / "ca.key"
    crt = out_dir / "ca.crt"

    # 4096-bit RSA: the root lives for years and is the thing you cannot rotate.
    run(["genrsa", "-out", str(key), "4096"])
    key.chmod(0o600)
    args = [
        "req", "-new", "-x509", "-key", str(key), "-out", str(crt),
        "-days", str(days), "-subj", subject, "-sha256",
    ]
    for e in ROOT_ADDEXT:
        args += ["-addext", e]
    run(args)
    print(f"root certificate : {crt}")
    print(f"root key         : {key}  (mode 600, keep it offline)")
    # certutil wants DER on Windows; the PEM copy is what `openssl verify` uses.
    der = out_dir / "ca.der"
    run(["x509", "-in", str(crt), "-outform", "DER", "-out", str(der)])
    print(f"root (DER)       : {der}")
    print()
    print("Install the root on the machines that must trust it:")
    print(f"  certutil -addstore -f Root {der}")


def make_leaf(ca_dir: Path, out_pfx: Path, days: int, subject: str, password: str | None) -> None:
    # The leaf's material goes next to the .pfx, not into the CA directory: the
    # CA directory is the thing you keep offline, and it should hold only the
    # root key and root certificate.
    work = out_pfx.parent
    work.mkdir(parents=True, exist_ok=True)
    key = work / "leaf.key"
    csr = work / "leaf.csr"
    crt = work / "leaf.crt"
    ext = work / "leaf.ext"

    run(["genrsa", "-out", str(key), "2048"])
    key.chmod(0o600)
    run(["req", "-new", "-key", str(key), "-out", str(csr), "-subj", subject, "-sha256"])
    ext.write_text(LEAF_EXT)
    run([
        "x509", "-req", "-in", str(csr), "-CA", str(ca_dir / "ca.crt"),
        "-CAkey", str(ca_dir / "ca.key"), "-CAcreateserial", "-out", str(crt),
        "-days", str(days), "-sha256", "-extfile", str(ext), "-extensions", "v3_leaf",
    ])
    # The .pfx is what set_signing_secrets.py uploads. It must be the legacy
    # PKCS#12 format (3DES/RC2/SHA-1): Windows signtool.exe cannot read the
    # PBES2/AES-256 form that OpenSSL 3 produces by default, and it fails the
    # import with a confusing error. -legacy selects the old algorithms.
    pfx_args = ["pkcs12", "-export", "-out", str(out_pfx), "-inkey", str(key),
                "-in", str(crt), "-certfile", str(ca_dir / "ca.crt"), "-name", "Iridium Office"]
    if password:
        pfx_args += ["-passout", f"pass:{password}"]
    else:
        pfx_args += ["-passout", "pass:"]
    pfx_args += ["-legacy"]
    run(pfx_args)

    # Self-check: the file must be the legacy format that Windows can import.
    # The header is readable without the legacy crypto provider, but the bag
    # contents are not (RC2-40 is not available by default in OpenSSL 3), so
    # check the algorithm names rather than decrypting.
    info = run_info(["pkcs12", "-in", str(out_pfx), "-passin", f"pass:{password or ''}",
                      "-info", "-noout", "-nomacver"])
    if "AES-256" in info or "PBES2" in info:
        die("the .pfx is in the PBES2/AES format, which Windows signtool cannot import")
    if "RC2" not in info and "3DES" not in info and "DES-EDE3" not in info:
        die(f"the .pfx is not in a Windows-readable PKCS#12 form: {info.strip()[:200]}")
    # The chain must verify against our own root.
    run(["verify", "-CAfile", str(ca_dir / "ca.crt"), str(crt)])

    print(f"leaf certificate : {crt}")
    print(f"leaf .pfx        : {out_pfx}")
    print(f"leaf key         : {key}  (not needed for CI; the .pfx carries the key)")
    print()
    print("Next:")
    print(f"  python3 set_signing_secrets.py --pfx {out_pfx} --password '<the password>'")
    print("The root must also be installed on any machine that verifies the signature:")
    print(f"  certutil -addstore -f Root {ca_dir / 'ca.der'}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("root", help="create the self-signed CA root (keep it offline)")
    r.add_argument("--out-dir", required=True, type=Path)
    r.add_argument("--days", type=int, default=3650)
    r.add_argument("--subject", default="/O=Iridium Technologies, Inc./CN=Iridium Office Root CA")

    l = sub.add_parser("leaf", help="issue a code-signing leaf signed by the root")
    l.add_argument("--ca-dir", required=True, type=Path)
    l.add_argument("--out", required=True, type=Path)
    l.add_argument("--days", type=int, default=397)
    l.add_argument("--subject", default="/O=Iridium Technologies, Inc./CN=Iridium Technologies, Inc.")
    l.add_argument("--password", default=None)

    args = ap.parse_args()
    if args.cmd == "root":
        make_root(args.out_dir, args.days, args.subject)
    else:
        make_leaf(args.ca_dir, args.out, args.days, args.subject, args.password)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
