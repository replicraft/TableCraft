"""Compare explicit cached Byte/Int16/Int64/Single/Guid readers with inert getters.

Original files remain unchanged. No formulas execute, and no native Access SQL,
cached freshness, mutation or calculated-result coercion is qualified.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import subprocess
import tempfile
import uuid

from cli_tsv import display_tsv
from compare_browsing import display
from compare_calculated_decimals import digest, JACKCESS_SHA256

ROOT = Path(__file__).resolve().parents[2]
RECIPES = Path(__file__).resolve().parent
PROFILES = ("V2016", "V2019")
KINDS = {
    "BYTE": (2, "Byte"),
    "INT": (3, "Int16"),
    "BIG_INT": (19, "Int64"),
    "FLOAT": (6, "Single"),
    "GUID": (15, "Guid"),
}


def execute(*arguments):
    result = subprocess.run(list(map(str, arguments)), capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stderr
    return result.stdout


def records(output):
    tables = {}
    for line in output.splitlines():
        tag, name, *cells = line.split("\t")
        if tag == "D":
            kind, capacity, calculated, result = cells
            assert capacity == "39" and calculated == "true"
            assert int(result) == KINDS[kind][0]
            tables[name] = {"kind": kind, "rows": []}
        elif tag == "R":
            ordinal, identity, value = cells
            assert int(ordinal) == len(tables[name]["rows"])
            tables[name]["rows"].append((int(identity), value))
        else:
            raise AssertionError("Unexpected original getter record")
    assert len(tables) == 12
    return tables


def assert_cell(actual, tagged):
    if not tagged.startswith("F"):
        assert actual == display(tagged), (actual, tagged)
        return
    expected = struct.unpack(">f", bytes.fromhex(tagged[1:]))[0]
    if math.isnan(expected):
        # CLI textual NaN cannot carry its sign/payload. Engine unit tests and
        # the independent getter retain raw bits separately.
        assert math.isnan(float(actual))
    else:
        assert struct.pack(">f", float(actual)) == bytes.fromhex(tagged[1:]), (actual, tagged)


def payload(tagged):
    if tagged.startswith("U"):
        return int(tagged[1:]).to_bytes(1, "little")
    if tagged.startswith("I:"):
        value = int(tagged[2:])
        return value.to_bytes(2 if -32768 <= value <= 32767 else 8, "little", signed=True)
    if tagged.startswith("F"):
        return bytes.fromhex(tagged[1:])[::-1]
    if tagged.startswith("G"):
        return uuid.UUID(tagged[1:]).bytes_le
    raise AssertionError("Expected a nonnull unique late cache")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    arguments = parser.parse_args()
    assert not arguments.report.exists(), "Refuse to overwrite accepted evidence"
    assert digest(arguments.jar) == JACKCESS_SHA256
    paths = {
        arguments.cli, arguments.jar, Path(__file__),
        RECIPES / "CalculatedCarrierFixture.java", RECIPES / "CalculatedCarrierOracle.java",
        RECIPES / "cli_tsv.py", RECIPES / "compare_browsing.py",
        RECIPES / "compare_calculated_decimals.py", RECIPES / "float_model.py",
        *sorted((ROOT / "crates/engines/tablecraft-ace/src/table/calculated").glob("*.rs")),
        *[arguments.directory / f"{profile}.accdb" for profile in PROFILES],
    }
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    controls = 0
    cached_cell_comparisons = 0
    observations = []

    def cli(*options):
        return display_tsv(execute(arguments.cli, "--format", "tsv", *options))

    def query(path, sql, *options):
        return cli("query", "run", sql, "--database", path, "--stored-calculations", *options)

    def refused(*options, reason=None):
        result = subprocess.run(
            list(map(str, (arguments.cli, *options))), capture_output=True, timeout=120
        )
        assert result.returncode == 2 and not result.stdout, result.stderr
        if reason is not None:
            assert reason.encode() in result.stderr, result.stderr

    for profile in PROFILES:
        path = arguments.directory / f"{profile}.accdb"
        original = path.read_bytes()
        oracle = execute("java", "-cp", arguments.jar, RECIPES / "CalculatedCarrierOracle.java", path)
        for name, table in records(oracle).items():
            kind, scalar = KINDS[table["kind"]]
            rows = table["rows"]
            exceptional = any(
                value.startswith("F") and not math.isfinite(struct.unpack(">f", bytes.fromhex(value[1:]))[0])
                for _, value in rows
            )
            inspected = cli("inspect", path, "--table", name)
            declaration = next(line.split("\t")[1:] for line in inspected.splitlines() if line.startswith("Cached\t"))
            assert declaration == [str(kind), "39", "0x02", "0xc0", "Unsupported"]
            controls += 1

            header = f"Table\t{name}\nColumn\tCached\nValues\tStored; not recalculated\n"
            header += f"Row ordinal\tStored {scalar} value\n"
            stored = cli("inspect", path, "--table", name, "--stored-calculation", "Cached")
            assert stored.startswith(header)
            values = [line.split("\t") for line in stored[len(header):].splitlines()]
            assert len(values) == len(rows)
            for ordinal, ((actual_ordinal, actual), (_, tagged)) in enumerate(zip(values, rows)):
                assert int(actual_ordinal) == ordinal
                assert_cell(actual, tagged)
            controls += 1
            cached_cell_comparisons += len(rows)

            consumers = [(cli("rows", path, name, "--stored-calculations"), "ID\tCached [cached]")]
            if exceptional:
                for sql in (f"SELECT ID,Cached FROM [{name}]", f"SELECT TOP 1 Cached FROM [{name}]"):
                    refused("query", "run", sql, "--database", path, "--stored-calculations",
                            reason="non-finite number")
                    controls += 1
            else:
                consumers.append((query(path, f"SELECT ID,Cached FROM [{name}]"), "ID\tCached"))
            for actual, expected_header in consumers:
                heading, *actual_rows = actual.splitlines()
                assert heading == expected_header and len(actual_rows) == len(rows)
                for actual_row, (identity, tagged) in zip(actual_rows, rows):
                    actual_id, cell = actual_row.split("\t")
                    assert int(actual_id) == identity
                    assert_cell(cell, tagged)
                controls += 1
                cached_cell_comparisons += len(rows)
            refused("query", "run", f"SELECT Cached FROM [{name}]", "--database", path)
            controls += 1
            if not rows:
                continue

            if exceptional:
                for sql in (
                    f"SELECT Count(Cached) AS N FROM [{name}]",
                    f"SELECT ID FROM [{name}] WHERE Cached IS NULL",
                ):
                    refused("query", "run", sql, "--database", path, "--stored-calculations",
                            reason="non-finite number")
            else:
                assert query(path, f"SELECT Count(Cached) AS N FROM [{name}]") == "N\n6\n"
                assert query(path, f"SELECT ID FROM [{name}] WHERE Cached IS NULL") == "ID\n4\n"
            assert cli("query", "run", f"SELECT Count(*) AS N FROM [{name}]", "--database", path) == "N\n7\n"
            controls += 3
            if not exceptional:
                for flag, reason in (
                    ("--native-bytes", "Bytes resource limit"),
                    ("--execution-steps", "Steps resource limit"),
                    ("--output-bytes", "output limit"),
                ):
                    refused("query", "run", f"SELECT Cached FROM [{name}]", "--database", path,
                            "--stored-calculations", flag, "1", reason=reason)
                    controls += 1

            late = payload(rows[-1][1])
            wrapped = bytes(16) + len(late).to_bytes(4, "little") + late + bytes(3)
            assert original.count(wrapped) == 1
            changed = bytearray(original)
            changed[changed.index(wrapped)] = 1
            with tempfile.TemporaryDirectory(prefix="tablecraft-calculated-carriers-") as directory:
                copy = Path(directory) / "invalid.accdb"
                copy.write_bytes(changed)
                refused("inspect", copy, "--table", name, "--stored-calculation", "Cached")
                refused("rows", copy, name, "--stored-calculations")
                for sql in (
                    f"SELECT TOP 1 Cached FROM [{name}]",
                    f"SELECT Cached FROM [{name}] WHERE ID=-1",
                ):
                    refused("query", "run", sql, "--database", copy, "--stored-calculations")
                assert query(copy, f"SELECT Count(*) AS N FROM [{name}]") == "N\n7\n"
                assert copy.read_bytes() == changed
                controls += 5
            observations.append({
                "profile": profile, "table": name, "carrier": scalar, "rows": len(rows),
                "getter_values": [value for _, value in rows],
                "oracle_sha256": hashlib.sha256(oracle.encode()).hexdigest(),
            })

    assert all(digest(ROOT / path) == value for path, value in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "cached_cell_comparisons": cached_cell_comparisons, "hashes_unchanged": True,
        "hashes": hashes, "observations": observations,
        "limitations": "Independent inert cached getters only; no native Access, formula or freshness qualification. "
                        "CLI NaN compares category; raw payload is preserved in engine tests and getter observations.",
    }
    arguments.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} controls and {cached_cell_comparisons} cached-cell comparisons")


if __name__ == "__main__":
    main()
