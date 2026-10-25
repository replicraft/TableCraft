"""Compare native DATE candidates with exact retained getters and rational time.

Only TableCraft executes SQL. Jackcess creates inert fresh databases and returns
stored bits/table/index encounter order. No Microsoft Access conformance claim.
"""

import argparse
import csv
from fractions import Fraction
import hashlib
from io import StringIO
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
ROW_COUNT = 20_000
PAGE_BYTES = 4096
PROFILES = ("V2000", "V2007", "V2016", "V2019")
RECIPE = Path(__file__).with_name("DateCandidateFixture.java")
LARGER = (
    "--native-bytes", "67108864",
    "--native-rows", "100000",
    "--native-steps", "10000000",
    "--execution-steps", "10000000",
)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def temporal(bits):
    """Exact IEEE rational, truncation toward zero, then absolute fraction."""
    serial = Fraction.from_float(struct.unpack(">d", bytes.fromhex(bits))[0])
    day = int(serial)
    return day, abs(serial - day)


def checked(command):
    result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=60)
    if result.returncode != 0 or result.stderr:
        raise AssertionError((command, result.returncode, result.stderr))
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed reference artifact"
    assert not args.report.exists(), "Report must be fresh"
    args.directory.mkdir(parents=True, exist_ok=True)
    files = {profile: args.directory / (profile + (".mdb" if profile == "V2000" else ".accdb"))
             for profile in PROFILES}
    if args.generate:
        assert all(not path.exists() for path in files.values()), "Never overwrite original inputs"

    sources = (RECIPE, Path(__file__), Path("docs/testing/StoredDateBitsOracle.java"))
    report = {
        "scope": "Original 20,000-row DATE candidate replay; independent rational predicates, not native SQL qualification",
        "pins": [{"path": str(path), "sha256": digest(path)} for path in (args.jar, args.cli, *sources)],
        "inputs": [],
        "controls": [],
    }

    def save():
        args.report.write_text(json.dumps(report, indent=2) + "\n")

    def query(path, label, sql, expected, options=(), larger=True, error=None):
        command = [str(args.cli), "query", "run", sql, "--database", str(path), "--format", "tsv"]
        command.extend(LARGER if larger else ())
        command.extend(options)
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=60)
        actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        success = (result.returncode == 0 and not result.stderr and actual == expected) if error is None else (
            result.returncode == 2 and not result.stdout and error in result.stderr
        )
        recorded_command = command.copy()
        if label == "corrupt-selected-no-fallback":
            recorded_command[recorded_command.index("--database") + 1] = "<disposable>/" + path.name
        report["controls"].append({
            "profile": path.name,
            "label": label,
            "command": recorded_command,
            "exit_code": result.returncode,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "expected_sha256": hashlib.sha256(json.dumps(expected).encode()).hexdigest(),
            "expected_rows": None if expected is None else len(expected) - 1,
            "stderr": result.stderr,
            "passed": success,
        })
        save()
        assert success, (label, result.returncode, result.stderr, actual[:8], None if expected is None else expected[:8])

    with tempfile.TemporaryDirectory(prefix="tablecraft-date-candidate-reference-") as temporary:
        checked(["javac", "-cp", str(args.jar), "-d", temporary, str(RECIPE)])
        java = ["java", "-Dstdout.encoding=UTF-8", "-cp", os.pathsep.join((str(args.jar), temporary)), "DateCandidateFixture"]
        for profile, path in files.items():
            if args.generate:
                checked([*java, "create", str(path), profile])
            original = digest(path)
            report["inputs"].append({"path": str(path), "sha256": original, "bytes": path.stat().st_size})
            orders = {}
            for order in ("table", "DateAsc", "DateDesc"):
                raw = checked([*java, "read", str(path), order])
                records = [line.split("\t") for line in raw.splitlines()]
                payloads = {bits for _, bits in records}
                assert {
                    "NULL", "0000000000000000", "8000000000000000",
                    "0000000000000001", "8000000000000001",
                    "3fd0000000000000", "bfd0000000000000",
                    "bff4000000000000", "bffc000000000000",
                } <= payloads, "Getter lost retained aliases or subnormal bits"
                rows = [(int(identifier), None if bits == "NULL" else temporal(bits))
                        for identifier, bits in records]
                assert len(rows) == ROW_COUNT
                assert sorted(identifier for identifier, _ in rows) == list(range(ROW_COUNT))
                orders[order] = rows
                report["inputs"][-1][order + "_getter_sha256"] = hashlib.sha256(raw.encode()).hexdigest()
            assert [identifier for identifier, _ in orders["table"]] != list(range(ROW_COUNT))
            assert dict(orders["table"]) == dict(orders["DateAsc"]) == dict(orders["DateDesc"])

            cases = (
                ("epoch-alias", "Moment=#12/30/1899 06:00#", lambda value: value == (0, Fraction(1, 4))),
                ("signed-midnight", "Moment=#12/30/1899#", lambda value: value == (0, 0)),
                ("negative-point", "Moment=#12/29/1899 06:00#", lambda value: value == (-1, Fraction(1, 4))),
                ("negative-range", "Moment>=#12/29/1899 06:00# AND Moment<#12/29/1899 18:00#",
                 lambda value: value is not None and (-1, Fraction(1, 4)) <= value < (-1, Fraction(3, 4))),
                ("reversed-operand", "#12/29/1899 18:00#>Moment AND #12/29/1899 06:00#<=Moment",
                 lambda value: value is not None and (-1, Fraction(1, 4)) <= value < (-1, Fraction(3, 4))),
                ("empty-temporal-range", "Moment>=#12/29/1899 18:00# AND Moment<#12/29/1899 06:00#",
                 lambda value: False),
                ("null", "Moment IS NULL", lambda value: value is None),
                ("not-null", "Moment IS NOT NULL", lambda value: value is not None),
                ("null-intersection", "Moment IS NULL AND Moment>#12/30/1899#", lambda value: False),
            )
            for label, predicate, matches in cases:
                for order, options in (
                    ("table", ()),
                    ("table", ("--auto-index",)),
                    ("DateAsc", ("--index", "DateCandidates", "DateAsc")),
                    ("DateDesc", ("--index", "DateCandidates", "DateDesc")),
                ):
                    expected = [["ID"], *[[str(identifier)] for identifier, value in orders[order] if matches(value)]]
                    query(path, label + ":" + ("auto" if options == ("--auto-index",) else order),
                          "SELECT ID FROM DateCandidates WHERE " + predicate, expected, options)

            for label, predicate, matches in cases[:3] + cases[6:7]:
                expected = [["ID"], *[[str(identifier)] for identifier, value in orders["table"] if matches(value)]]
                query(path, label + ":default-selected", "SELECT ID FROM DateCandidates WHERE " + predicate,
                      expected, ("--auto-index",), larger=False)

            for value, endpoint in (("1899-12-30 06:00:00", (0, Fraction(1, 4))),
                                    ("1899-12-30", (0, Fraction(0))),
                                    ("1899-12-29 18:00:00", (-1, Fraction(3, 4)))):
                expected = [["ID"], *[[str(identifier)] for identifier, actual in orders["table"] if actual == endpoint]]
                query(path, "typed-parameter:" + value,
                      "PARAMETERS P DATETIME; SELECT ID FROM DateCandidates WHERE P=Moment", expected,
                      ("--auto-index", "--param", "P=date:" + value))

            query(path, "default-full-snapshot-refusal", "SELECT Count(*) AS N FROM DateCandidates", None,
                  larger=False, error="query structural limit exceeded")
            query(path, "default-wide-candidate-refusal", "SELECT ID FROM DateCandidates WHERE Moment IS NOT NULL", None,
                  ("--auto-index",), larger=False, error="query structural limit exceeded")
            query(path, "selected-prefix-is-not-complete-cache",
                  "SELECT ID,(SELECT Count(*) FROM DateCandidates) AS N FROM DateCandidates WHERE Moment=#12/30/1899 06:00#",
                  None, ("--auto-index",), larger=False, error="query structural limit exceeded")
            query(path, "unsafe-eager-and", "SELECT ID FROM DateCandidates WHERE Moment=#12/30/1899 06:00# AND 1/0=0",
                  None, ("--auto-index",), error="division by zero")

            metadata = checked([str(args.cli), "inspect", str(path), "--table", "DateCandidates", "--indexes", "--format", "tsv"])
            entries = list(csv.reader(StringIO(metadata), delimiter="\t", strict=True))
            roots = {row[0]: int(row[3]) for row in entries if row and row[0] in ("DateAsc", "DateDesc")}
            assert set(roots) == {"DateAsc", "DateDesc"}
            image = path.read_bytes()
            assert all(image[root * PAGE_BYTES] == 3 for root in roots.values()), "Expected internal node trees"
            with tempfile.TemporaryDirectory(prefix="tablecraft-date-candidate-corrupt-") as corrupted:
                copy = Path(corrupted) / path.name
                changed = bytearray(image)
                changed[roots["DateAsc"] * PAGE_BYTES] = 0
                copy.write_bytes(changed)
                query(copy, "corrupt-selected-no-fallback", "SELECT ID FROM DateCandidates WHERE Moment=#12/30/1899 06:00#",
                      None, ("--index", "DateCandidates", "DateAsc"), error="query source failed")
            assert digest(path) == original, "Original changed"
            report["inputs"][-1]["unchanged"] = True
            save()
            print(profile, "passed", flush=True)

    for pin in report["pins"]:
        assert digest(pin["path"]) == pin["sha256"], "Artifact/source changed during replay"
    report["all_pins_unchanged"] = True
    report["control_count"] = len(report["controls"])
    save()
    print(report["control_count"], "controls passed; native Access unrun")


if __name__ == "__main__":
    main()
