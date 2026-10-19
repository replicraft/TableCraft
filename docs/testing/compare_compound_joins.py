"""Compare same-pair native join conjunctions and their stored condition order."""
import argparse
import hashlib
from pathlib import Path
import tempfile

from compare_composite_constraints import roots, run
from compare_complex_links import column_layout
from compare_decimal_relationships import variable_start
from compare_attachment_columns import fields
from compare_saved_queries import catalog_ids
from compare_saved_selects import compare, oracle, rejected


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "example", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--jet4", type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019, args.jet4):
        if path is None:
            continue
        original = path.read_bytes()
        names = oracle(args, path).splitlines()
        if len(names) != 28 or sum(name.startswith("Compound") for name in names) != 6:
            raise RuntimeError("Original compound join controls changed")
        canonical = "".join(name + "\t" + compare(args, path, name) for name in names)
        ids = catalog_ids(args, path)
        definition = ids["MSysQueries"]
        object_offset, _, columns = column_layout(original, definition, "ObjectId")
        layout = fields(original, definition)
        def fixed_offset(name, width):
            at = layout[name][0]
            if not original[at + 15] & 1 or int.from_bytes(original[at + 23:at + 25], "little") != width:
                raise RuntimeError("Unexpected native fixed metadata profile")
            return int.from_bytes(original[at + 21:at + 23], "little")
        attribute_offset, flag_offset = fixed_offset("Attribute", 1), fixed_offset("Flag", 2)
        name = "CompoundLeft3"
        selected = [slot for slot in roots(original, definition) if int.from_bytes(
            original[slot[0] + 2 + object_offset:slot[0] + 6 + object_offset], "little", signed=True) == ids[name]]
        joins = [slot for slot in selected if original[slot[0] + 2 + attribute_offset] == 7]
        if len(selected) != 9 or len(joins) != 3:
            raise RuntimeError("Missing original compound join mutation controls")
        first, second, third = joins
        cases = []
        changed = bytearray(original)
        changed[second[0] + 2 + flag_offset:second[0] + 4 + flag_offset] = (3).to_bytes(2, "little")
        cases.append(("mixed native join kinds", changed))
        for field in ("Flag", "Name1", "Name2"):
            changed = bytearray(original)
            number = layout[field][2]
            mask = second[1] - (columns + 7) // 8
            changed[mask + number // 8] &= ~(1 << (number % 8))
            cases.append(("NULL condition " + field, changed))
        left = variable_start(original, second, layout["Name1"][3], columns)
        right = variable_start(original, second, layout["Name2"][3], columns)
        if original[left:left + 2] != b"a\0" or original[right:right + 2] != b"b\0":
            raise RuntimeError("Unexpected original one-character endpoint encoding")
        changed = bytearray(original)
        changed[left:left + 2], changed[right:right + 2] = original[right:right + 2], original[left:left + 2]
        cases.append(("unqualified reversed endpoints", changed))
        changed = bytearray(original)
        changed[left:left + 2] = b"Z\0"
        cases.append(("unqualified condition endpoint", changed))
        order_variable = layout["Order"][3]
        offsets = [variable_start(original, slot, order_variable, columns) for slot in joins]
        if [original[at:at + 4] for at in offsets] != [i.to_bytes(4, "big") for i in (1, 2, 3)]:
            raise RuntimeError("Unexpected stored condition ordering profile")
        changed = bytearray(original)
        changed[offsets[1]:offsets[1] + 4] = original[offsets[0]:offsets[0] + 4]
        cases.append(("duplicate condition key", changed))
        changed = bytearray(original)
        changed[offsets[1]:offsets[1] + 4], changed[offsets[2]:offsets[2] + 4] = original[offsets[2]:offsets[2] + 4], original[offsets[1]:offsets[1] + 4]
        cases.append(("reversed storage keys", changed))
        with tempfile.TemporaryDirectory(prefix="tablecraft-compound-joins-") as directory:
            copy = Path(directory) / "copy.accdb"
            for _, changed in cases:
                copy.write_bytes(changed)
                if "Fragment rows\t9\n" not in run(args.cli, "inspect", copy, "--query", name):
                    raise RuntimeError("Compound mutation lost inert inspectability")
                rejected(args, copy, name)
        if path.read_bytes() != original:
            raise RuntimeError("Original compound join fixture changed")
        print(path.name, "saved SELECTs", len(names), "compound joins", 6, "rejected copies", len(cases),
            "file", hashlib.sha256(original).hexdigest(), "syntax", hashlib.sha256(canonical.encode()).hexdigest())
    print("Independent condition ordering/models match; no query execution or actual Access qualification.")


if __name__ == "__main__":
    main()
