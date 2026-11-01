"""Reproduce pinned DESC/NULL DELETE source expectations; no SDK or engine calls."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPORT_HASHES = {5: '32e26cc2d401d9f3af35c0884835db5c1c411052c2e691dd036ae13cdf72a213', 6: '2f1540ffe03cfa7e118afc8d38cfdcd8be1c71c9a435641c257c9e95f7ac4ee8'}


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2) + "\n").encode()


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("h5-original", "h6-original", "h5-report", "h6-report", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    if not __debug__ or args.output.exists() or args.output.is_symlink():
        raise ValueError("Enabled source assertions and fresh output required")
    model = module("desc_nullable_literal_model", HERE / "model.py")
    reports = module("desc_nullable_literal_reports", HERE / "reports.py")
    recipes = json.loads((HERE / "requests.json").read_text())["recipes"]
    artifacts = {}
    cases = []
    for header, original_path, report_path in (
        (5, args.h5_original, args.h5_report),
        (6, args.h6_original, args.h6_report),
    ):
        before = original_path.read_bytes()
        original_report = report_path.read_bytes()
        if digest(original_report) != REPORT_HASHES[header]:
            raise ValueError("Pinned original complete observer report required")
        model.qualify(before, header)
        if reports.full(before, before, original_report, header, range(3)) != original_report:
            raise ValueError("Complete original observer identity")
        for recipe in recipes:
            after, proof = model.construct(before, header, recipe)
            name = f"h{header}-desc-null-delete-{recipe['id']}"
            case = {"id": name, "header": header, "recipe": recipe["id"],
                    "selected": recipe["selected"], "request_orders": recipe["request_orders"],
                    "retained": recipe["remaining"], "cursors": {}}
            for kind, filename, raw in (
                ("before", name + "-before.accdb", before),
                ("expected", name + "-deleted.accdb", after),
                ("before_reader", name + "-before.inventory.tsv", original_report),
                ("reader", name + "-deleted.inventory.tsv",
                 reports.full(before, after, original_report, header, recipe["remaining"])),
                ("proof", name + ".proof.json", encoded(proof)),
            ):
                artifacts[filename] = raw
                case[kind] = {"file": filename, "bytes": len(raw), "sha256": digest(raw)}
            for index in reports.INDEXES:
                case["cursors"][index] = {}
                for phase, retained in (("before", range(3)), ("expected", recipe["remaining"])):
                    filename = f"{name}-{phase}-{index}.cursor.tsv"
                    raw = reports.cursor(retained, index)
                    artifacts[filename] = raw
                    case["cursors"][index][phase] = {"file": filename, "bytes": len(raw), "sha256": digest(raw)}
            cases.append(case)
    if len(cases) != 8 or len(artifacts) != 104 or sum(map(len, artifacts.values())) > 64 * 1024 * 1024:
        raise ValueError("Closed bounded source inventory")
    artifacts["manifest.json"] = encoded({"schema": 1, "cases": cases, "source_views": 16,
        "sdk_inventory_calls": 16, "sdk_cursor_calls": 64, "native_producer_calls": 0,
        "tool_pins": [{"file": name, "sha256": digest((HERE / name).read_bytes())}
                      for name in ("model.py", "reports.py", "requests.json", "create_expected.py")]})
    args.output.mkdir(parents=True, exist_ok=False)
    for filename, raw in artifacts.items():
        with (args.output / filename).open("xb") as handle:
            handle.write(raw)
    print(json.dumps({"source_views": 16, "artifacts": 105,
                      "manifest_sha256": digest(artifacts["manifest.json"]), "native_producer_calls": 0}))


if __name__ == "__main__":
    main()
