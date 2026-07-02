#!/usr/bin/env python3
"""Decrypt (and re-encrypt) MSI Center ``!!MSI!!`` data files.

MSI Center ships several data manifests obfuscated with a ``!!MSI!!`` magic
prefix followed by base64 (e.g. ``Data/PackageDataV2.dat``, ``GameDataV4.dat``,
``CreatorDataV2.dat``, ``AI Definition.dat``). They are AES-256-CBC encrypted.

The scheme is recovered from ``CS_CommonAPI.dll`` → ``C_Encrypt.DecryptBase64``:

    key = SHA256(UTF8(CryptoKey))          # 32 bytes -> AES-256
    iv  = MD5   (UTF8(CryptoKey))          # 16 bytes
    plaintext = AES-CBC-decrypt(base64(body_after_"!!MSI!!"))   # PKCS7

    CryptoKey = (<recovered-from-CS_CommonAPI.dll>).ToString("X")  # C#  ==  "<recovered>"

Verified against MSI Center 2.0.71.0. See ../docs/msi-center-architecture.md.

Usage:
    ./decrypt-msi-dat.py PackageDataV2.dat            # -> stdout (decrypted)
    ./decrypt-msi-dat.py PackageDataV2.dat -o out.json
    ./decrypt-msi-dat.py --encrypt out.json -o RoundTrip.dat
"""
import argparse
import base64
import hashlib
import sys

MAGIC = b"!!MSI!!"
# C#: (<recovered-from-CS_CommonAPI.dll>).ToString("X") -> uppercase hex, no leading zeros
CRYPTO_KEY = format(<recovered-from-CS_CommonAPI.dll>, "X")  # "<recovered>"


def _key_iv(crypto_key: str):
    k = crypto_key.encode()
    return hashlib.sha256(k).digest(), hashlib.md5(k).digest()


def _aes_cbc(key, iv):
    # Prefer pycryptodome, fall back to the `cryptography` package.
    try:
        from Crypto.Cipher import AES
        return ("pycryptodome", AES.new(key, AES.MODE_CBC, iv))
    except ImportError:
        from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
        return ("cryptography", Cipher(algorithms.AES(key), modes.CBC(iv)))


def decrypt(raw: bytes, crypto_key: str = CRYPTO_KEY) -> bytes:
    if raw[: len(MAGIC)] != MAGIC:
        # Not encrypted -> return as-is (matches MSI's GetFileContent behaviour).
        return raw
    body = base64.b64decode(raw[len(MAGIC):])
    key, iv = _key_iv(crypto_key)
    backend, ciph = _aes_cbc(key, iv)
    if backend == "pycryptodome":
        pt = ciph.decrypt(body)
    else:
        d = ciph.decryptor()
        pt = d.update(body) + d.finalize()
    return pt[: -pt[-1]]  # strip PKCS7 padding


def encrypt(plaintext: bytes, crypto_key: str = CRYPTO_KEY) -> bytes:
    key, iv = _key_iv(crypto_key)
    pad = 16 - (len(plaintext) % 16)
    plaintext = plaintext + bytes([pad]) * pad
    backend, ciph = _aes_cbc(key, iv)
    if backend == "pycryptodome":
        ct = ciph.encrypt(plaintext)
    else:
        e = ciph.encryptor()
        ct = e.update(plaintext) + e.finalize()
    return MAGIC + base64.b64encode(ct)


def main() -> int:
    ap = argparse.ArgumentParser(description="Decrypt/encrypt MSI Center !!MSI!! data files")
    ap.add_argument("infile")
    ap.add_argument("-o", "--outfile", help="output path (default: stdout)")
    ap.add_argument("--encrypt", action="store_true", help="encrypt instead of decrypt")
    ap.add_argument("--key", default=CRYPTO_KEY, help=f"CryptoKey (default: {CRYPTO_KEY})")
    args = ap.parse_args()

    with open(args.infile, "rb") as f:
        raw = f.read()
    out = encrypt(raw, args.key) if args.encrypt else decrypt(raw, args.key)

    if args.outfile:
        with open(args.outfile, "wb") as f:
            f.write(out)
    else:
        sys.stdout.buffer.write(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
