"""Compare a fresh Jet 4 Int32 edit while retaining legacy Decimal indexes."""
import argparse
import hashlib
from pathlib import Path
import tempfile

from compare_composite_constraints import run


def changed_rows(text):
    rows = []
    count = 0
    for line in text.splitlines():
        cells = line.split("\t")
        if cells[1] == "I32:1":
            cells[1] = "I32:2001"
            count += 1
        rows.append("\t".join(cells) + "\n")
    if count != 1:
        raise RuntimeError("Expected exactly one original ID")
    return "".join(rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "database"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    original = args.database.read_bytes()
    if original[4:21] != b"Standard Jet DB\0\1":
        raise RuntimeError("Expected original Jet 4 fixture")
    with tempfile.TemporaryDirectory(prefix="tablecraft-jet4-decimal-edit-") as directory:
        copy = Path(directory) / "edited.mdb"
        result = run(args.cli, "query", "update",
            "UPDATE Decimal4 SET ID = ID + 2000 WHERE ID = 1",
            "--database", args.database, "--output", copy)
        if result != "Matched rows\t1\nChanged rows\t1\n":
            raise RuntimeError("Unexpected update result")
        after = copy.read_bytes()
        differences = [i for i, (a, b) in enumerate(zip(original, after)) if a != b]
        if len(original) != len(after) or not differences:
            raise RuntimeError("Image length changed or edit missing")
        start = differences[0]
        if (differences[-1] >= start + 4 or original[start:start + 4] != (1).to_bytes(4, "little")
                or after[start:start + 4] != (2001).to_bytes(4, "little")):
            raise RuntimeError("Bytes outside the exact Int32 replacement changed")
        canonical = ""
        for index in (None, "DecimalAsc", "DecimalDesc"):
            suffix = [] if index is None else ["--index", index]
            oracle_suffix = ["Decimal4"] if index is None else ["--index", "Decimal4", index]
            before = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", args.database, *oracle_suffix)
            expected = changed_rows(before)
            oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", copy, *oracle_suffix)
            actual = run(args.rows, copy, "Decimal4", *suffix)
            if actual != expected or oracle != expected:
                raise RuntimeError("Edited exact carriers/index order differ")
            canonical += actual
        report = run(args.cli, "inspect", copy, "--table", "Decimal4", "--check")
        if "unsupported" in report.lower() or "checked" not in report.lower():
            raise RuntimeError("Edited index checks incomplete")
        print("Edited image", hashlib.sha256(after).hexdigest(),
            "rows", hashlib.sha256(canonical.encode()).hexdigest())
    if args.database.read_bytes() != original:
        raise RuntimeError("Original input changed")
    print("Jet 4 offline copy, exact field span and legacy index preservation passed; native Access unrun.")


if __name__ == "__main__":
    main()
