"""Compare exact DATE index serials separately from residual civil-time ordering."""
import argparse
import hashlib
import math
from pathlib import Path
import struct
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
        for table, indexes, rows in (
            ("DateIndexed", ("DateAsc", "DateDesc"), 1000),
            ("DateMixed", ("MixedAsc", "MixedDirections"), 1000),
            ("DateTen", ("TenMixed",), 500),
            ("DateUnique", ("DateKey",), 15),
            ("DatePrimary", ("DateKey",), 13),
            ("DateEmpty", ("EmptyKey",), 0),
        ):
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes")
            roots = {row.split("\t")[0]: int(row.split("\t")[3])
                for row in metadata.splitlines() if row.split("\t")[0] in indexes}
            if set(roots) != set(indexes):
                raise RuntimeError("Missing native DATE roots")
            sequential = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, "--date-bits", table)
            if run(args.rows, path, table, "--date-bits") != sequential:
                raise RuntimeError("Sequential DATE bits differ")
            for index in indexes:
                root = roots[index]
                if rows >= 500 and original[root * 4096] != 3:
                    raise RuntimeError("Expected actual DATE node tree")
                if table == "DateIndexed" and index == "DateAsc":
                    corrupt_root = root
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/TableDataOracle.java", path, "--date-bits", "--index", table, index)
                if len(expected.splitlines()) != rows:
                    raise RuntimeError("Unexpected independent DATE cursor count")
                actual = run(args.rows, path, table, "--date-bits", "--index", index)
                if actual != expected:
                    raise RuntimeError("Exact DATE key/reference order differs: " + table + "/" + index)
                normalized += actual
                count += rows
                if rows >= 500:
                    ids = [int(row.split("\t")[1].removeprefix("I32:")) for row in expected.splitlines()]
                    sql = "SELECT ID FROM " + table
                    for suffix, selected in (("", ids), (" WHERE ID < 10", [i for i in ids if i < 10])):
                        if run(args.cli, "query", "run", sql + suffix, "--database", path,
                                "--index", table, index) != "ID\n" + "".join(str(i) + "\n" for i in selected):
                            raise RuntimeError("Required DATE index residual query differs")
                    if table == "DateIndexed":
                        values = []
                        for i, row in zip(ids, expected.splitlines()):
                            cell = row.split("\t")[2]
                            value = None if cell == "N" else struct.unpack("<d", int(cell.removeprefix("Y64:"), 16).to_bytes(8, "little"))[0]
                            values.append((i, value))
                        selections = [
                            (" WHERE Moment IS NULL", [i for i, value in values if value is None]),
                            (" WHERE Moment = #1899-12-30 06:00:00#", [i for i, value in values
                                if value is not None and math.trunc(value) == 0 and abs(value) == 0.25]),
                            (" ORDER BY Moment, ID", [i for i, value in sorted(values, key=lambda row:
                                (0, 0, 0, row[0]) if row[1] is None else
                                (1, math.trunc(row[1]), abs(row[1] - math.trunc(row[1])), row[0]))]),
                        ]
                        for suffix, selected in selections:
                            if run(args.cli, "query", "run", sql + suffix, "--database", path,
                                    "--index", table, index) != "ID\n" + "".join(str(i) + "\n" for i in selected):
                                raise RuntimeError("Physical DATE order leaked into civil SQL comparison/sort")
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                raise RuntimeError("DATE index coverage incomplete")
        with tempfile.TemporaryDirectory(prefix="tablecraft-date-index-") as directory:
            copy = Path(directory) / "copy.accdb"
            changed = bytearray(original);changed[corrupt_root * 4096] = 0;copy.write_bytes(changed)
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT ID FROM DateIndexed",
                "--database", str(copy), "--index", "DateIndexed", "DateAsc"], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Corrupt required DATE index silently fell back")
        if path.read_bytes() != original:
            raise RuntimeError("Original DATE index input changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Exact DATE trees/aliases, complete checks, residual civil semantics and buffered failures passed; native Access unrun.")


if __name__ == "__main__":
    main()
