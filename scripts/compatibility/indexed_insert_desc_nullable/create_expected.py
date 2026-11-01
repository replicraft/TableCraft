"""Reproduce fixed nullable/DESC INSERT source images/reports without SDK/engine calls."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
INDEXES = ("PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias")
# Historical source-phase input/code authorities retained for byte-exact replay.
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
    parser = argparse.ArgumentParser()
    for name in ("h5-original", "h6-original", "h5-report", "h6-report", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists() or args.output.is_symlink() or not __debug__:
        raise ValueError("Fresh output and enabled source assertions required")
    model = module("literal_optional_insert", HERE / "model.py")
    reports = module("literal_optional_insert_reports", HERE / "reports.py")
    requests = json.loads((HERE / "requests.json").read_text())
    pending = {}
    views = []
    for header, original_path, report_path in (
        (5, args.h5_original, args.h5_report),
        (6, args.h6_original, args.h6_report),
    ):
        before = original_path.read_bytes()
        original_report = report_path.read_bytes()
        if digest(original_report) != REPORT_HASHES[header]:
            raise ValueError("Pinned complete original observer report required")
        model.qualify(before, header)
        for request in [None, *requests["cases"]]:
            recipe = "original" if request is None else request["id"]
            if request is None:
                after, proof = before, None
            else:
                after, proof = model.construct(before, header, request)
            name = f"h{header}-desc-null-insert-{recipe}"
            image_name = name + ".accdb"
            pending[image_name] = after
            view = {"id": name, "header": header, "recipe": recipe, "request": request,
                    "image": {"file": image_name, "bytes": len(after), "sha256": digest(after)},
                    "expected": {}}
            observations = {"inventory": reports.full(before, after, original_report, header, recipe),
                            "metadata": reports.metadata(header)}
            for index in INDEXES:
                observations["cursor-" + index] = reports.cursor(recipe, index)
            if request is None:
                if observations["inventory"] != original_report:
                    raise ValueError("Complete original report identity")
            for kind, raw in observations.items():
                filename = f"{name}-{kind}.tsv"
                pending[filename] = raw
                view["expected"][kind] = {"file": filename, "bytes": len(raw), "sha256": digest(raw)}
            if proof is not None:
                filename = name + ".proof.json"
                raw = encoded(proof)
                pending[filename] = raw
                view["proof"] = {"file": filename, "bytes": len(raw), "sha256": digest(raw)}
            views.append(view)
    if len(views) != 6 or len(pending) != 46 or sum(map(len, pending.values())) > 64 * 1024 * 1024:
        raise ValueError("Closed six-view bounded source inventory")
    manifest = {"state": "LITERAL_IMAGES_AND_EXPECTATIONS_BEFORE_READERS",
                "input_freeze_sha256": "fe28a3fe31ef47094d90037e365194b64bc27a4e75184be8b8538671c16de64f",
                "code_freeze_sha256": "dd68f3d99d2358c020500623646731a1ebf2d1980e11a61b01608bf32e5558e0",
                "views": views, "read_only_calls": 36, "native_producer_calls": 0, "child_calls": 0}
    pending["manifest.json"] = encoded(manifest)
    args.output.mkdir(parents=True, exist_ok=False)
    for filename, raw in pending.items():
        with (args.output / filename).open("xb") as handle:
            handle.write(raw)
    print(json.dumps({"status": "PASS", "views": 6, "artifacts": 47,
                      "manifest_sha256": digest(pending["manifest.json"]), "native_producer_calls": 0, "child_calls": 0}))


if __name__ == "__main__":
    main()
