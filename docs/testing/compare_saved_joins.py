"""Compare two-source saved join models and reject ambiguous native endpoints."""
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
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        names = oracle(args, path).splitlines()
        if len(names) != 22 or sum(name.startswith("Join") for name in names) != 9:
            raise RuntimeError("Original saved join controls changed")
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
        name = "JoinLeftForward"
        selected = [slot for slot in roots(original, definition) if int.from_bytes(
            original[slot[0] + 2 + object_offset:slot[0] + 6 + object_offset], "little", signed=True) == ids[name]]
        joins = [slot for slot in selected if original[slot[0] + 2 + attribute_offset] == 7]
        tables = [slot for slot in selected if original[slot[0] + 2 + attribute_offset] == 5]
        if len(selected) != 7 or len(joins) != 1 or len(tables) != 2:
            raise RuntimeError("Missing original endpoint mutation controls")
        join = joins[0]
        cases = []
        changed = bytearray(original)
        changed[join[0] + 2 + flag_offset:join[0] + 4 + flag_offset] = (4).to_bytes(2, "little")
        cases.append(("unknown join kind", changed))
        for field in ("Flag", "Name1", "Name2", "Expression"):
            changed = bytearray(original)
            number = layout[field][2]
            mask = join[1] - (columns + 7) // 8
            changed[mask + number // 8] &= ~(1 << (number % 8))
            cases.append(("NULL " + field, changed))
        left = variable_start(original, join, layout["Name1"][3], columns)
        alias = variable_start(original, tables[1], layout["Name2"][3], columns)
        if original[left:left + 2] != b"A\0" or original[alias:alias + 2] != b"b\0":
            raise RuntimeError("Unexpected original one-character UTF-16 label encoding")
        changed = bytearray(original)
        changed[left:left + 2] = b"Z\0"
        cases.append(("undeclared left endpoint", changed))
        changed = bytearray(original)
        changed[alias:alias + 2] = b"a\0"
        cases.append(("ambiguous aliases", changed))
        changed = bytearray(original)
        changed[alias:alias + 2] = "é".encode("utf-16-le")
        cases.append(("unqualified label comparison", changed))
        with tempfile.TemporaryDirectory(prefix="tablecraft-saved-join-") as directory:
            copy = Path(directory) / "copy.accdb"
            for _, changed in cases:
                copy.write_bytes(changed)
                # All mutations retain inspectable carriers; rejection belongs
                # to reconstruction, not a silently broken mutation recipe.
                report = run(args.cli, "inspect", copy, "--query", name)
                if "Fragment rows\t7\n" not in report:
                    raise RuntimeError("Mutated fragments lost inert inspectability")
                rejected(args, copy, name)
        if path.read_bytes() != original:
            raise RuntimeError("Original saved join fixture changed")
        print(path.name, "saved SELECTs", len(names), "two-source joins", 9, "rejected copies", len(cases),
            "file", hashlib.sha256(original).hexdigest(), "syntax", hashlib.sha256(canonical.encode()).hexdigest())
    print("Independent two-source join reconstruction matches; no execution or native Access qualification.")


if __name__ == "__main__":
    main()
