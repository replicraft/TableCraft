"""Compare exact ACE 16 fixed scalar candidates, including untouched long values."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_attachment_columns import fields
from compare_composite_constraints import roots, run
from compare_saved_queries import catalog_ids


CASES = (
    ("Tiny", "byte", "1", "255", "U1", "U255"),
    ("Narrow", "int16", "-32768", "32767", "I16:-32768", "I16:32767"),
    ("Number", "int32", "-2147483648", "2147483647", "I32:-2147483648", "I32:2147483647"),
    ("Large", "int64", "-9223372036854775808", "9223372036854775807", "I64:-9223372036854775808", "I64:9223372036854775807"),
    ("Amount", "currency-scaled", "-9223372036854775808", "9223372036854775807", "C-9223372036854775808", "C9223372036854775807"),
    ("S", "single-bits", "80000000", "3f800001", "F80000000", "F3f800001"),
    ("D", "double-bits", "8000000000000000", "3ff0000000000001", "D8000000000000000", "D3ff0000000000001"),
    ("Moment", "date-bits", "bff4000000000000", "4004000000000000", "Y64:bff4000000000000", "Y64:4004000000000000"),
    ("G", "guid", "00112233-4455-6677-8899-aabbccddeeff", "ffeeddcc-bbaa-9988-7766-554433221100",
        "G00112233445566778899aabbccddeeff", "Gffeeddccbbaa99887766554433221100"),
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "example", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path, code in ((args.ace2016, 5), (args.ace2019, 6)):
        original = path.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Expected original ACE 16 family")
        def oracle(source):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", source, "--date-bits", "Items")
        before = oracle(path)
        if run(args.rows, path, "Items", "--date-bits") != before:
            raise RuntimeError("Original exact values differ")
        definition = catalog_ids(args, path)["Items"]
        layout = fields(original, definition)
        names = list(layout)
        if names != ["Id", *[case[0] for case in CASES], "Notes", "Payload"]:
            raise RuntimeError("Original column order changed")
        records = roots(original, definition)
        if len(records) != 3 or len(before.splitlines()) != 3:
            raise RuntimeError("Expected three original inline rows")
        canonical = ""
        with tempfile.TemporaryDirectory(prefix="tablecraft-fixed-scalar-") as directory:
            for name, kind, old, new, tag, replacement in CASES:
                at = layout[name][0]
                width = int.from_bytes(original[at + 23:at + 25], "little")
                if not original[at + 15] & 1 or not 1 <= width <= 16:
                    raise RuntimeError("Expected bounded fixed field")
                offset = int.from_bytes(original[at + 21:at + 23], "little")
                start = records[1][0] + 2 + offset
                lines = before.splitlines()
                cells = lines[1].split("\t")
                position = names.index(name) + 1
                if cells[1] != "I32:2" or cells[position] != tag:
                    raise RuntimeError("Unexpected original scalar")
                cells[position] = replacement
                lines[1] = "\t".join(cells)
                expected = "\n".join(lines) + "\n"
                output = Path(directory) / (name + ".accdb")
                command = [str(args.example), str(path), str(output), "Items", name, "1", kind, old, new]
                result = subprocess.run(command, capture_output=True, text=True)
                if (result.returncode or result.stdout != "candidate_changed_rows=1\n"
                        or "qualification pending" not in result.stderr):
                    raise RuntimeError("Fixed scalar candidate failed: " + name)
                after = output.read_bytes()
                if (len(after) != len(original) or after[:start] != original[:start]
                        or after[start + width:] != original[start + width:]):
                    raise RuntimeError("Candidate altered bytes outside its exact field")
                if oracle(output) != expected or run(args.rows, output, "Items", "--date-bits") != expected:
                    raise RuntimeError("Candidate exact carriers/long values differ: " + name)
                report = run(args.cli, "inspect", output, "--table", "Items", "--check")
                if "unsupported" in report.lower() or "checked" not in report.lower():
                    raise RuntimeError("Candidate check coverage incomplete")
                repeated = subprocess.run(command, capture_output=True)
                if repeated.returncode == 0 or repeated.stdout or output.read_bytes() != after:
                    raise RuntimeError("Existing candidate was overwritten")
                canonical += expected
                print(path.name, name, "candidate", hashlib.sha256(after).hexdigest())
            rejected = Path(directory) / "wrong-carrier.accdb"
            result = subprocess.run([str(args.example), str(path), str(rejected), "Items", "Large", "1",
                "int32", "0", "1"], capture_output=True)
            if result.returncode == 0 or result.stdout or rejected.exists():
                raise RuntimeError("Wrong-width carrier created output")
        if path.read_bytes() != original:
            raise RuntimeError("Original input changed")
        print(path.name, "original", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(canonical.encode()).hexdigest())
    print("Nine exact scalar carriers, untouched long values and field-span proofs pass; native Access/macros unqualified.")


if __name__ == "__main__":
    main()
