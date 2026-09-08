#!/usr/bin/env python3
"""Re-chooses the table strength of an already calibrated preset, the way the
conveyor now does for new ones, then validates and audits it again."""
from __future__ import annotations

import json
import sys

from calibrate_all import OPTIMIZED, LEDGER, choose_table_strength, last_number, run


def main() -> int:
    ledger = json.loads(LEDGER.read_text()) if LEDGER.exists() else {}
    for preset in sys.argv[1:]:
        best = OPTIMIZED / preset / "best.json"
        document = json.loads(best.read_text())
        if "color_table" not in document["settings"]:
            print(f"{preset}: keine Tabelle", flush=True)
            continue
        before = document["settings"]["color_table"]["strength"]
        strength, arc = choose_table_strength(preset, document, OPTIMIZED / preset / "strength")
        print(f"{preset}: Staerke {before} -> {strength} (Bogenmass {arc})", flush=True)
        if strength != before:
            document["settings"]["color_table"]["strength"] = strength
            best.write_text(json.dumps(document, indent=1))
            run(["validate_preset.py", preset], minutes=120)
            audit = run(["audit_preset.py", preset], minutes=120)
            validation = json.loads((OPTIMIZED / preset / "validation.json").read_text())
            record = ledger.setdefault(preset, {"preset": preset})
            record.update(table_strength=strength, sky_arc=arc,
                          delta_e=round(validation["delta_e_mean"], 3),
                          ssim=round(validation["ssim_mean"], 4),
                          images_with_findings=last_number(audit, "images have findings"))
            LEDGER.write_text(json.dumps(ledger, indent=1))
            print(f"   dE {record['delta_e']} ssim {record['ssim']}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
