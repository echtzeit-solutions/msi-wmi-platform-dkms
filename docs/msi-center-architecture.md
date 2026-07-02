# How MSI Center (Windows) decides & drives features

Reverse-engineered from MSI Center **2.0.71.0** (decompiled managed DLLs with `ilspycmd`;
decrypted data manifests). This is the reference that shaped the `msi-wmi-platform` driver's
capability-based design. Companion data + tooling: `../msi-center-manifest/`.

## The three layers
```
 (1) Feature plugins  — API_NB_*.dll (User Scenario, General Settings, Gaming Mode, …)
        thin UI/orchestration; hold NO register knowledge.
        build opaque command frames, e.g. CMD_SetFanAdvanced = {0,21},
        CMD_FanCoolerBoostON = {2,0,0,9,1,7,0,1}
        │  DataCenter.Transfer_ToAPI("Kernel", frame)
        ▼
 (2) Native engine    — API_Kernel.dll / *_Engine.dll  (UNMANAGED C++)
        translates frames → WMI ACPI method calls with a selector byte.
        This is the only layer that knows frame→register; not decompilable (Ghidra only).
        │  InvokeWmiMethod(method, selector, data)
        ▼
 (3) WMI ACPI iface   — MSIWMIACPI2 (managed wrapper) → ACPI \_SB.PC00.LPCB.EC.WMAM
        Set_Data(idx,val) = raw EC write to register idx ; Get_Device(0x01) = presence bitmap
        ▼
      EC firmware      — interprets the selector per model. THIS is where models differ.
```

## How features are gated (two tiers)
**Presence features — runtime-probed** (no static table): `Get_Device(0x01)` returns a capability
bitmap from EC/BIOS, re-read on each access. Decode (see `capability-map.md`): `Data[1]` bit1=WebCam,
bit4=PanelOD; `Data[2]` bit3=Backlight, bit6=HSR.

**Control features — offered generically** (fan / profile / charge / boost): in the package
manifest these are `Support=null` (universal to all notebooks). MSI does **not** keep a per-model
table for them; whether they work is decided by the EC firmware. There is **no per-model/family
branch** in MSI's setters — e.g. `SetShiftModeValueInEC` is `Set_Data(0xD2, val)` with `val` built
purely by bitmask math; the only branch is `WmiMajorVersion >= 2` vs. a legacy named-pipe fallback.

→ **Driver consequence:** probe presence at runtime like MSI; offer control generically; keep a
small **per-family allow-list** (by EC-ID) only as a *safety* gate for control features (default-off
on unrecognized boards), plus register-convention fields *if* a future generation diverges.

## Confirmed WMI selectors (`Set_Data idx` = raw EC register)
`0xD2` shift/profile · `0xD4` fan mode · `0x98` cooler boost · `0xDB` USB LED · `0xE8` Fn/Win ·
`0xD1/0xD3` status · `0x2C/0x2E/0x2F` mic-LED/webcam/resume. (Full map + encodings:
`ec-register-map.md`.) Charge-threshold (`0xD7`) and fan-curve tables (`0x6a/0x72` CPU,
`0x82/0x8b` GPU) are set through the native engine, not the managed layer; our values are
hardware-validated from msi-ec + EC-diff RE.

## The CDN & the manifest (verdict: no device-specific binary)
- MSI Center downloads **feature packages** from `download*.msi.com`; each `DefinePackage[].Dependent[]`
  file is `"<FeatureName>_<Version>.exe"` — keyed by **feature name + version only**, never
  model/board/EC-ID. Eligible machines of any family get the **same** binary.
- The manifest (`PackageDataV2.dat`) is AES-256-CBC encrypted behind a `!!MSI!!` prefix:
  `key = SHA256(CryptoKey)`, `iv = MD5(CryptoKey)`, `CryptoKey = (<recovered-from-CS_CommonAPI.dll>).ToString("X") = "<recovered>"`
  (from `CS_CommonAPI.C_Encrypt.DecryptBase64`). Decrypt/query tooling: `../msi-center-manifest/`.
- Census: **1,919 models × 21 NB components × 21,452 support edges**. Gating vocabulary:
  `Platform` (digit-flags; `1`=NB, `7`=NB+AI-Engine), `Allow`/`Deny`/`Allow_NBFamily`
  (marketing-name or `*<board>_<family>`), `DeviceType`/`DeviceID` (connected-USB gating).
  Our `Stealth GS66 12UHS` resolves to 6 universal + 4 platform-NB + Sound Tune (allow).

## Bottom line for the driver
MSI's own architecture validates: **runtime capability probing for presence + generic control over
uniform selectors + a thin per-family safety/convention table**, with no downloaded manifest or
per-device blob. This is exactly the `msi-wmi-platform` refactor (see the approved plan).
