# MSI Center manifest census

Reverse-engineered data from the **Windows MSI Center** app (v2.0.71.0), extracted to understand
how MSI decides which features a given laptop supports. This directly informed the
`msi-wmi-platform` driver design (see `../docs/msi-center-architecture.md`).

## TL;DR — how MSI Center gates features
- **Presence** (webcam, panel-OD, backlight, HSR panel) → probed at **runtime** via WMI
  `Get_Device(0x01)` capability bitmap. No static table.
- **Control** (fan / profile / charge / boost) → offered **generically to every notebook**
  (`Support=null` in the manifest); whether it actually works is decided by the **EC firmware**.
- The real per-model discriminator is the **EC register layout**, accessed via uniform selectors
  (`Set_Data(idx,val)` = raw EC write). No per-model branch in the app; **no device-specific DLL is
  downloaded** — CDN packages are feature-name+version only.

## Contents
| File | What |
|---|---|
| `decrypt-msi-dat.py` | Decrypt/encrypt MSI Center `!!MSI!!` files (AES-256-CBC; key derived below) |
| `build-manifest-db.py` | Build the queryable SQLite census from `PackageDataV2.json` |
| `data/PackageDataV2.json` | Decrypted NB feature-package catalog (21 components + gating rules) |
| `data/GameDataV4.json`, `CreatorDataV2.json`, `AI_Definition.json` | Decrypted per-app profiles |
| `msi-nb-manifest.sqlite` | Census: 1,919 models × 21 components × 21,452 support edges |

## The `!!MSI!!` encryption (from `CS_CommonAPI.dll` → `C_Encrypt.DecryptBase64`)
```
strip "!!MSI!!" prefix -> base64-decode -> AES-256-CBC (PKCS7)
key = SHA256(UTF8(CryptoKey)) ; iv = MD5(UTF8(CryptoKey))
CryptoKey = (<recovered-from-CS_CommonAPI.dll>).ToString("X")  ==  "<recovered>"
```

## Querying the census (SQLite)
Tables: `component`, `gate` (allow/deny/allow_nbfamily/devicetype/deviceid), `model`,
`model_component` (resolved edges; `via` ∈ universal / platform-nb / allow).
```sh
# features offered to a given machine (by BIOS marketing name):
sqlite3 msi-nb-manifest.sqlite \
  "SELECT component,via FROM model_component WHERE model='Stealth GS66 12UHS' ORDER BY via;"

# which models a feature targets:
sqlite3 msi-nb-manifest.sqlite \
  "SELECT value FROM gate WHERE component='Mystic Light' AND kind='allow';"
```
(No `sqlite3` CLI? Python's `sqlite3` module reads it directly — see `build-manifest-db.py`.)

## Refreshing from a newer MSI Center release
1. Download the installer (`https://download.msi.com/uti_exe/desktop/MSI-Center.zip`) — it wraps an
   Inno Setup exe → an MSIX AppxBundle → `DCv2/Package/MSI Center SDK.exe` (another Inno installer)
   whose `app/Data/` holds `PackageDataV2.dat` etc. Extract with `innoextract` + `unzip`.
2. `python3 decrypt-msi-dat.py PackageDataV2.dat -o data/PackageDataV2.json`
3. `python3 build-manifest-db.py data/PackageDataV2.json -o msi-nb-manifest.sqlite`

Requires `pycryptodome` **or** `cryptography` for the AES step.
