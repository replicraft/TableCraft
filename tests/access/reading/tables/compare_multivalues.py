"""Compare bounded scalar collections through independent multivalue APIs."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run
from compare_complex_links import column_layout


def rejected(rows, path, table, column):
    result = subprocess.run([str(rows), str(path), table, "--multivalues", column],
        capture_output=True, text=True)
    if result.returncode == 0 or result.stdout:
        raise RuntimeError("Unqualified/corrupt collection published output: " + column)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "ace2016", "ace2019", "complex2016", "complex2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--ace2007", type=Path)
    parser.add_argument("--complex2007", type=Path)
    args = parser.parse_args()
    if bool(args.ace2007) != bool(args.complex2007):
        parser.error("--ace2007 and --complex2007 must be supplied together")
    columns = ("Bytes", "Shorts", "Ints", "Singles", "Doubles", "Guids", "Decimals", "Texts")
    cases = [(args.ace2016, args.complex2016, 5), (args.ace2019, args.complex2019, 6)]
    if args.ace2007:
        cases.append((args.ace2007, args.complex2007, 2))
    for path, complex_path, code in cases:
        original = path.read_bytes()
        old_complex = complex_path.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Unexpected ACE profile")
        catalog = run(args.cli, "inspect", path, "--catalog", "--show-system").splitlines()
        start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        fields = catalog[start].split("\t")
        ids = {cells[fields.index("Name")]: int(cells[fields.index("Native ID")])
            for cells in (line.split("\t") for line in catalog[start + 1:])}
        declarations = run(args.cli, "inspect", path, "--table", "Multivalues", "--complex").splitlines()[2:]
        child_tables = {row.split("\t")[0]: row.split("\t")[3:] for row in declarations}
        normalized = ""
        values = 0
        cases = []
        for column in columns:
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/MultivalueOracle.java", path, "Multivalues", column)
            actual = run(args.rows, path, "Multivalues", "--multivalues", column)
            lines = [line.split("\t") for line in actual.splitlines()]
            if actual != expected or len(lines) != 4:
                raise RuntimeError("Exact multivalue collection differs: " + column)
            if lines[1][3:] != ["N"] or lines[2][3:]:
                raise RuntimeError("NULL stored value was confused with empty collection")
            if column == "Ints" and len(lines[3][3:]) != 100:
                raise RuntimeError("100-value boundary not exercised")
            if column == "Texts" and lines[0][3:].count("T526564") != 2:
                raise RuntimeError("Stored duplicate values were collapsed")
            if column == "Singles" and "F7fc01234" not in lines[0]:
                raise RuntimeError("Single payload bits were lost")
            if column == "Doubles" and "D7ff8000000001234" not in lines[0]:
                raise RuntimeError("Double payload bits were lost")
            values += sum(len(line) - 3 for line in lines)
            normalized += "".join(column + "\t" + line + "\n" for line in actual.splitlines())
            flat, foreign, _, _ = child_tables[column]
            slots = roots(original, ids[flat])
            offset, _, _ = column_layout(original, ids[flat], foreign)
            changed = bytearray(original)
            changed[slots[0][0] + 2 + offset:slots[0][0] + 6 + offset] = (999).to_bytes(4, "little")
            cases.append((column, changed))
        template = ids["MSysComplexType_Decimal"] * 4096
        indexes = int.from_bytes(original[template + 51:template + 55], "little")
        field = template + 63 + indexes * 12
        if original[field] != 16 or original[field + 23:field + 25] != bytes([9, 0]):
            raise RuntimeError("Expected conceptual Decimal width hint missing")
        for offset, value in ((field + 11, 17), (field + 12, 1), (field + 23, 8), (template + 16, 1)):
            changed = bytearray(original)
            changed[offset] = value
            cases.append(("Decimals", changed))
        with tempfile.TemporaryDirectory(prefix="tablecraft-multivalues-") as directory:
            copy = Path(directory) / "copy.accdb"
            for column, changed in cases:
                copy.write_bytes(changed)
                rejected(args.rows, copy, "Multivalues", column)
        rejected(args.rows, path, "Overflow", "Items")
        if len(run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/MultivalueOracle.java", path, "Overflow", "Items").strip().split("\t")) != 104:
            raise RuntimeError("Independent 101-item overflow control missing")
        if "\t101\n" not in run(args.cli, "inspect", path, "--table", "Overflow", "--complex-links", "Items"):
            raise RuntimeError("Raw links unexpectedly inherited the typed collection cap")
        expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            "docs/testing/MultivalueOracle.java", complex_path, "Collections", "Choices")
        if run(args.rows, complex_path, "Collections", "--multivalues", "Choices") != expected:
            raise RuntimeError("Existing original Text choices changed")
        declarations = run(args.cli, "inspect", complex_path, "--table", "Collections", "--complex").splitlines()[2:]
        history = [row.split("\t")[0] for row in declarations if row.split("\t")[0] not in ("Choices", "Files")]
        if len(history) != 1:
            raise RuntimeError("Expected one resolved hidden history column")
        for column in ("Files", "Notes", history[0]):
            rejected(args.rows, complex_path, "Collections", column)
        if path.read_bytes() != original or complex_path.read_bytes() != old_complex:
            raise RuntimeError("Original multivalue/complex input changed")
        print(path.name, "parent collections", 32, "stored values", values, "corrupt copies", len(cases),
            "file", hashlib.sha256(original).hexdigest(), "collections", hashlib.sha256(normalized.encode()).hexdigest())
    print("Exact typed collections, empty/NULL/duplicate distinctions, conceptual Decimal metadata and buffered failures passed; native Access, SQL expansion and mutation unrun.")


if __name__ == "__main__":
    main()
