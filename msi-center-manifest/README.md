# MSI Center manifest census

Tooling to reverse-engineer the **Windows MSI Center** feature manifest, to understand how MSI
decides which features a given laptop supports. This informed the `msi-wmi-platform` driver design
(see `../docs/msi-center-architecture.md`).

**No proprietary MSI data is shipped here** — this repo carries only the *tooling* and *method*.
You regenerate the manifests/census locally from **your own installed MSI Center**, and the
decryption key is **recovered from your own `CS_CommonAPI.dll`**, never embedded.

## TL;DR — how MSI Center gates features
- **Presence** (webcam, panel-OD, backlight, HSR panel) → probed at **runtime** via WMI
  `Get_Device(0x01)` capability bitmap. No static table.
- **Control** (fan / profile / charge / boost) → offered **generically to every notebook**;
  whether it works is decided by the **EC firmware**.
- The real per-model discriminator is the **EC register layout**, accessed via uniform selectors
  (`Set_Data(idx,val)` = raw EC write). No per-model branch in the app; **no device-specific DLL**
  is downloaded — CDN packages are feature-name+version only.

## Contents
| File | What |
|---|---|
| `decrypt-msi-dat.py` | Decrypt/encrypt MSI Center `!!MSI!!` files. Recovers the key from your `CS_CommonAPI.dll` (`--dll`) or takes `--key`. |
| `build-manifest-db.py` | Build the queryable SQLite census from a decrypted `PackageDataV2.json`. |

Generated artifacts (`data/*.json`, `*.sqlite`) are git-ignored — produce them locally (below).

## The `!!MSI!!` encryption (from `CS_CommonAPI.dll` → `C_Encrypt.DecryptBase64`)
```
strip "!!MSI!!" prefix -> base64-decode -> AES-256-CBC (PKCS7)
key = SHA256(UTF8(CryptoKey)) ; iv = MD5(UTF8(CryptoKey))
CryptoKey = (<int>).ToString("X")     # the <int> is a constant in CS_CommonAPI.dll
```
`decrypt-msi-dat.py` recovers `<int>` by decompiling *your* `CS_CommonAPI.dll` with `ilspycmd`
(`dotnet tool install -g ilspycmd`) — so a local MSI Center install is required (interop posture).

## Regenerate the census locally
MSI Center's data lives in `…/MSI Center/Data/` (or inside the extracted MSIX `DCv2/Package/MSI
Center SDK.exe` → `app/Data/`). Point at your install:
```sh
DLL="/mnt/win/Program Files (x86)/MSI/MSI Center/CS_CommonAPI.dll"
DAT="/mnt/win/Program Files (x86)/MSI/MSI Center/Data/PackageDataV2.dat"
python3 decrypt-msi-dat.py "$DAT" --dll "$DLL" -o data/PackageDataV2.json
python3 build-manifest-db.py data/PackageDataV2.json -o msi-nb-manifest.sqlite
```
The census schema (tables `component`, `gate`, `model`, `model_component`) and example queries are
described in `../docs/msi-center-architecture.md`. Requires `pycryptodome` **or** `cryptography`.
