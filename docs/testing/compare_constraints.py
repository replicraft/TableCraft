"""Compare native index declarations, checks and all rows against Jackcess."""

from cli_tsv import display_tsv
import argparse
import csv
import hashlib
import io
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "example", "jar", "ace", "jet"]:
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for format, path in [("ace", args.ace), ("jet4", args.jet)]:
        declarations = subprocess.check_output([
            "java", "-cp", str(args.jar), "docs/testing/ConstraintFixture.java", "--inspect", str(path),
        ], text=True)
        for line in declarations.splitlines():
            table, *expected, rows = line.split("\t")
            output = display_tsv(subprocess.check_output([str(args.cli), "inspect", str(path), "--table", table, "--indexes"], text=True))
            records = list(csv.DictReader(io.StringIO(output.split("\n", 1)[1]), delimiter="\t"))
            actual = [records[0][key].lower() for key in ["Primary", "Unique", "Required key", "Ignore NULLs"]]
            if len(records) != 1 or actual != expected:
                raise RuntimeError(f"{format}: declaration disagreement")
            checked = display_tsv(subprocess.check_output([str(args.cli), "inspect", str(path), "--table", table, "--check"], text=True))
            if f"Rows checked\t{rows}\n" not in checked or "Key\tChecked\tChecked: exact unique/required\n" not in checked:
                raise RuntimeError(f"{format}: incomplete constraint check")
            expected_rows = subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar), "docs/testing/TableDataOracle.java", str(path), table,
            ])
            actual_rows = subprocess.check_output([str(args.example), str(path), table])
            if actual_rows != expected_rows or len(actual_rows.splitlines()) != int(rows):
                raise RuntimeError(f"{format}: row disagreement")
            print(format, table, rows, hashlib.sha256(actual_rows).hexdigest(), flush=True)


if __name__ == "__main__":
    main()
