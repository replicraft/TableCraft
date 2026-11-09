"""Compare complete mixed/all-GUID tuple indexes with independent native cursors."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run
from compare_composite_indexes import geometry


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        normalized = ""
        count = 0
        mixed_roots = []
        for table, indexes, columns, guid_columns, rows in (
            ("GuidMixedIndexed", ("FiveAsc", "FiveDesc", "MixedDirections"),
                ("ID", "G1", "Amount", "Large", "G2", "Narrow"), ("G1", "G2"), 1000),
            ("GuidTenIndexed", ("TenAsc", "TenDesc", "TenMixed"),
                ("ID", *("K" + str(i) for i in range(10))), tuple("K" + str(i) for i in range(10)), 500),
            ("GuidTupleUnique", ("UniqueTuple",), ("G1", "Amount", "G2"), ("G1", "G2"), 9),
            ("GuidTuplePrimary", ("PrimaryTuple",), ("G1", "Amount", "G2"), ("G1", "G2"), 3),
            ("EmptyGuidTuple", ("EmptyTuple",), ("G", "N"), ("G",), 0),
        ):
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes")
            roots = {row.split("\t")[0]: int(row.split("\t")[3]) for row in metadata.splitlines()
                if row.split("\t")[0] in indexes}
            if set(roots) != set(indexes):
                raise RuntimeError("Missing GUID tuple native root metadata")
            if rows >= 500 and any(original[root * 4096] != 3 for root in roots.values()):
                raise RuntimeError("Expected actual GUID tuple node/leaf trees")
            if table == "GuidMixedIndexed":
                mixed_roots = list(roots.values())
            if table == "GuidTenIndexed":
                root = roots["TenAsc"] * 4096
                owner = int.from_bytes(original[root + 4:root + 8], "little")
                counts, largest, prefix_max = geometry(original, owner)
                if largest != {3: 198, 4: 194}:
                    raise RuntimeError("Maximum-width GUID tuple geometry is missing")
                print(path.name, "ten-GUID node/leaf pages", counts[3], counts[4],
                    "largest entries", largest[3], largest[4], "largest prefix", prefix_max)
            for index in indexes:
                cursor = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/IndexRowsOracle.java", path, table, index, *columns)
                lines = []
                for row in cursor.splitlines():
                    cells = row.split("\t")
                    for column in guid_columns:
                        at = columns.index(column)
                        if cells[at] != "NULL":
                            cells[at] = cells[at].lower()
                    lines.append("\t".join(cells))
                if len(lines) != rows:
                    raise RuntimeError("Unexpected independent GUID tuple cursor count")
                header = "\t".join(columns) + "\n"
                sql = "SELECT " + ",".join("[" + column + "]" for column in columns) + " FROM " + table
                expected = header + "".join(row + "\n" for row in lines)
                actual = run(args.cli, "query", "run", sql, "--database", path, "--index", table, index)
                if actual != expected:
                    raise RuntimeError("Mixed GUID/numeric tuple key-reference order differs")
                null_at = columns.index(guid_columns[0])
                selected = "".join(row + "\n" for row in lines if row.split("\t")[null_at] == "NULL")
                if run(args.cli, "query", "run", sql + " WHERE [" + guid_columns[0] + "] IS NULL",
                        "--database", path, "--index", table, index) != header + selected:
                    raise RuntimeError("GUID tuple required-index NULL filtering differs")
                normalized += actual
                count += rows
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                raise RuntimeError("GUID tuple complete check coverage incomplete")
        with tempfile.TemporaryDirectory(prefix="tablecraft-guid-tuple-") as directory:
            copy = Path(directory) / "copy.accdb"
            changed = bytearray(original)
            changed[mixed_roots[0] * 4096] = 0
            copy.write_bytes(changed)
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT ID FROM GuidMixedIndexed",
                "--database", str(copy), "--index", "GuidMixedIndexed", "FiveAsc"], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Corrupt required GUID tuple index silently fell back")
        failed = subprocess.run([str(args.cli), "query", "run", "SELECT * FROM GuidTextTuple",
            "--database", str(path), "--index", "GuidTextTuple", "GuidText"], capture_output=True, text=True)
        if failed.returncode != 2 or failed.stdout:
            raise RuntimeError("Unqualified GUID/Text tuple silently fell back")
        if path.read_bytes() != original:
            raise RuntimeError("Original GUID tuple input changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("GUID/numeric compound trees, maximum-width keys, NULL/unique/primary checks and failure buffering passed; native Access unrun.")


if __name__ == "__main__":
    main()
