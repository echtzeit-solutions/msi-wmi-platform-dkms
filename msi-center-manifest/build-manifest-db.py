#!/usr/bin/env python3
"""Build a queryable SQLite census from MSI Center's decrypted PackageDataV2.

PackageDataV2 is MSI Center's notebook feature-package catalog. Each package
(component) carries a ``Support`` gate that decides which machines are offered
that feature:

  - Support == null            -> universal (every notebook)
  - Platform "1"/"7"           -> platform-type flags ("1"=NB, "7"=NB+AI-Engine)
  - Allow / Allow_NBFamily     -> marketing-name or "*<board>_<family>" whitelist
  - Deny                       -> exclusions
  - DeviceType / DeviceID      -> connected-peripheral (USB) gating

This mirrors MSI Center's ``ParseIsSupport`` (CS_CommonAPI.dll). The resolved
``model_component`` table applies the marketing-name / universal / platform-NB
rules (it does NOT resolve DeviceID gating, which needs a live USB scan).

Usage:  ./build-manifest-db.py PackageDataV2.json -o msi-nb-manifest.sqlite
"""
import argparse
import json
import os
import sqlite3

SCHEMA = """
CREATE TABLE component(name TEXT PRIMARY KEY, typeid INT, platform TEXT, deny_nb INT,
  end_os TEXT, combo_set INT, universal INT, allow_n INT, deny_n INT, device_gated INT);
CREATE TABLE gate(component TEXT, kind TEXT, value TEXT);   -- allow/deny/allow_nbfamily/devicetype/deviceid
CREATE TABLE model(name TEXT PRIMARY KEY, is_family INT);
CREATE TABLE model_component(model TEXT, component TEXT, via TEXT);  -- via: universal/platform-nb/allow
"""


def build(pkg_json: str, db_path: str) -> None:
    d = json.load(open(pkg_json, encoding="utf-8"))
    pkgs = d["DefinePackage"]
    if os.path.exists(db_path):
        os.remove(db_path)
    c = sqlite3.connect(db_path)
    x = c.cursor()
    x.executescript(SCHEMA)

    models = set()
    for p in pkgs:
        comp, s = p["Component"], p.get("Support")
        if s is None:
            x.execute("INSERT INTO component VALUES(?,?,?,?,?,?,?,?,?,?)",
                      (comp, p.get("TypeID"), None, 0, None, None, 1, 0, 0, 0))
            continue
        allow = s.get("Allow") or []
        deny = s.get("Deny") or []
        nbfam = s.get("Allow_NBFamily") or []
        dt, did = s.get("DeviceType") or [], s.get("DeviceID") or []
        x.execute("INSERT INTO component VALUES(?,?,?,?,?,?,?,?,?,?)",
                  (comp, p.get("TypeID"), s.get("Platform"), 1 if s.get("DenyNB") == 1 else 0,
                   s.get("EndOSVersion"), s.get("ComboSet"), 0, len(allow), len(deny),
                   1 if (dt or did) else 0))
        for m in allow:
            x.execute("INSERT INTO gate VALUES(?,?,?)", (comp, "allow", m)); models.add(m)
        for m in deny:
            x.execute("INSERT INTO gate VALUES(?,?,?)", (comp, "deny", m)); models.add(m)
        for m in nbfam:
            x.execute("INSERT INTO gate VALUES(?,?,?)", (comp, "allow_nbfamily", m)); models.add(m)
        for v in dt:
            x.execute("INSERT INTO gate VALUES(?,?,?)", (comp, "devicetype", str(v)))
        for v in did:
            x.execute("INSERT INTO gate VALUES(?,?,?)", (comp, "deviceid", str(v)))
    for m in models:
        x.execute("INSERT OR IGNORE INTO model VALUES(?,?)", (m, 1 if m.startswith("*") else 0))

    denysets = {p["Component"]: set((p.get("Support") or {}).get("Deny") or []) for p in pkgs}
    for (m,) in x.execute("SELECT name FROM model").fetchall():
        for p in pkgs:
            comp, s = p["Component"], p.get("Support")
            via = None
            if s is None:
                via = "universal"
            else:
                allow = set(s.get("Allow") or []) | set(s.get("Allow_NBFamily") or [])
                plat = s.get("Platform") or ""
                if m in allow and m not in denysets[comp]:
                    via = "allow"
                elif not allow and not (s.get("DeviceType") or s.get("DeviceID")) and ("1" in plat or "7" in plat):
                    via = "platform-nb"
            if via:
                x.execute("INSERT INTO model_component VALUES(?,?,?)", (m, comp, via))
    c.commit()
    n = lambda q: x.execute(q).fetchone()[0]
    print(f"components={n('SELECT COUNT(*) FROM component')} "
          f"universal={n('SELECT COUNT(*) FROM component WHERE universal=1')} "
          f"models={n('SELECT COUNT(*) FROM model')} "
          f"edges={n('SELECT COUNT(*) FROM model_component')} -> {db_path}")
    c.close()


def main() -> int:
    ap = argparse.ArgumentParser(description="Build SQLite census from PackageDataV2.json")
    ap.add_argument("pkg_json")
    ap.add_argument("-o", "--outfile", default="msi-nb-manifest.sqlite")
    args = ap.parse_args()
    build(args.pkg_json, args.outfile)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
