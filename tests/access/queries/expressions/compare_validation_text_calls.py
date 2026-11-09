"""Independently model stored Len/explicit binary StrComp validation read audits.

Only a separate original reference-check mode evaluates supplied pure rules.
The accepted main corpus and all getter/CLI inspection remain inert.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess

from cli_tsv import display_tsv
from compare_browsing import display
from compare_calculated_decimals import digest, JACKCESS_SHA256

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
PROFILES = ("V2016", "V2019")
UNSUPPORTED = {
    "Locale": 'StrComp([Label],"ABC",1)=0',
    "MissingMode": 'StrComp([Label],"ABC")=0',
    "Upper": 'UCase([Label])="ABC"',
    "Clock": "Date() Is Not Null",
    "Unknown": "UnknownFunction([Label])=0",
    "Numeric": "Len([ID])<100",
    "Foreign": "Len([Foreign].[Label])<100",
    "CrossField": "Len([Other])<100",
}


def execute(*arguments):
    result = subprocess.run(
        list(map(str, arguments)), capture_output=True, text=True, timeout=120
    )
    assert result.returncode == 0, result.stderr
    return result.stdout


def text(tag):
    if tag == "N":
        return None
    assert tag.startswith("T")
    return bytes.fromhex(tag[1:]).decode("utf-8")


def units(value):
    encoded = value.encode("utf-16le")
    return struct.unpack("<" + "H" * (len(encoded) // 2), encoded)


def records(output):
    tables = {}
    for line in output.splitlines():
        kind, *cells = line.split("\t")
        if kind == "T":
            name, rule = cells
            tables[name] = {"table_rule": text(rule), "fields": {}, "rows": []}
        elif kind == "C":
            name, column, carrier, rule = cells
            tables[name]["fields"][column] = (carrier, text(rule))
        elif kind == "R":
            name, ordinal, *values = cells
            assert int(ordinal) == len(tables[name]["rows"])
            tables[name]["rows"].append(values)
        else:
            raise AssertionError("Unexpected original validation oracle record")
    return tables


def contract(name):
    field = name.startswith("Field")
    suffix = name.removeprefix("Field").removeprefix("Table")
    if suffix.startswith("Len"):
        guarded = suffix == "LenNullable"
        rule = "Len([Label])<100"
        if guarded:
            rule = "[Label] Is Null Or " + rule
        return field, rule, "length_guarded" if guarded else "length"
    if suffix.startswith("Binary") and not name.startswith("BinaryPairs"):
        return field, 'StrComp([Label],"ABC",0)=0', "equal"
    if name.startswith("BinaryPairs"):
        guarded = name.endswith("Nullable")
        rule = "StrComp([Label],[Other],0)<=0"
        if guarded:
            rule = "[Label] Is Null Or [Other] Is Null Or " + rule
        return False, rule, "ordered_guarded" if guarded else "ordered"
    excluded = suffix.removesuffix("Empty").removesuffix("Stored")
    return field, UNSUPPORTED[excluded], "unsupported"


def outcome(row, model):
    _, left_tag, right_tag = row
    left, right = text(left_tag), text(right_tag)
    if model == "unsupported":
        return None
    if model == "length_guarded" and left is None:
        return True
    if model.startswith("length"):
        return None if left is None else len(units(left)) < 100
    if model == "equal":
        return None if left is None else units(left) == units("ABC")
    if model == "ordered_guarded" and (left is None or right is None):
        return True
    if left is None or right is None:
        return None
    return units(left) <= units(right)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite accepted evidence"
    assert digest(args.jar) == JACKCESS_SHA256
    oracle = RECIPES / "ValidationTextCallOracle.java"
    paths = {
        args.cli, args.jar, oracle, Path(__file__),
        RECIPES / "ValidationTextCallFixture.java",
        *[RECIPES / name for name in (
            "cli_tsv.py", "compare_browsing.py", "compare_calculated_decimals.py", "float_model.py",
        )],
        *sorted((ROOT / "crates/engines/tablecraft-ace/src/constraints/rules").rglob("*.rs")),
        ROOT / "crates/engines/tablecraft-ace/src/constraints/fields.rs",
    }
    for profile in PROFILES:
        paths.add(args.directory / f"{profile}.accdb")
        paths.add(args.directory / f"reference-{profile}.accdb")
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    controls = 0
    getter_cells = 0
    observations = []

    for profile in PROFILES:
        path = args.directory / f"{profile}.accdb"
        original = execute("java", "-cp", args.jar, oracle, path)
        tables = records(original)
        reference_output = execute(
            "java", "-cp", args.jar, oracle, args.directory / f"reference-{profile}.accdb"
        )
        reference = records(reference_output)
        for name, table in reference.items():
            assert table == tables[name], "Pure probe getters differ from inert equivalent"
        observations.append({
            "profile": profile, "tables": len(tables), "rows": sum(len(t["rows"]) for t in tables.values()),
            "oracle_sha256": hashlib.sha256(original.encode()).hexdigest(),
            "reference_oracle_sha256": hashlib.sha256(reference_output.encode()).hexdigest(),
            "rules": [],
        })

        for name, table in tables.items():
            field, rule, model = contract(name)
            assert table["fields"]["Label"][0] == "TEXT"
            actual = table["fields"]["Label"][1] if field else table["table_rule"]
            assert actual == rule, (name, actual, rule)
            expected = "ID\tLabel\tOther\n" + "".join(
                "\t".join(display(value) for value in row) + "\n" for row in table["rows"]
            )
            returned = display_tsv(execute(
                args.cli, "--format", "tsv", "query", "run", f"SELECT * FROM [{name}]",
                "--database", path,
            ))
            assert returned == expected, (name, returned, expected)
            getter_cells += 3 * len(table["rows"])
            controls += 1

            outcomes = [outcome(row, model) for row in table["rows"]]
            failed = any(value is False for value in outcomes)
            unsupported = model == "unsupported" or any(value is None for value in outcomes)
            audit = subprocess.run(
                [str(args.cli), "--format", "tsv", "inspect", str(path),
                 "--table", name, "--constraints"], capture_output=True, text=True, timeout=120,
            )
            if failed:
                message = ("field" if field else "table") + " validation rule rejected the record"
                assert audit.returncode == 2 and not audit.stdout and message in audit.stderr, audit.stderr
                status = "Rejected"
            else:
                assert audit.returncode == 0, audit.stderr
                status = "Not checked: unsupported rule" if unsupported else "Checked"
                lines = [line.split("\t") for line in display_tsv(audit.stdout).splitlines()]
                checked = next(row for row in lines if row[0] == ("Label" if field else "Table validation rule"))
                assert checked[-1] == status, (name, checked, status)
                assert ["Rows checked", str(len(table["rows"]))] in lines
            controls += 1
            observations[-1]["rules"].append({
                "table": name, "scope": "field" if field else "table", "rule": rule,
                "rows": len(table["rows"]), "status": status,
                "true": sum(value is True for value in outcomes),
                "false": sum(value is False for value in outcomes),
                "unknown": sum(value is None for value in outcomes),
            })

    assert all(digest(ROOT / path) == value for path, value in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "getter_cells": getter_cells, "hashes_unchanged": True,
        "inputs": hashes, "observations": observations,
    }
    with args.report.open("x") as destination:
        destination.write(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} validation text-call controls and {getter_cells} stored getter cells")


if __name__ == "__main__":
    main()
