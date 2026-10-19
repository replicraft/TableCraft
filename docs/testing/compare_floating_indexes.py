"""Compare exact IEEE bits/native key order with independent tagged index cursors."""
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
        for table, indexes, rows in (
            ("SingleIndexed", ("FloatAsc", "FloatDesc"), 1000),
            ("DoubleIndexed", ("FloatAsc", "FloatDesc"), 1000),
            ("SingleFinite", ("FloatAsc", "FloatDesc"), 1000),
            ("DoubleFinite", ("FloatAsc", "FloatDesc"), 1000),
            ("FloatingMixed", ("MixedAsc", "MixedDirections"), 1000),
            ("FloatingTen", ("TenMixed",), 500),
            ("SingleUnique", ("UniqueKey",), 20),
            ("DoubleUnique", ("UniqueKey",), 20),
            ("FloatingEmpty", ("EmptyKey",), 0),
        ):
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes")
            roots = {row.split("\t")[0]: int(row.split("\t")[3])
                for row in metadata.splitlines() if row.split("\t")[0] in indexes}
            if set(roots) != set(indexes):
                raise RuntimeError("Missing floating native root metadata")
            # The existing sequential oracle path must retain every original bit too.
            sequential = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, table)
            if run(args.rows, path, table) != sequential:
                raise RuntimeError("Exact sequential IEEE input differs")
            for index in indexes:
                root = roots[index]
                if rows >= 500 and original[root * 4096] != 3:
                    raise RuntimeError("Expected actual floating node/leaf tree")
                if table == "DoubleFinite" and index == "FloatAsc":
                    corrupt_root = root
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/TableDataOracle.java", path, "--index", table, index)
                if len(expected.splitlines()) != rows:
                    raise RuntimeError("Unexpected independent floating cursor count")
                actual = run(args.rows, path, table, "--index", index)
                if actual != expected:
                    raise RuntimeError("Exact IEEE key/reference order differs: " + table + "/" + index)
                normalized += actual
                count += rows
                if table in ("SingleFinite", "DoubleFinite"):
                    ids = [int(row.split("\t")[1].removeprefix("I32:")) for row in expected.splitlines()]
                    sql = "SELECT ID FROM " + table
                    for suffix, selected in (("", ids), (" WHERE ID < 10", [i for i in ids if i < 10])):
                        if run(args.cli, "query", "run", sql + suffix, "--database", path,
                                "--index", table, index) != "ID\n" + "".join(str(i) + "\n" for i in selected):
                            raise RuntimeError("Required floating index query/residual differs")
                    if table in ("SingleFinite", "DoubleFinite"):
                        null_ids = [str(i) + "\n" for i, row in zip(ids, expected.splitlines())
                            if row.split("\t")[2] == "N"]
                        if run(args.cli, "query", "run", sql + " WHERE [Value] IS NULL", "--database", path,
                                "--index", table, index) != "ID\n" + "".join(null_ids):
                            raise RuntimeError("Required floating index NULL filter differs")
                elif rows:
                    # The native bit-preserving reader is broader than query's
                    # existing finite-only source validation. Preserve that gate.
                    failed = subprocess.run([str(args.cli), "query", "run", "SELECT * FROM " + table,
                        "--database", str(path), "--index", table, index], capture_output=True, text=True)
                    if failed.returncode != 2 or failed.stdout or "non-finite" not in failed.stderr:
                        raise RuntimeError("Non-finite query boundary unexpectedly changed")
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                raise RuntimeError("Floating check coverage incomplete")
        with tempfile.TemporaryDirectory(prefix="tablecraft-floating-index-") as directory:
            copy = Path(directory) / "copy.accdb"
            changed = bytearray(original)
            changed[corrupt_root * 4096] = 0
            copy.write_bytes(changed)
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT ID FROM DoubleFinite",
                "--database", str(copy), "--index", "DoubleFinite", "FloatAsc"], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Corrupt required floating index silently fell back")
        if path.read_bytes() != original:
            raise RuntimeError("Original floating fixture changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Exact IEEE/NULL/zero/NaN key trees, complete checks and buffered failures passed; native Access/SQL non-finite conformance unrun.")


if __name__ == "__main__":
    main()
