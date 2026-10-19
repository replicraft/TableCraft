"""Compare complete ACE 16 Decimal trees without narrowing their coefficients."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run


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
        cases = []
        for scale in (0, 4, 28):
            cases += [
                ("Decimal" + str(scale), ("DecimalAsc", "DecimalDesc"), 1000),
                ("DecimalUnique" + str(scale), ("DecimalKey",), 18),
                ("DecimalPrimary" + str(scale), ("DecimalKey",), 16),
                ("DecimalEmpty" + str(scale), ("EmptyKey",), 0),
            ]
        cases.append(("DecimalSmall", ("SmallAsc", "SmallDesc"), 7))
        for table, indexes, rows in cases:
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes")
            roots = {row.split("\t")[0]: int(row.split("\t")[3])
                for row in metadata.splitlines() if row.split("\t")[0] in indexes}
            if set(roots) != set(indexes):
                raise RuntimeError("Missing Decimal root metadata")
            sequential = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, table)
            if run(args.rows, path, table) != sequential:
                raise RuntimeError("Exact sequential Decimal carriers differ")
            for index in indexes:
                root = roots[index]
                if rows >= 500 and original[root * 4096] != 3:
                    raise RuntimeError("Expected actual Decimal node/leaf tree")
                if table == "Decimal4" and index == "DecimalAsc":
                    corrupt_root = root
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/TableDataOracle.java", path, "--index", table, index)
                if len(expected.splitlines()) != rows:
                    raise RuntimeError("Unexpected independent Decimal cursor count")
                actual = run(args.rows, path, table, "--index", index)
                if actual != expected:
                    raise RuntimeError("Exact Decimal key/reference order differs: " + table + "/" + index)
                normalized += actual
                count += rows
                if rows >= 500:
                    ids = [int(row.split("\t")[1].removeprefix("I32:")) for row in expected.splitlines()]
                    nulls = [i for i, row in zip(ids, expected.splitlines()) if row.split("\t")[2] == "N"]
                    sql = "SELECT ID FROM " + table
                    for suffix, selected in (("", ids), (" WHERE ID < 10", [i for i in ids if i < 10]),
                                             (" WHERE [Value] IS NULL", nulls)):
                        if run(args.cli, "query", "run", sql + suffix, "--database", path,
                                "--index", table, index) != "ID\n" + "".join(str(i) + "\n" for i in selected):
                            raise RuntimeError("Required Decimal index query/residual differs")
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                raise RuntimeError("Decimal complete check coverage incomplete")
        with tempfile.TemporaryDirectory(prefix="tablecraft-decimal-index-") as directory:
            copy = Path(directory) / "copy.accdb"
            changed = bytearray(original);changed[corrupt_root * 4096] = 0;copy.write_bytes(changed)
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT ID FROM Decimal4",
                "--database", str(copy), "--index", "Decimal4", "DecimalAsc"], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Corrupt required Decimal root silently fell back")
        if run(args.cli, "query", "run", "SELECT * FROM DecimalCompound", "--database", path,
                "--index", "DecimalCompound", "CompoundKey") != "Value\tID\n":
            raise RuntimeError("Empty qualified Decimal compound differs")
        if "unsupported" in run(args.cli, "inspect", path, "--table", "DecimalCompound", "--check").lower():
            raise RuntimeError("Empty Decimal compound was not checked")
        if path.read_bytes() != original:
            raise RuntimeError("Original Decimal input changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Exact Decimal coefficients/scales, complete trees/constraints and buffered failures passed; native Access unrun.")


if __name__ == "__main__":
    main()
