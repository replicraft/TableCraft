"""Compare explicit cached Memo reads with inert independent Text getters.

No formulas execute; original storage and buffered failures remain separate from
Microsoft Access recalculation, freshness and calculated-Memo admissibility.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import display
from compare_calculated_decimals import digest, run

ROOT = Path(__file__).resolve().parents[2]
RECIPE = ROOT / "docs/testing/CalculatedMemoFixture.java"
JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
SOURCE_FILES = [
    ROOT / "crates/engines/tablecraft-ace/src/table" / name
    for name in ("record.rs", "long_text.rs", "calculated/encoding.rs", "calculated/long_text.rs", "calculated/mod.rs")
]


def wrapped(payload):
    return bytes(16) + len(payload).to_bytes(4, "little") + payload + bytes(3)


def inline(payload):
    return (0x80000000 | len(payload)).to_bytes(4, "little") + bytes(8) + payload


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2016", "ace2019", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JACKCESS_SHA256
    paths = [args.cli, args.jar, RECIPE, Path(__file__), args.ace2016, args.ace2019, *SOURCE_FILES]
    hashes = {str(path): digest(path) for path in paths}
    observations = []
    controls = 0
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPE, "--inspect", path)
        first, second, *records = oracle.splitlines()
        # Requested builder compression does not persist on calculated Memo
        # declarations in this artifact. Record actual getters, not intent.
        assert first == "D\tWide\tMEMO\t0\tfalse"
        assert second == "D\tCompact\tMEMO\t0\tfalse"
        controls += 2
        rows = {name: [] for name in ("Wide", "Compact")}
        for record in records:
            tag, ordinal, name, value = record.split("\t")
            assert tag == "R" and int(ordinal) == len(rows[name])
            rows[name].append(value)
        assert all(len(values) == 7 for values in rows.values())
        for name, values in rows.items():
            header = f"Table\tCalculatedMemos\nColumn\t{name}\nValues\tStored; not recalculated\nRow ordinal\tStored Text value\n"
            expected = header + "".join(f"{ordinal}\t{display(value)}\n" for ordinal, value in enumerate(values))
            assert run(args.cli, "inspect", path, "--table", "CalculatedMemos", "--stored-calculation", name) == expected
            controls += len(values)
            empty = run(args.cli, "inspect", path, "--table", "EmptyCalculatedMemos", "--stored-calculation", name)
            assert empty == header.replace("CalculatedMemos", "EmptyCalculatedMemos", 1)
            controls += 1
        expected = "ID\tWide [cached]\tCompact [cached]\n" + "".join(f"{ordinal + 1}\t{display(rows['Wide'][ordinal])}\t{display(rows['Compact'][ordinal])}\n" for ordinal in range(7))
        assert run(args.cli, "rows", path, "CalculatedMemos", "--stored-calculations", "--format", "tsv") == expected
        controls += 14
        query = subprocess.run([str(args.cli), "query", "run", "SELECT Wide FROM CalculatedMemos", "--database", str(path)], capture_output=True)
        assert query.returncode == 2 and not query.stdout
        controls += 1

        # Public read-only Column.read independently checks compression marker
        # behavior inside authored inline wrappers without changing a database.
        for payload, text in [(b"\xff\xfeASCII", "ASCII"), (b"\xff\xfe", ""), (b"\xff\xfe\x00\xff\xfe", "\ufeff"), ("é🦀".encode("utf-16le"), "é🦀")]:
            actual = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPE, "--decode", path, inline(wrapped(payload)).hex())
            assert actual == "T" + text.encode().hex() + "\n"
            controls += 1

        late = wrapped("late-cache".encode("utf-16le"))
        starts = [position for position in range(len(original)) if original.startswith(late, position)]
        assert len(starts) == 2
        # Both fields share authored values; corrupt only Wide's resolved cache.
        # Checking both explicit inspectors establishes which copy was changed.
        for offset, value in [(0, 1), (16, 19), (len(late) - 1, 1)]:
            changed = bytearray(original)
            changed[starts[0] + offset] = value
            with tempfile.TemporaryDirectory(prefix="tablecraft-calculated-memos-") as directory:
                copy = Path(directory) / "invalid.accdb"
                copy.write_bytes(changed)
                rejected = 0
                for name in rows:
                    result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", "CalculatedMemos", "--stored-calculation", name], capture_output=True)
                    if result.returncode == 2:
                        assert not result.stdout
                        rejected += 1
                    else:
                        assert result.returncode == 0
                assert rejected == 1
                result = subprocess.run([str(args.cli), "rows", str(copy), "CalculatedMemos", "--stored-calculations", "--format", "tsv"], capture_output=True)
                assert result.returncode == 2 and not result.stdout
                assert copy.read_bytes() == changed
            controls += 2
        observations.append({"file": str(path), "bytes": len(original), "sha256": digest(path), "oracle_sha256": hashlib.sha256(oracle.encode()).hexdigest(), "cached_cells": 14, "declarations": [first, second]})
    assert all(digest(Path(path)) == expected for path, expected in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "hashes_unchanged": True,
        "inputs": {str(Path(path).resolve().relative_to(ROOT)): value for path, value in hashes.items()},
        "observations": observations,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} cached Memo controls; all source/input/binary hashes unchanged")


if __name__ == "__main__":
    main()
