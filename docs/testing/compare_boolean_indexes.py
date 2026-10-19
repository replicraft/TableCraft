"""Compare native Yes/No keys and mixed tuples with independent Jackcess cursors."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run


def canonical(cursor, columns):
    lines = []
    for line in cursor.splitlines():
        cells = line.split("\t")
        if len(cells) != len(columns):
            raise RuntimeError("Independent Boolean column count differs")
        for i, column in enumerate(columns):
            if column == "Flag" or column == "Other" or column.startswith("K"):
                if cells[i] not in ("true", "false"):
                    raise RuntimeError("Stored Boolean unexpectedly has NULL or another carrier")
                cells[i] = cells[i].upper()
            elif column == "G" and cells[i] != "NULL":
                cells[i] = cells[i].lower()
        lines.append("\t".join(cells) + "\n")
    return "".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        normalized = ""
        count = 0
        roots = []
        for table, indexes, columns, rows in (
            ("BooleanIndexed", ("BoolAsc", "BoolDesc"), ("ID", "Flag"), 1000),
            ("BooleanMixed", ("MixedAsc", "MixedDirections"),
                ("ID", "Flag", "Amount", "G", "Small", "Other"), 1000),
            ("BooleanTen", ("TenMixed",), ("ID", *("K" + str(i) for i in range(10))), 500),
            ("BooleanUnique", ("BoolKey",), ("Flag",), 2),
            ("BooleanPrimary", ("BoolKey",), ("Flag",), 2),
            ("BooleanTupleUnique", ("TupleKey",), ("Flag", "Amount"), 5),
            ("BooleanEmpty", ("EmptyKey",), ("Flag",), 0),
        ):
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes")
            native_roots = {row.split("\t")[0]: int(row.split("\t")[3])
                for row in metadata.splitlines() if row.split("\t")[0] in indexes}
            if set(native_roots) != set(indexes):
                raise RuntimeError("Missing native Boolean index roots")
            for index in indexes:
                root = native_roots[index]
                if rows >= 500 and original[root * 4096] != 3:
                    raise RuntimeError("Expected actual Boolean-bearing node/leaf tree")
                if table == "BooleanIndexed":
                    roots.append(root)
                expected = canonical(run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/IndexRowsOracle.java", path, table, index, *columns), columns)
                if len(expected.splitlines()) != rows:
                    raise RuntimeError("Unexpected independent Boolean cursor count")
                header = "\t".join(columns) + "\n"
                sql = "SELECT " + ",".join("[" + column + "]" for column in columns) + " FROM " + table
                actual = run(args.cli, "query", "run", sql, "--database", path, "--index", table, index)
                if actual != header + expected:
                    raise RuntimeError("Boolean key/row-reference order differs: " + table + "/" + index)
                normalized += actual
                count += rows
                if "Flag" in columns:
                    slot = columns.index("Flag")
                    for value in ("TRUE", "FALSE"):
                        selected = "".join(line + "\n" for line in expected.splitlines()
                            if line.split("\t")[slot] == value)
                        if run(args.cli, "query", "run", sql + " WHERE Flag = " + value,
                                "--database", path, "--index", table, index) != header + selected:
                            raise RuntimeError("Required-index Boolean residual filter differs")
                    if run(args.cli, "query", "run", sql + " WHERE Flag IS NULL", "--database", path,
                            "--index", table, index) != header:
                        raise RuntimeError("Stored Boolean unexpectedly satisfies IS NULL")
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                raise RuntimeError("Boolean index check coverage incomplete")
        with tempfile.TemporaryDirectory(prefix="tablecraft-boolean-index-") as directory:
            copy = Path(directory) / "copy.accdb"
            changed = bytearray(original)
            changed[roots[0] * 4096] = 0
            copy.write_bytes(changed)
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT ID FROM BooleanIndexed",
                "--database", str(copy), "--index", "BooleanIndexed", "BoolAsc"], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Corrupt required Boolean index silently fell back")
        if path.read_bytes() != original:
            raise RuntimeError("Original Boolean fixture changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Boolean complete/mixed trees, residual filters, no-NULL constraints and buffered failures passed; native Access unrun.")


if __name__ == "__main__":
    main()
