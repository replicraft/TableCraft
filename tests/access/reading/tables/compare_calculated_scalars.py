"""Compare inert cached scalar results, preserving NULL/empty and exact carriers."""

from cli_tsv import command_output
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile


def run(*args):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(result.stderr)
    return command_output(args, result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    fields = ("Flag", "Number", "Amount", "Moment", "Label", "Compact")
    types = ("Boolean", "Double", "Currency", "DateTime", "Text", "Text")
    displays = (
        ["TRUE", "FALSE", "NULL", "TRUE", "FALSE"],
        None,
        ["-1.2345", "0.0000", "NULL", "922337203685477.5807", "-922337203685477.5808"],
        ["#2000-01-02 03:04:05.000000065#", "#1899-12-29 06:00:00.000000000#", "NULL", "#2024-02-29 12:00:00.000000000#", "#1900-01-01 00:00:00.000000000#"],
        ['"ASCII"', '""', 'NULL', '"é🦀\\t\\\"\\\\\\n"', '"' + "x" * 40 + '"'],
        ['"ASCII"', '""', 'NULL', '"é🦀\\t\\\"\\\\\\n"', '"' + "x" * 40 + '"'],
    )
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        independent = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            "docs/testing/TableDataOracle.java", path, "CalculatedScalars")
        rows = [line.split("\t") for line in independent.splitlines()]
        if len(rows) != 5 or any(len(row) != 8 for row in rows):
            raise RuntimeError("Unexpected original recipe rows")
        for encoded,expected_text in (("fffe00fffe","efbbbf\n"),("fffe","\n")):
            decoded=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,"docs/testing/CalculatedScalarFixture.java","--decode-text",path,encoded)
            if decoded!=expected_text:raise RuntimeError("Independent leading compression-marker rule differs")
        for position, (field, kind) in enumerate(zip(fields, types)):
            actual = run(args.rows, path, "CalculatedScalars", "--stored-calculation", field)
            expected = "".join("CalculatedScalars\t" + row[position + 2] + "\n" for row in rows)
            if kind == "DateTime":
                actual=run(args.rows,path,"CalculatedScalars","--stored-calculation",field,"--date-bits")
                expected=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,"docs/testing/StoredDateBitsOracle.java",path,"CalculatedScalars",field)
            if actual != expected:
                raise RuntimeError("Independent exact carrier mismatch: " + field)
            header = f"Table\tCalculatedScalars\nColumn\t{field}\nValues\tStored; not recalculated\nRow ordinal\tStored {kind} value\n"
            output = run(args.cli, "inspect", path, "--table", "CalculatedScalars", "--stored-calculation", field)
            if not output.startswith(header) or len(output.splitlines()) != 9:
                raise RuntimeError("Unexpected buffered inspection shape")
            if displays[position] is not None and output != header + "".join(f"{i}\t{value}\n" for i, value in enumerate(displays[position])):
                raise RuntimeError("CLI formatted values differ: " + field)
            if kind == "Double":
                values = [line.split("\t")[1] for line in output.splitlines()[4:]]
                if values[0] != "-0" or float(values[1]) != float.fromhex("0x0.0000000000001p-1022") or values[2] != "NULL" or float(values[3]) != float.fromhex("0x1.fffffffffffffp+1023") or values[4] != "-1.25":
                    raise RuntimeError("Double rendering lost exact finite values")
            empty = run(args.cli, "inspect", path, "--table", "EmptyCalculatedScalars", "--stored-calculation", field)
            if empty != header.replace("CalculatedScalars\n", "EmptyCalculatedScalars\n", 1):
                raise RuntimeError("Empty result declarations differ")
        ordinary = subprocess.run([str(args.cli), "query", "run", "SELECT * FROM CalculatedScalars", "--database", str(path)], capture_output=True)
        if ordinary.returncode != 2 or ordinary.stdout:
            raise RuntimeError("Stored inspection enabled ordinary calculation queries")
        # A later invalid payload must suppress all buffered CLI output. Locate
        # the true wrapper without relying on native parser code or its offsets.
        needle = bytes(16) + (1).to_bytes(4, "little") + bytes([0xff, 0, 0, 0])
        starts = [at for at in range(len(original)) if original.startswith(needle, at)]
        if len(starts) < 2:
            raise RuntimeError("Expected two true Boolean stored wrappers")
        with tempfile.TemporaryDirectory(prefix="tablecraft-calculated-scalars-") as directory:
            copy = Path(directory) / "corrupt.accdb"
            changed = bytearray(original)
            changed[starts[0] + 20] = 1
            copy.write_bytes(changed)
            result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", "CalculatedScalars", "--stored-calculation", "Flag"], capture_output=True)
            if result.returncode != 2 or result.stdout:
                raise RuntimeError("Unknown Boolean wrapper published partial output")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        print(path.name, "stored cells", 30, "file", hashlib.sha256(original).hexdigest(), "rows", hashlib.sha256(independent.encode()).hexdigest())
    print("Six typed stored projections/format, NULL/empty/Boolean distinctions, exact bits, immutable sources and buffered rejection passed; formulas/native Access unrun.")


if __name__ == "__main__":
    main()
