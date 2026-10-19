"""Compare native saved parameter declarations without prompting or execution."""
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
        if len(names) != 21 or "AllParameterTypes" not in names:
            raise RuntimeError("Original saved parameter controls changed")
        canonical = "".join(name + "\t" + compare(args, path, name) for name in names)
        ids = catalog_ids(args, path)
        definition = ids["MSysQueries"]
        object_offset, _, columns = column_layout(original, definition, "ObjectId")
        layout = fields(original, definition)
        def fixed_offset(name, width):
            at = layout[name][0]
            if not original[at + 15] & 1 or int.from_bytes(original[at + 23:at + 25], "little") != width:
                raise RuntimeError("Unexpected fixed metadata profile")
            return int.from_bytes(original[at + 21:at + 23], "little")
        attribute_offset = fixed_offset("Attribute", 1)
        flag_offset, extra_offset = fixed_offset("Flag", 2), fixed_offset("LvExtra", 4)
        name = "AllParameterTypes"
        selected = [slot for slot in roots(original, definition) if int.from_bytes(
            original[slot[0] + 2 + object_offset:slot[0] + 6 + object_offset], "little", signed=True) == ids[name]]
        parameters = [slot for slot in selected if original[slot[0] + 2 + attribute_offset] == 2]
        if len(selected) != 17 or len(parameters) != 12:
            raise RuntimeError("Missing native parameter controls")
        first, second, text = parameters[0], parameters[1], parameters[9]
        if [int.from_bytes(original[s[0] + 2 + flag_offset:s[0] + 4 + flag_offset], "little") for s in (first, second, text)] != [1, 2, 10]:
            raise RuntimeError("Unexpected native parameter type ordering")
        def presence(changed, slot, field, present):
            number = layout[field][2]
            mask = slot[1] - (columns + 7) // 8
            if present:
                changed[mask + number // 8] |= 1 << (number % 8)
            else:
                changed[mask + number // 8] &= ~(1 << (number % 8))
        cases = []
        changed = bytearray(original)
        changed[first[0] + 2 + flag_offset:first[0] + 4 + flag_offset] = (16).to_bytes(2, "little")
        cases.append(("unqualified parameter type", changed))
        for field in ("Flag", "Name1"):
            changed = bytearray(original)
            presence(changed, first, field, False)
            cases.append(("NULL " + field, changed))
        changed = bytearray(original)
        changed[text[0] + 2 + extra_offset:text[0] + 6 + extra_offset] = (-1).to_bytes(4, "little", signed=True)
        cases.append(("negative text syntax length", changed))
        changed = bytearray(original)
        presence(changed, first, "LvExtra", True)
        changed[first[0] + 2 + extra_offset:first[0] + 6 + extra_offset] = bytes(4)
        cases.append(("unhandled non-text extra", changed))
        first_name = variable_start(original, first, layout["Name1"][3], columns)
        second_name = variable_start(original, second, layout["Name1"][3], columns)
        first_end = variable_start(original, first, layout["Name1"][3] + 1, columns)
        second_end = variable_start(original, second, layout["Name1"][3] + 1, columns)
        encoded = (original[first_name:first_end], original[second_name:second_end])
        if encoded not in ((b"\xff\xfepBool", b"\xff\xfepByte"), ("pBool".encode("utf-16-le"), "pByte".encode("utf-16-le"))):
            raise RuntimeError("Unexpected original equal-width name encoding")
        changed = bytearray(original)
        changed[second_name:second_end] = encoded[0]
        cases.append(("ambiguous parameter names", changed))
        first_order = variable_start(original, first, layout["Order"][3], columns)
        second_order = variable_start(original, second, layout["Order"][3], columns)
        if original[first_order:first_order + 4] != b"\0\0\0\1" or original[second_order:second_order + 4] != b"\0\0\0\2":
            raise RuntimeError("Unexpected parameter ordering profile")
        changed = bytearray(original)
        changed[second_order:second_order + 4] = original[first_order:first_order + 4]
        cases.append(("duplicate parameter order", changed))
        changed = bytearray(original)
        presence(changed, first, "Name2", True)
        cases.append(("unused non-NULL empty name", changed))
        with tempfile.TemporaryDirectory(prefix="tablecraft-saved-parameters-") as directory:
            copy = Path(directory) / "copy.accdb"
            for _, changed in cases:
                copy.write_bytes(changed)
                if "Fragment rows\t17\n" not in run(args.cli, "inspect", copy, "--query", name):
                    raise RuntimeError("Parameter mutation lost inert inspectability")
                rejected(args, copy, name)
        if path.read_bytes() != original:
            raise RuntimeError("Original parameter fixture changed")
        print(path.name, "saved SELECTs", len(names), "parameterized", 8, "native types", 12, "rejected copies", len(cases),
            "file", hashlib.sha256(original).hexdigest(), "syntax", hashlib.sha256(canonical.encode()).hexdigest())
    print("Independent declaration reconstruction matches. No prompting, native query execution or actual Access qualification.")


if __name__ == "__main__":
    main()
