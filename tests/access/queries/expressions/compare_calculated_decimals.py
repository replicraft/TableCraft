"""Compare explicit cached Decimal reads with inert independent BigDecimal getters.

No formulas execute. This checks exact coefficient/scale, cached CLI integration
and buffered corruption refusal, not Microsoft Access calculation semantics.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from cli_tsv import command_output

JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
RECIPE = Path(__file__).with_name("CalculatedDecimalFixture.java")
ROOT = Path(__file__).resolve().parents[2]
SOURCE_FILES = [
    ROOT / "crates/engines/tablecraft-ace/src/table/calculated" / name
    for name in ("decimal.rs", "encoding.rs", "mod.rs", "page.rs", "read.rs")
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*arguments):
    result = subprocess.run(list(map(str, arguments)), capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    return command_output(arguments, result.stdout)


def refused(cli, path, table):
    arguments = [cli, "inspect", path, "--table", table, "--stored-calculation", "Amount"]
    result = subprocess.run(list(map(str, arguments)), capture_output=True)
    assert result.returncode == 2 and not result.stdout, result.stderr


def decimal_text(coefficient, scale):
    negative = coefficient.startswith("-")
    digits = coefficient.removeprefix("-").zfill(scale + 1)
    value = digits if scale == 0 else digits[:-scale] + "." + digits[-scale:]
    return ("-" if negative else "") + value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2016", "ace2019", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JACKCESS_SHA256
    paths = [args.cli, args.jar, RECIPE, Path(__file__), args.ace2016, args.ace2019, *SOURCE_FILES]
    hashes = {str(path): digest(path) for path in paths}
    controls = 0
    observations = []
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        output = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPE, "--inspect", path)
        declaration, *records = output.splitlines()
        assert declaration == "D\tNUMERIC\t39\t28\t0"
        controls += 1
        rows = [record.split("\t") for record in records]
        assert len(rows) == 11
        displays = []
        for ordinal, row in enumerate(rows):
            tag, actual_ordinal, coefficient, scale, encoded = row
            assert tag == "R" and int(actual_ordinal) == ordinal
            if coefficient == "N":
                assert scale == encoded == "N"
                displays.append("NULL")
                continue
            displays.append(decimal_text(coefficient, int(scale)))
            wrapper = bytes.fromhex(encoded)
            assert len(wrapper) == 39 and wrapper[:16] == bytes(16)
            assert wrapper[16:24] == bytes([16, 0, 0, 0, 14, 0, int(scale), 128 if coefficient.startswith("-") else 0])
            assert wrapper[-3:] == bytes(3)
            assert original.count(wrapper) == 1
            controls += 1

        header = "Table\tCalculatedDecimals\nColumn\tAmount\nValues\tStored; not recalculated\nRow ordinal\tStored Decimal value\n"
        expected = header + "".join(f"{ordinal}\t{value}\n" for ordinal, value in enumerate(displays))
        actual = run(args.cli, "inspect", path, "--table", "CalculatedDecimals", "--stored-calculation", "Amount")
        assert actual == expected, (path, actual, expected)
        controls += len(rows)
        empty = run(args.cli, "inspect", path, "--table", "EmptyCalculatedDecimals", "--stored-calculation", "Amount")
        assert empty == header.replace("CalculatedDecimals", "EmptyCalculatedDecimals", 1)
        controls += 1
        expected = "ID\tAmount [cached]\n" + "".join(f"{ordinal + 1}\t{value}\n" for ordinal, value in enumerate(displays))
        assert run(args.cli, "rows", path, "CalculatedDecimals", "--stored-calculations", "--format", "tsv") == expected
        controls += len(rows)
        refused_query = subprocess.run([str(args.cli), "query", "run", "SELECT Amount FROM CalculatedDecimals", "--database", str(path)], capture_output=True)
        assert refused_query.returncode == 2 and not refused_query.stdout
        controls += 1

        # Late malformed caches must suppress the entire buffered inspection.
        # Use only disposable copies and locate original writer bytes, never
        # production parser offsets or a synthetic expected native result.
        wrapper = bytes.fromhex(rows[-2][4])
        start = original.index(wrapper)
        for offset, value in [(0, 1), (16, 15), (20, 13), (22, 29), (23, 1), (38, 1)]:
            changed = bytearray(original)
            changed[start + offset] = value
            with tempfile.TemporaryDirectory(prefix="tablecraft-calculated-decimals-") as directory:
                copy = Path(directory) / "invalid.accdb"
                copy.write_bytes(changed)
                refused(args.cli, copy, "CalculatedDecimals")
                assert copy.read_bytes() == changed
            controls += 1
        observations.append({"file": str(path), "bytes": len(original), "sha256": digest(path), "oracle_sha256": hashlib.sha256(output.encode()).hexdigest(), "rows": len(rows), "declaration": declaration})

    assert all(digest(Path(path)) == expected for path, expected in hashes.items())
    portable_hashes = {}
    for path, value in hashes.items():
        relative = Path(path).resolve().relative_to(ROOT)
        portable_hashes[str(relative)] = value
    report = {
        "date": "2026-10-24",
        "scope": __doc__,
        "controls": controls,
        "hashes_unchanged": True,
        "inputs": portable_hashes,
        "observations": observations,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} cached Decimal controls; all source/input/binary hashes unchanged")


if __name__ == "__main__":
    main()
