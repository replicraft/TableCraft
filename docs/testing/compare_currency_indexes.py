"""Compare exact scaled Currency native indexes with independent Jackcess cursors."""
import argparse
from decimal import Decimal
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
        for table, index, columns, rows in (
            ("MoneyIndexed", "MoneyAsc", ("ID", "Amount"), 700),
            ("MoneyIndexed", "MoneyDesc", ("ID", "Amount"), 700),
            ("MoneyUnique", "MoneyUniqueKey", ("Amount",), 14),
            ("MoneyPrimary", "MoneyPrimaryKey", ("Amount",), 12),
            ("MoneyEmpty", "EmptyMoneyKey", ("ID", "Amount"), 0),
        ):
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/IndexRowsOracle.java", path, table, index, *columns)
            if len(expected.splitlines()) != rows:
                raise RuntimeError("Unexpected independent Currency cursor count")
            header = "\t".join(columns) + "\n"

            def query(where=""):
                return run(args.cli, "query", "run", "SELECT " + ",".join("[" + column + "]" for column in columns) + " FROM " + table + " " + where,
                    "--database", path, "--index", table, index)

            actual = query()
            if actual != header + expected:
                raise RuntimeError("Exact Currency native key/reference order differs")
            for where, accepts in (
                ("WHERE [Amount] IS NULL", lambda text: text == "NULL"),
                ("WHERE [Amount] < 0", lambda text: text != "NULL" and Decimal(text) < 0),
                ("WHERE [Amount] >= 0", lambda text: text != "NULL" and Decimal(text) >= 0),
            ):
                filtered = "".join(row + "\n" for row in expected.splitlines()
                    if accepts(row.split("\t")[-1]))
                if query(where) != header + filtered:
                    raise RuntimeError("Currency carrier SQL filtering differs")
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in coverage.lower() or "checked" not in coverage.lower():
                raise RuntimeError("Currency index coverage is incomplete")
            normalized += actual
            count += rows
        metadata = run(args.cli, "inspect", path, "--table", "MoneyIndexed", "--indexes")
        roots = [int(row.split("\t")[3]) for row in metadata.splitlines()
            if row.split("\t")[0] in ("MoneyAsc", "MoneyDesc")]
        if len(roots) != 2 or any(original[root * 4096] != 3 for root in roots):
            raise RuntimeError("Expected actual Currency node/leaf trees")
        if path.read_bytes() != original:
            raise RuntimeError("Original Currency fixture changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Exact Currency indexes, real trees, NULL/duplicate/primary checks and immutable sources passed; native Access unrun.")


if __name__ == "__main__":
    main()
