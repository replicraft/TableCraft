"""Compare exact mixed/ten-Decimal tuples with independent native index cursors."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run
from compare_composite_indexes import geometry


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        normalized = ""
        count = 0
        corrupt_root = None
        for table, indexes, rows in (
            ("DecimalMixed", ("MixedAsc", "MixedDirections"), 1000),
            ("DecimalTen", ("TenAsc", "TenDesc", "TenMixed"), 500),
            ("DecimalTupleUnique", ("UniqueTuple",), 9),
            ("DecimalTuplePrimary", ("PrimaryTuple",), 3),
            ("DecimalTupleEmpty", ("EmptyTuple",), 0),
        ):
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes")
            roots = {row.split("\t")[0]: int(row.split("\t")[3])
                for row in metadata.splitlines() if row.split("\t")[0] in indexes}
            if set(roots) != set(indexes):
                raise RuntimeError("Missing native Decimal tuple roots")
            sequential = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, "--date-bits", table)
            if run(args.rows, path, table, "--date-bits") != sequential:
                raise RuntimeError("Exact mixed Decimal sequential carriers differ")
            for index in indexes:
                root = roots[index]
                if rows >= 500 and original[root * 4096] != 3:
                    raise RuntimeError("Expected actual Decimal tuple node/leaf trees")
                if table == "DecimalMixed" and index == "MixedAsc":
                    corrupt_root = root
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/TableDataOracle.java", path, "--date-bits", "--index", table, index)
                if len(expected.splitlines()) != rows:
                    raise RuntimeError("Unexpected independent Decimal tuple row count")
                actual = run(args.rows, path, table, "--date-bits", "--index", index)
                if actual != expected:
                    raise RuntimeError("Exact Decimal tuple order/reference/carrier differs: " + table + "/" + index)
                normalized += actual
                count += rows
                if rows >= 500:
                    ids = [int(row.split("\t")[1].removeprefix("I32:")) for row in expected.splitlines()]
                    sql = "SELECT ID FROM " + table
                    for suffix, selected in (("", ids), (" WHERE ID < 10", [i for i in ids if i < 10])):
                        if run(args.cli, "query", "run", sql + suffix, "--database", path,
                                "--index", table, index) != "ID\n" + "".join(str(i) + "\n" for i in selected):
                            raise RuntimeError("Required Decimal tuple residual query differs")
                    if table == "DecimalMixed":
                        null_ids = [i for i, row in zip(ids, expected.splitlines()) if row.split("\t")[2] == "N"]
                        if run(args.cli, "query", "run", sql + " WHERE A IS NULL", "--database", path,
                                "--index", table, index) != "ID\n" + "".join(str(i) + "\n" for i in null_ids):
                            raise RuntimeError("Decimal tuple NULL residual differs")
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                raise RuntimeError("Decimal tuple check coverage incomplete")
            if table == "DecimalTen":
                root = roots[indexes[0]]
                owner = int.from_bytes(original[root * 4096 + 4:root * 4096 + 8], "little")
                counts, largest, prefix = geometry(original, owner)
                if largest != {3: 188, 4: 184}:
                    raise RuntimeError("Maximum ten-Decimal entry widths missing")
                print(path.name, "ten-Decimal node/leaf pages", counts[3], counts[4],
                    "largest entries", largest[3], largest[4], "largest prefix", prefix)
        with tempfile.TemporaryDirectory(prefix="tablecraft-decimal-tuple-") as directory:
            copy = Path(directory) / "copy.accdb"
            changed = bytearray(original);changed[corrupt_root * 4096] = 0;copy.write_bytes(changed)
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT ID FROM DecimalMixed",
                "--database", str(copy), "--index", "DecimalMixed", "MixedAsc"], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Corrupt required Decimal tuple root silently fell back")
        failed = subprocess.run([str(args.cli), "query", "run", "SELECT * FROM DecimalTextTuple",
            "--database", str(path), "--index", "DecimalTextTuple", "TextTuple"], capture_output=True, text=True)
        if failed.returncode != 2 or failed.stdout:
            raise RuntimeError("Unqualified Decimal/Text tuple silently fell back")
        if path.read_bytes() != original:
            raise RuntimeError("Original Decimal tuple input changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Exact mixed/ten-Decimal trees, field scales, NULL/unique/primary checks and buffered failures passed; native Access unrun.")


if __name__ == "__main__":
    main()
