"""Compare native Byte/Int16 index order and constraints with Jackcess cursors."""
import argparse
import hashlib
from pathlib import Path
import subprocess


def run(*args):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        normalized = ""
        count = 0
        for label in ("Byte", "Short"):
            for table, index, columns, rows in (
                (label + "Indexed", "KeyAsc", ("ID", "Value"), 700),
                (label + "Indexed", "KeyDesc", ("ID", "Value"), 700),
                (label + "Unique", "UniqueKey", ("Value",), 258 if label == "Byte" else 9),
                (label + "Empty", "EmptyKey", ("ID", "Value"), 0),
            ):
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/IndexRowsOracle.java", path, table, index, *columns)
                if len(expected.splitlines()) != rows:
                    raise RuntimeError("Unexpected independent cursor count")
                header = "\t".join(columns) + "\n"
                def query(where=""):
                    return run(args.cli, "query", "run", "SELECT " + ",".join("[" + column + "]" for column in columns) + " FROM " + table + " " + where,
                        "--database", path, "--index", table, index)
                actual = query()
                if actual != header + expected:
                    raise RuntimeError("Narrow native key/reference order differs")
                for where, accepts in (
                    ("WHERE [Value] IS NULL", lambda text: text == "NULL"),
                    ("WHERE [Value] >= 128", lambda text: text != "NULL" and int(text) >= 128),
                    ("WHERE [Value] < 0", lambda text: text != "NULL" and int(text) < 0),
                ):
                    filtered = "".join(row + "\n" for row in expected.splitlines() if accepts(row.split("\t")[-1]))
                    if query(where) != header + filtered:
                        raise RuntimeError("Narrow carrier SQL filtering differs")
                coverage = run(args.cli, "inspect", path, "--table", table, "--check")
                if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                    raise RuntimeError("Narrow index coverage is not complete")
                normalized += actual
                count += rows
            metadata = run(args.cli, "inspect", path, "--table", label + "Indexed", "--indexes")
            roots = [int(row.split("\t")[3]) for row in metadata.splitlines() if row.split("\t")[0] in ("KeyAsc", "KeyDesc")]
            if len(roots) != 2 or any(original[root * 4096] != 3 for root in roots):
                raise RuntimeError("Expected actual narrow node/leaf trees")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(), "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Byte unsigned/Int16 signed indexes, real trees, NULL/duplicate/unique coverage and immutable sources passed; native Access unrun.")


if __name__ == "__main__":
    main()
