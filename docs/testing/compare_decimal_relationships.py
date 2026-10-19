"""Compare exact Decimal foreign keys and reject corrupted disposable copies."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run


def coefficient_bytes(value):
    magnitude = abs(value).to_bytes(16, "big")
    return bytes([0x80 if value < 0 else 0]) + b"".join(
        magnitude[i:i + 4][::-1] for i in range(0, 16, 4))


def variable_start(data, slot, position, columns):
    start, end = slot
    mask_bytes = (columns + 7) // 8
    count = int.from_bytes(data[end - mask_bytes - 2:end - mask_bytes], "little")
    if position >= count:
        raise RuntimeError("Missing native variable column")
    at = end - mask_bytes - 2 - (position + 1) * 2
    return start + int.from_bytes(data[at:at + 2], "little")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--jet4", type=Path)
    parser.add_argument("--ace2007", type=Path)
    args = parser.parse_args()
    for path, code in ((args.ace2016, 5), (args.ace2019, 6), (args.jet4, 1), (args.ace2007, 2)):
        if path is None:
            continue
        original = path.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Unexpected ACE profile")
        catalog = run(args.cli, "inspect", path, "--catalog").splitlines()
        start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        fields = catalog[start].split("\t")
        ids = {cells[fields.index("Name")]: int(cells[fields.index("Native ID")])
            for cells in (line.split("\t") for line in catalog[start + 1:])}
        tagged = ""
        row_count = 0
        cases = []
        for prefix, count in (("Decimal0", 14), ("Decimal4", 14), ("Decimal28", 14), ("DecimalMixed", 8)):
            for suffix, rows, relation in (("Parents", count, "Keys"), ("Children", count, "Keys"),
                    ("UniqueChildren", 4, "UniqueKeys")):
                table = prefix + suffix
                actual = run(args.rows, path, table)
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/TableDataOracle.java", path, table)
                if actual != expected or len(actual.splitlines()) != rows:
                    raise RuntimeError("Exact Decimal/integer carriers differ: " + table)
                output = run(args.cli, "inspect", path, "--table", table, "--constraints")
                if (f"Rows checked\t{rows}\n" not in output
                        or f"{prefix + relation}\tChecked: exact key membership\n" not in output
                        or "Not checked" in output):
                    raise RuntimeError("Decimal key membership coverage incomplete: " + table)
                tagged += actual
                row_count += rows
            child_name = prefix + "Children"
            child = roots(original, ids[child_name])
            changed = bytearray(original)
            # Replace an existing non-NULL variable payload without changing row geometry.
            at = variable_start(original, child[0], 0, 3 if prefix == "DecimalMixed" else 1)
            changed[at:at + 17] = coefficient_bytes(2)
            cases.append((child_name, changed, "foreign key has no referenced record"))
            for suffix, error in (("Parents", "referenced key is not unique"),
                    ("UniqueChildren", "one-to-one relationship has duplicate dependent keys")):
                table = prefix + suffix
                slots = roots(original, ids[table])
                changed = bytearray(original)
                left, right = slots[0], slots[1]
                if left[1] - left[0] != right[1] - right[0]:
                    raise RuntimeError("Expected equal-size duplicate test rows")
                changed[right[0] + 2:right[1]] = original[left[0] + 2:left[1]]
                cases.append((table, changed, error))
        mixed = roots(original, ids["DecimalMixedChildren"])
        # Adjacent >2^53 coefficient, crossed Band, and partial NULL must stay exact.
        changed = bytearray(original)
        at = variable_start(original, mixed[2], 0, 3)
        changed[at:at + 17] = coefficient_bytes(9_007_199_254_740_993)
        cases.append(("DecimalMixedChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[mixed[4][0] + 2:mixed[4][0] + 6] = (-(1 << 31)).to_bytes(4, "little", signed=True)
        cases.append(("DecimalMixedChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        at = variable_start(original, mixed[0], 1, 3)
        changed[at:at + 17] = coefficient_bytes(0)
        cases.append(("DecimalMixedChildren", changed, "foreign key has no referenced record"))
        for suffix, error in (("Parents", "referenced key is not unique"),
                ("UniqueChildren", "one-to-one relationship has duplicate dependent keys")):
            table = "DecimalMixed" + suffix
            slots = roots(original, ids[table])
            left, right = (slots[4], slots[5]) if suffix == "Parents" else (slots[0], slots[1])
            if left[1] - left[0] != right[1] - right[0]:
                raise RuntimeError("Expected equal-size partial-NULL rows")
            changed = bytearray(original)
            changed[right[0] + 2:right[1]] = original[left[0] + 2:left[1]]
            cases.append((table, changed, error))
        with tempfile.TemporaryDirectory(prefix="tablecraft-decimal-relationships-") as directory:
            copy = Path(directory) / "copy.accdb"
            for table, changed, error in cases:
                copy.write_bytes(changed)
                result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", table,
                    "--constraints"], capture_output=True, text=True)
                if result.returncode != 2 or result.stdout or error not in result.stderr:
                    raise RuntimeError("Decimal corrupt-copy audit did not fail atomically: " + table)
            exempt = bytearray(original)
            exempt[mixed[0][1] - 1] = 0
            copy.write_bytes(exempt)
            if "DecimalMixedKeys\tChecked: exact key membership\n" not in run(args.cli,
                    "inspect", copy, "--table", "DecimalMixedChildren", "--constraints"):
                raise RuntimeError("All-NULL Decimal tuple was not exempt")
            # Mismatched native parameters must decline even when every child is NULL.
            definition = ids["Decimal4Children"] * 4096
            indexes = int.from_bytes(original[definition + 51:definition + 55], "little")
            field = definition + 63 + indexes * 12
            if original[field] != 16 or original[field + 11:field + 13] != bytes([28, 4]):
                raise RuntimeError("Unexpected Decimal metadata for mismatch probe")
            for parameter, value in ((11, 27), (12, 3)):
                changed = bytearray(original)
                for slot in roots(original, ids["Decimal4Children"]):
                    changed[slot[1] - 1] = 0
                changed[field + parameter] = value
                copy.write_bytes(changed)
                output = run(args.cli, "inspect", copy, "--table", "Decimal4Children", "--constraints")
                if "Decimal4Keys\tNot checked: unsupported profile\n" not in output:
                    raise RuntimeError("Mismatched precision/scale was silently checked")
        if path.read_bytes() != original:
            raise RuntimeError("Original Decimal relationship input changed")
        print(path.name, "rows", row_count, "corrupt copies", len(cases), "metadata mismatches", 2, "file",
            hashlib.sha256(original).hexdigest(), "rows", hashlib.sha256(tagged.encode()).hexdigest())
    print("Exact Decimal/integer tuples, field scales, partial/all-NULL rules, uniqueness and buffered failures passed; native Access and cascades unrun.")


if __name__ == "__main__":
    main()
