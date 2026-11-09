"""Freeze raw rename copies before any independent SDK metadata observation."""
from pathlib import Path
import argparse
import datetime
import hashlib
import json
import model

HERE = Path(__file__).resolve().parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    root = args.output_dir
    root.mkdir(exist_ok=False)
    files = root / "files"
    files.mkdir()
    manifest = json.loads((HERE / "source_manifest.json").read_text())
    cases = []
    for source in manifest["sources"]:
        original = args.sources_dir / source["source"]
        before = original.read_bytes()
        assert digest(before) == source["before_sha256"]
        case = source["case"]
        (files / (case + "-before.accdb")).write_bytes(before)
        for variant, rename_group in [("td-only", False), ("live-group", True)]:
            after, facts = model.rename(
                before, source["old_name"], source["new_name"], rename_group,
            )
            output = files / (case + "-" + variant + ".accdb")
            output.write_bytes(after)
            cases.append({
                "case": case,
                "variant": variant,
                "old_name": source["old_name"],
                "new_name": source["new_name"],
                "source": source["source"],
                "before_sha256": source["before_sha256"],
                "after": str(output.relative_to(root)),
                "after_sha256": digest(after),
                "after_bytes": len(after),
                **facts,
            })
        assert original.read_bytes() == before
    report = {
        "frozen_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "model_sha256": digest((HERE / "model.py").read_bytes()),
        "observer_sha256": digest((HERE / "FieldRenameObserver.java").read_bytes()),
        "scope": "Independent raw hypotheses; no SDK rename or native Access write",
        "cases": cases,
    }
    (root / "predictions-before-observation.json").write_text(
        json.dumps(report, indent=2) + "\n"
    )
    print("FROZEN", len(cases), "complete raw hypotheses before SDK reads")


if __name__ == "__main__":
    main()
