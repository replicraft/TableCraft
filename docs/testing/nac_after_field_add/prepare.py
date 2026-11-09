"""Freeze predictions and original snapshots before any SDK mutation.

Run from the repository root in a fresh recipe directory. The manifest maps
each named case to a user-supplied generated source path and SHA-256 pin.
"""

import datetime
import hashlib
import json
from pathlib import Path

import model


def main():
    root = Path(__file__).parent
    manifest = json.loads((root / "source_manifest.json").read_text())
    for name in ("inputs", "expected", "outputs"):
        (root / name).mkdir()
    cases = []
    for source in manifest:
        original = Path(source["source"]).read_bytes()
        assert hashlib.sha256(original).hexdigest() == source["source_sha256"]
        expected, facts = model.expected(original, Path("."))
        filename = source["case"] + ".accdb"
        (root / "inputs" / filename).write_bytes(original)
        (root / "expected" / filename).write_bytes(expected)
        cases.append({**source, "expected_sha256": hashlib.sha256(expected).hexdigest(),
                      "expected_bytes": len(expected), **facts})
    report = {"created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "model_sha256": hashlib.sha256((root / "model.py").read_bytes()).hexdigest(),
              "cases": cases}
    (root / "predictions-before-sdk.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
