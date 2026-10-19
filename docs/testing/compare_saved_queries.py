"""Compare inert saved-query fragments with independent native metadata getters."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run
from compare_complex_links import column_layout
from compare_attachment_columns import fields


def oracle(args, path, name=None):
    command = ["java", "-Dfile.encoding=UTF-8", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
        "docs/testing/SavedQueryOracle.java", path]
    return run(*(command + ([] if name is None else [name])))


def compare(args, path, name):
    expected = oracle(args, path, name)
    actual = run(args.cli, "inspect", path, "--query=" + name)
    if actual != expected:
        raise RuntimeError("Exact saved-query metadata differs: " + name)
    return actual


def rejected(args, path, name):
    result = subprocess.run([str(args.cli), "inspect", str(path), "--query=" + name],
        capture_output=True, text=True)
    if result.returncode != 2 or result.stdout:
        raise RuntimeError("Unqualified/corrupt query metadata published output")


def catalog_ids(args, path):
    lines = run(args.cli, "inspect", path, "--catalog").splitlines()
    start = next(i for i, line in enumerate(lines) if line.startswith("Object ID\t"))
    labels = lines[start].split("\t")
    return {cells[labels.index("Name")]: int(cells[labels.index("Native ID")])
        for cells in (line.split("\t") for line in lines[start + 1:])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2016", "ace2019", "sample"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path, query_count, row_count in ((args.ace2016, 11, 75), (args.ace2019, 11, 75), (args.sample, 59, 332)):
        original = path.read_bytes()
        names = oracle(args, path).splitlines()
        if len(names) != query_count:
            raise RuntimeError("Independent saved-query count differs")
        canonical = ""
        rows = 0
        for name in names:
            actual = compare(args, path, name)
            count = int(actual.splitlines()[3].split("\t")[1])
            if len(actual.splitlines()) != count + 5:
                raise RuntimeError("Fragment report lost row boundaries")
            canonical += actual
            rows += count
            if name == "UnknownFragments":
                if ("200\t\"native é🦀\\u{0000}\\tline\\n\"\t-32768\t-2147483648\t\"NULL\"\t\"\"\t0x\n" not in actual
                        or "Flags\t0x80000003\n" not in actual):
                    raise RuntimeError("Unknown attributes, raw flags, NULL/empty/control text or binary changed")
            if name == "EmptyDefinition" and count != 0:
                raise RuntimeError("Empty stored definition was fabricated")
        if rows != row_count:
            raise RuntimeError("Saved-query fragments missing")
        if path != args.sample:
            ids = catalog_ids(args, path)
            definition = ids["MSysQueries"]
            native_id = ids["UnknownFragments"]
            layout = fields(original, definition)
            all_slots = roots(original, definition)
            object_offset, _, columns = column_layout(original, definition, "ObjectId")
            attribute_field, _, attribute_number, _ = layout["Attribute"]
            if original[attribute_field] != 2 or not original[attribute_field + 15] & 1:
                raise RuntimeError("Expected fixed Byte attribute field")
            attribute_offset = int.from_bytes(original[attribute_field + 21:attribute_field + 23], "little")
            selected = [slot for slot in all_slots if int.from_bytes(
                original[slot[0] + 2 + object_offset:slot[0] + 6 + object_offset], "little", signed=True) == native_id]
            if len(selected) != 3:
                raise RuntimeError("Unknown fragment mutation controls missing")
            with tempfile.TemporaryDirectory(prefix="tablecraft-saved-query-") as directory:
                copy = Path(directory) / "copy.accdb"
                for kind in ("attribute", "null-attribute", "null-order", "unbound"):
                    changed = bytearray(original)
                    slot = selected[1]
                    mask = slot[1] - (columns + 7) // 8
                    if kind == "attribute":
                        changed[slot[0] + 2 + attribute_offset] = 202
                    elif kind == "null-attribute":
                        changed[mask + attribute_number // 8] &= ~(1 << (attribute_number % 8))
                    elif kind == "null-order":
                        number = layout["Order"][2]
                        changed[mask + number // 8] &= ~(1 << (number % 8))
                    else:
                        changed[slot[0] + 2 + object_offset:slot[0] + 6 + object_offset] = (native_id + 10).to_bytes(4, "little", signed=True)
                    copy.write_bytes(changed)
                    compare(args, copy, "UnknownFragments")
                changed = bytearray(original)
                at = definition * 4096 + 16
                changed[at:at + 4] = (row_count + 1).to_bytes(4, "little")
                copy.write_bytes(changed); rejected(args, copy, "UnknownFragments")
                changed = bytearray(original); changed[layout["Flag"][0]] = 9
                copy.write_bytes(changed); rejected(args, copy, "UnknownFragments")
            rejected(args, path, "Items")
            rejected(args, path, "missing")
        if path.read_bytes() != original:
            raise RuntimeError("Original saved-query input changed")
        print(path.name, "queries", query_count, "fragments", rows, "file",
            hashlib.sha256(original).hexdigest(), "metadata", hashlib.sha256(canonical.encode()).hexdigest())
    print("Exact native identities/fragments, unknown/NULL inspection, namespace/type/geometry failures and unchanged bytes passed; SQL reconstruction/execution, index integrity and native Access unrun.")


if __name__ == "__main__":
    main()
