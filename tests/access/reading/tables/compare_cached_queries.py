"""Compare opted-in cached queries with original independent stored-value getters.

This checks stored read/query composition, not recalculation or Access SQL.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import display
from compare_calculated_decimals import decimal_text, digest, run, JACKCESS_SHA256

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
SOURCE_PATHS = [
    "crates/engines/tablecraft-ace/src/table/calculated/projection.rs",
    "crates/engines/tablecraft-ace/src/table/page/cursor.rs",
    "crates/engines/tablecraft-ace/src/table/features/mod.rs",
    "crates/backends/tablecraft-access/src/source.rs",
    *[f"crates/backends/tablecraft-access/src/source/{name}.rs" for name in
      ("schema", "snapshot", "projection", "selection", "calculation")],
    "apps/tablecraft-cli/src/commands/query/mod.rs",
    "apps/tablecraft-cli/src/commands/query/run_args.rs",
    "apps/tablecraft-cli/src/commands/query/retrieval.rs",
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "decimal_directory", "memo_directory", "report"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite an evidence report"
    assert digest(args.jar) == JACKCESS_SHA256
    inputs = [args.cli, args.jar, Path(__file__), RECIPES / "CalculatedDecimalFixture.java",
              RECIPES / "CalculatedMemoFixture.java", *[ROOT / name for name in SOURCE_PATHS]]
    files = [(directory / (profile + ".accdb"), kind) for directory, kind in
             [(args.decimal_directory, "decimal"), (args.memo_directory, "memo")]
             for profile in ("V2016", "V2019")]
    inputs.extend(path for path, _ in files)
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in inputs}
    controls = 0
    observations = []

    def query(path, sql):
        return run(args.cli, "query", "run", sql, "--database", path,
                   "--stored-calculations", "--format", "tsv")

    def refused(path, sql, cached=True):
        arguments = [str(args.cli), "query", "run", sql, "--database", str(path)]
        if cached:
            arguments.append("--stored-calculations")
        result = subprocess.run(arguments, capture_output=True, timeout=120)
        assert result.returncode == 2 and not result.stdout, result.stderr

    for path, kind in files:
        original = path.read_bytes()
        recipe = RECIPES / ("CalculatedDecimalFixture.java" if kind == "decimal"
                            else "CalculatedMemoFixture.java")
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, recipe, "--inspect", path)
        if kind == "decimal":
            _, *records = oracle.splitlines()
            records = [line.split("\t") for line in records]
            cells = ["NULL" if row[2] == "N" else decimal_text(row[2], int(row[3]))
                     for row in records]
            table, field = "CalculatedDecimals", "Amount"
            expected = "ID\tAmount\n" + "".join(f"{i+1}\t{value}\n" for i, value in enumerate(cells))
            assert query(path, f"SELECT ID,Amount FROM {table} ORDER BY ID") == expected
            controls += len(cells)
            assert query(path, "SELECT Amount FROM EmptyCalculatedDecimals") == "Amount\n"
            controls += 1
            wrapper = bytes.fromhex(records[-2][4])
        else:
            rows = {"Wide": [], "Compact": []}
            for line in oracle.splitlines()[2:]:
                tag, ordinal, name, value = line.split("\t")
                assert tag == "R" and int(ordinal) == len(rows[name])
                rows[name].append(value)
            cells = [display(value) for value in rows["Wide"]]
            table, field = "CalculatedMemos", "Wide"
            expected = "ID\tWide\tCompact\n" + "".join(
                f"{i+1}\t{display(wide)}\t{display(compact)}\n"
                for i, (wide, compact) in enumerate(zip(rows["Wide"], rows["Compact"])))
            assert query(path, f"SELECT ID,Wide,Compact FROM {table} ORDER BY ID") == expected
            controls += 2 * len(cells)
            lengths = ["NULL" if value == "N" else str(len(bytes.fromhex(value[1:]).decode().encode("utf-16le")) // 2)
                       for value in rows["Wide"]]
            assert query(path, f"SELECT Len(Wide) AS N FROM {table} ORDER BY ID") == "N\n" + "".join(value + "\n" for value in lengths)
            controls += len(cells)
            assert query(path, "SELECT Wide FROM EmptyCalculatedMemos") == "Wide\n"
            controls += 1
            payload = "late-cache".encode("utf-16le")
            wrapper = bytes(16) + len(payload).to_bytes(4, "little") + payload + bytes(3)

        nonnull = sum(value != "NULL" for value in cells)
        assert query(path, f"SELECT Count({field}) AS N FROM {table}") == f"N\n{nonnull}\n"
        expected_nulls = "ID\n" + "".join(f"{i+1}\n" for i, value in enumerate(cells) if value == "NULL")
        assert query(path, f"SELECT ID FROM {table} WHERE {field} IS NULL ORDER BY ID") == expected_nulls
        refused(path, f"SELECT {field} FROM {table}", cached=False)
        controls += 3

        changed = bytearray(original)
        changed[original.index(wrapper)] = 1
        with tempfile.TemporaryDirectory(prefix="tablecraft-cached-query-") as directory:
            copy = Path(directory) / "invalid.accdb"
            copy.write_bytes(changed)
            for sql in [f"SELECT TOP 1 {field} FROM {table}",
                        f"SELECT {field} FROM {table} WHERE ID=-1"]:
                refused(copy, sql)
                controls += 1
            assert query(copy, f"SELECT Count(*) AS N FROM {table}") == f"N\n{len(cells)}\n"
            controls += 1
            assert copy.read_bytes() == changed
        observations.append({"file": str(path.resolve().relative_to(ROOT)),
                             "oracle_sha256": hashlib.sha256(oracle.encode()).hexdigest(),
                             "rows": len(cells), "kind": kind})
    assert all(digest(ROOT / name) == value for name, value in hashes.items())
    report = {"date": "2026-10-24", "scope": __doc__, "controls": controls,
              "hashes_unchanged": True, "inputs": hashes, "observations": observations}
    with args.report.open("x") as output:
        output.write(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} cached query/getter controls; all input/source/binary hashes unchanged")


if __name__ == "__main__":
    main()
