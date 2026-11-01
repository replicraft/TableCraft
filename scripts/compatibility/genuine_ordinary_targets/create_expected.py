#!/usr/bin/env python3
"""Construct frozen genuine-source expectations without invoking a database runtime."""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path

import model
import reports

ORIGINAL_REPORTS = {
    5: (918249, "a7b1781d83001aaf568fdc42219fefbf9409e5d1074236dd3536f506852bb7c8"),
    6: (971376, "d4c61254651e03ddc172a6d0c6e0cc5d1fab599d158ab0f45b321eb5db5a01d4"),
}
MAX_ARTIFACT_BYTES = 64 * 1024 * 1024
REQUESTS_SHA256 = "053995b3538c0a424cbab102ac55930219ddcaf1b349081b807be9ca788ab28f"


def read_pinned(path, length, expected_digest):
    with path.open("rb") as stream:
        raw = stream.read(length + 1)
    if len(raw) != length or hashlib.sha256(raw).hexdigest() != expected_digest:
        raise ValueError(f"pinned input does not match: {path}")
    return raw


def json_bytes(value):
    return (json.dumps(value, indent=2) + "\n").encode("utf-8")


def build(originals, original_reports, inventory):
    """Return all artifacts before creating any output directory."""
    artifacts = {}
    proofs = []
    views = []
    for header in (5, 6):
        before = originals[header]
        qualified = model.qualify(before, header)
        per_header = [{
            "id": f"h{header}-original",
            "header": header,
            "file": f"h{header}-original.accdb",
            "rows": [list(row) for row in model.ROWS],
            "sha256": model.digest(before),
        }]
        artifacts[per_header[0]["file"]] = before
        for case in inventory["cases"]:
            if case["header"] != header or case["expected_outcome"]["kind"] != "candidate":
                continue
            after, proof = model.construct(before, qualified, case)
            artifacts[case["id"] + ".accdb"] = after
            proofs.append(proof)
            per_header.append({
                "id": case["id"],
                "header": header,
                "file": case["id"] + ".accdb",
                "rows": proof["rows"],
                "sha256": proof["after_sha256"],
            })
        if len(per_header) != 6:
            raise ValueError("expected one original and five outcomes per header")
        for view in per_header:
            view_id = view["id"]
            report_id = view_id if view_id.endswith("-original") else view_id + "-after"
            after = artifacts[view["file"]]
            artifacts[f"reports/{report_id}.inventory.tsv"] = reports.full_observer(
                before, after, original_reports[header], header, view["rows"]
            )
            for index in reports.LOGICAL_INDEXES:
                artifacts[f"reports/{report_id}-cursor-{index}.tsv"] = reports.cursor(
                    view["rows"], index
                )
            views.append(view)
    if len(views) != 12 or len(proofs) != 10:
        raise ValueError("closed source request counts do not match")
    artifacts["proofs.json"] = json_bytes(proofs)
    artifacts["views.json"] = json_bytes(views)
    native = io.StringIO(newline="")
    writer = csv.writer(native, delimiter="\t", lineterminator="\n")
    writer.writerow(("id", "header", "file", "id0", "score0", "guard0", "sha256"))
    for view in views:
        first = ["NULL" if value is None else value for value in view["rows"][0]]
        writer.writerow((view["id"], view["header"], view["file"], *first, view["sha256"]))
    artifacts["native-views.tsv"] = native.getvalue().encode("utf-8")
    if sum(map(len, artifacts.values())) > MAX_ARTIFACT_BYTES:
        raise ValueError("closed artifact set exceeds its output envelope")
    return artifacts


def arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--h5-original", required=True, type=Path)
    parser.add_argument("--h6-original", required=True, type=Path)
    parser.add_argument("--h5-report", required=True, type=Path)
    parser.add_argument("--h6-report", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path, help="fresh, absent output directory")
    return parser.parse_args()


def main():
    args = arguments()
    if not __debug__:
        raise ValueError("source expectation checks require Python assertions")
    if args.output.exists() or args.output.is_symlink():
        raise ValueError("output directory must be absent")
    original_paths = {5: args.h5_original, 6: args.h6_original}
    report_paths = {5: args.h5_report, 6: args.h6_report}
    originals = {}
    original_reports = {}
    for header in (5, 6):
        facts = model.SOURCES[header]
        originals[header] = read_pinned(original_paths[header], facts["bytes"], facts["sha256"])
        length, digest = ORIGINAL_REPORTS[header]
        original_reports[header] = read_pinned(report_paths[header], length, digest)
    requests = Path(__file__).with_name("requests.json").read_bytes()
    if hashlib.sha256(requests).hexdigest() != REQUESTS_SHA256:
        raise ValueError("maintained original request inventory does not match its freeze")
    inventory = json.loads(requests)
    artifacts = build(originals, original_reports, inventory)

    # All source checks and complete expected artifacts precede filesystem writes.
    # mkdir/open exclusive creation refuse a race or any existing namespace.
    args.output.mkdir()
    (args.output / "reports").mkdir()
    for name, raw in artifacts.items():
        with (args.output / name).open("xb") as stream:
            stream.write(raw)
    print("Constructed 12 source views, 10 proofs and 60 expected reader reports; no database runtime invoked")


if __name__ == "__main__":
    main()
