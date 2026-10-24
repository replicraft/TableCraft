"""Compare bounded validation coverage with the original Jackcess rule recipe."""

import argparse
import hashlib
from pathlib import Path
import subprocess

from cli_tsv import display_tsv

RECIPE = Path(__file__).with_name("ValidationFixture.java")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
FIELD_CASES = {
    "Ranges": ("Score", 3, "Checked"),
    "Nullable": ("Score", 2, "Checked"),
    "Calls": ("Label", 1, "Checked"),
    "EmptyCalls": ("Label", 0, "Checked"),
    "UnknownNull": ("Score", 1, "Not checked: unsupported rule"),
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inspect_constraints(cli, path, table):
    output = subprocess.check_output(
        [str(cli), "inspect", str(path), "--table", table, "--constraints"],
        text=True,
        encoding="utf-8",
    )
    return display_tsv(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ["cli", "jar", "ace", "jet"]:
        parser.add_argument("--" + argument, required=True, type=Path)
    parser.add_argument("--ace-label", default="ace")
    parser.add_argument("--jet-label", default="jet4")
    args = parser.parse_args()

    if digest(args.jar) != JAR_SHA256:
        raise RuntimeError("Unreviewed Jackcess artifact")

    for label, path in [(args.ace_label, args.ace), (args.jet_label, args.jet)]:
        before = digest(path)
        oracle = subprocess.check_output(
            [
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPE), "--inspect", str(path),
            ],
            text=True,
            encoding="utf-8",
        )
        expected_rows = dict(line.split("\t") for line in oracle.splitlines())

        for table, (column, count, coverage) in FIELD_CASES.items():
            output = inspect_constraints(args.cli, path, table)
            lines = [line.split("\t") for line in output.splitlines()]
            record = next(row for row in lines if row[0] == column)
            if (
                record[-1] != coverage
                or f"Rows checked\t{count}\n" not in output
                or int(expected_rows[table]) != count
            ):
                raise RuntimeError(f"{label}: {table} field rule disagreement")
            print(label, table, count, coverage, flush=True)

        output = inspect_constraints(args.cli, path, "Bounds")
        if (
            "Table validation rule\tChecked\n" not in output
            or "Rows checked\t2\n" not in output
            or expected_rows["Bounds"] != "2"
        ):
            raise RuntimeError(f"{label}: table rule disagreement")
        if digest(path) != before:
            raise RuntimeError(f"{label}: input changed")
        print(label, "Bounds", 2, "Checked", before, flush=True)


if __name__ == "__main__":
    main()
