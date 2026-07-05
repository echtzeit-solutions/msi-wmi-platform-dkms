#!/usr/bin/env python3
"""Decrypt (and re-encrypt) MSI Center ``!!MSI!!`` data files.

MSI Center ships several data manifests obfuscated with a ``!!MSI!!`` magic
prefix followed by base64 (e.g. ``Data/PackageDataV2.dat``, ``GameDataV4.dat``,
``CreatorDataV2.dat``, ``AI Definition.dat``). They are AES-256-CBC encrypted.

The scheme (from ``CS_CommonAPI.dll`` -> ``C_Encrypt.DecryptBase64``):

    key = SHA256(UTF8(CryptoKey))          # 32 bytes -> AES-256
    iv  = MD5   (UTF8(CryptoKey))          # 16 bytes
    plaintext = AES-CBC-decrypt(base64(body_after_"!!MSI!!"))   # PKCS7

    CryptoKey = (<int>).ToString("X")      # a constant in CS_CommonAPI.dll

This tool deliberately does **not** embed MSI's key. For interoperability it
**recovers the CryptoKey from your own installed MSI Center** (`CS_CommonAPI.dll`)
via `find_crypto_key()`, or you can pass it with ``--key``. You therefore need a
local MSI Center install (or its extracted appx) to use this.

Usage:
    ./decrypt-msi-dat.py PackageDataV2.dat                 # auto-find key, -> stdout
    ./decrypt-msi-dat.py PackageDataV2.dat --dll /path/CS_CommonAPI.dll -o out.json
    ./decrypt-msi-dat.py PackageDataV2.dat --key XXXXXXX -o out.json
    ./decrypt-msi-dat.py --encrypt out.json --key XXXXXXX -o RoundTrip.dat
"""
import argparse
import base64
import glob
import hashlib
import os
import re
import shutil
import subprocess
import sys

MAGIC = b"!!MSI!!"

# Where MSI Center's CS_CommonAPI.dll typically lives (Windows mount or extracted appx).
_DLL_GLOBS = [
    "/mnt/win*/Program Files*/MSI/MSI Center/CS_CommonAPI.dll",
    "/mnt/win*/Program Files*/WindowsApps/*MSICenter*/DCv2/CS_CommonAPI.dll",
    os.path.expanduser("~/.local/share/msi-center*/CS_CommonAPI.dll"),
    "./CS_CommonAPI.dll",
]


def find_crypto_key(dll_path: str | None = None) -> str:
    """Recover the CryptoKey from an MSI Center CS_CommonAPI.dll.

    C_Encrypt uses ``CryptoKey = (<int>).ToString("X")``. We decompile the DLL
    with ilspycmd (a .NET tool) and read that integer, then format it as
    uppercase hex -- exactly what the app does at runtime.
    """
    if not dll_path:
        for pat in _DLL_GLOBS:
            hits = glob.glob(pat)
            if hits:
                dll_path = hits[0]
                break
    if not dll_path or not os.path.isfile(dll_path):
        raise SystemExit(
            "CS_CommonAPI.dll not found. Install/point to MSI Center and pass "
            "--dll <path>, or supply --key explicitly."
        )
    if not shutil.which("ilspycmd"):
        raise SystemExit(
            "ilspycmd not found (dotnet tool install -g ilspycmd). It is needed to "
            "recover the key from CS_CommonAPI.dll; alternatively pass --key."
        )
    res = subprocess.run(["ilspycmd", dll_path], capture_output=True, text=True)
    if res.returncode != 0:
        raise SystemExit(f"ilspycmd failed on {dll_path}:\n{res.stderr.strip()}")
    m = re.search(r'CryptoKey\s*=\s*(\d+)\s*\.\s*ToString\(\s*"X"\s*\)', res.stdout)
    if not m:
        raise SystemExit("Could not locate the CryptoKey constant in " + dll_path)
    return format(int(m.group(1)), "X")


def _key_iv(crypto_key: str):
    k = crypto_key.encode()
    return hashlib.sha256(k).digest(), hashlib.md5(k).digest()


def _aes_cbc(key, iv):
    try:
        from Crypto.Cipher import AES
        return ("pycryptodome", AES.new(key, AES.MODE_CBC, iv))
    except ImportError:
        from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
        return ("cryptography", Cipher(algorithms.AES(key), modes.CBC(iv)))


def decrypt(raw: bytes, crypto_key: str) -> bytes:
    if raw[: len(MAGIC)] != MAGIC:
        return raw  # not encrypted (matches MSI's GetFileContent behaviour)
    body = base64.b64decode(raw[len(MAGIC):])
    key, iv = _key_iv(crypto_key)
    backend, ciph = _aes_cbc(key, iv)
    pt = ciph.decrypt(body) if backend == "pycryptodome" else \
        (lambda d: d.update(body) + d.finalize())(ciph.decryptor())
    # strip PKCS7, validating it: garbage padding almost always means a wrong key
    pad = pt[-1] if pt else 0
    if not 1 <= pad <= 16 or pt[-pad:] != bytes([pad]) * pad:
        raise SystemExit("invalid PKCS7 padding after decrypt -- wrong CryptoKey?")
    return pt[:-pad]


def encrypt(plaintext: bytes, crypto_key: str) -> bytes:
    key, iv = _key_iv(crypto_key)
    pad = 16 - (len(plaintext) % 16)
    plaintext += bytes([pad]) * pad
    backend, ciph = _aes_cbc(key, iv)
    ct = ciph.encrypt(plaintext) if backend == "pycryptodome" else \
        (lambda e: e.update(plaintext) + e.finalize())(ciph.encryptor())
    return MAGIC + base64.b64encode(ct)


def main() -> int:
    ap = argparse.ArgumentParser(description="Decrypt/encrypt MSI Center !!MSI!! data files")
    ap.add_argument("infile")
    ap.add_argument("-o", "--outfile", help="output path (default: stdout)")
    ap.add_argument("--encrypt", action="store_true", help="encrypt instead of decrypt")
    ap.add_argument("--key", help="CryptoKey (else auto-recovered from CS_CommonAPI.dll)")
    ap.add_argument("--dll", help="path to CS_CommonAPI.dll for key recovery")
    args = ap.parse_args()

    key = args.key or find_crypto_key(args.dll)
    with open(args.infile, "rb") as f:
        raw = f.read()
    out = encrypt(raw, key) if args.encrypt else decrypt(raw, key)

    if args.outfile:
        with open(args.outfile, "wb") as f:
            f.write(out)
    else:
        sys.stdout.buffer.write(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
