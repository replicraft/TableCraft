"""Compare canonical GUID bytes/native index order with independent Jackcess cursors."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run


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
        for table, index, columns, rows in (
            ("GuidIndexed", "GuidAsc", ("ID", "Value"), 1000),
            ("GuidIndexed", "GuidDesc", ("ID", "Value"), 1000),
            ("GuidUnique", "UniqueGuid", ("Value",), 18),
            ("GuidPrimary", "PrimaryGuid", ("Value",), 16),
            ("GuidEmpty", "EmptyGuid", ("Value",), 0),
        ):
            cursor = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/IndexRowsOracle.java", path, table, index, *columns)
            lines = []
            for line in cursor.splitlines():
                cells = line.split("\t")
                # Compare canonical bytes through each host's explicit GUID
                # spelling; case is not a physical key or SQL collation policy.
                if cells[-1] != "NULL":
                    cells[-1] = cells[-1].lower()
                lines.append("\t".join(cells))
            expected = "".join(line + "\n" for line in lines)
            if len(lines) != rows:
                raise RuntimeError("Unexpected independent GUID cursor count")
            header = "\t".join(columns) + "\n"
            sql = "SELECT " + ",".join("[" + column + "]" for column in columns) + " FROM " + table
            actual = run(args.cli, "query", "run", sql, "--database", path, "--index", table, index)
            if actual != header + expected:
                raise RuntimeError("Canonical GUID/native key-reference order differs")
            selected = "".join(line + "\n" for line in lines if line.split("\t")[-1] == "NULL")
            if run(args.cli, "query", "run", sql + " WHERE [Value] IS NULL", "--database", path,
                    "--index", table, index) != header + selected:
                raise RuntimeError("GUID required-index NULL filtering differs")
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                raise RuntimeError("GUID complete index check coverage incomplete")
            if table == "GuidIndexed":
                metadata = run(args.cli, "inspect", path, "--table", table, "--indexes")
                root = next(int(row.split("\t")[3]) for row in metadata.splitlines()
                    if row.split("\t")[0] == index)
                if original[root * 4096] != 3:
                    raise RuntimeError("Expected actual GUID node/leaf tree")
                roots.append(root)
            normalized += actual
            count += rows
        with tempfile.TemporaryDirectory(prefix="tablecraft-guid-index-") as directory:
            copy = Path(directory) / "copy.accdb"
            changed = bytearray(original)
            changed[roots[0] * 4096] = 0
            copy.write_bytes(changed)
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT ID FROM GuidIndexed",
                "--database", str(copy), "--index", "GuidIndexed", "GuidAsc"], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Corrupt required GUID index silently fell back")
        # This originally unqualified empty compound now shares the explicit
        # GUID-tuple profile; populated/wide cases use their separate runner.
        if run(args.cli, "query", "run", "SELECT * FROM GuidCompound", "--database", path,
                "--index", "GuidCompound", "CompoundGuid") != "Value\tID\n":
            raise RuntimeError("Empty qualified GUID compound differs")
        if "unsupported" in run(args.cli, "inspect", path, "--table", "GuidCompound", "--check").lower():
            raise RuntimeError("Empty GUID compound was not checked")
        if path.read_bytes() != original:
            raise RuntimeError("Original GUID input changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Canonical GUID keys, complete trees, NULL/unique/primary coverage and buffered failures passed; native Access/SQL GUID order unrun.")


if __name__ == "__main__":
    main()
