"""Compare exact reciprocal-index metadata and reject corrupt disposable copies."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run
from compare_complex_links import column_layout
from compare_attachment_columns import fields
from compare_decimal_relationships import variable_start
from compare_saved_queries import catalog_ids


TABLES = ("RestrictParents", "RestrictChildren", "SecondChildren", "CascadeParents",
    "CascadeChildren", "SetNullParents", "SetNullChildren", "CompositeParents",
    "CompositeChildren", "SelfLinks", "OneParents", "OneChildren", "LooseParents", "LooseChildren")
PAGE_BYTES = 4096
LOGICAL_BYTES = 28


def logical_offsets(data, definition):
    """Controlled single-page fixtures only; no production decoder fallback."""
    base = definition * PAGE_BYTES
    if data[base:base + 2] != b"\x02\x01" or any(data[base + 4:base + 8]):
        raise RuntimeError("Expected original single-page table definition")
    physical = int.from_bytes(data[base + 51:base + 55], "little")
    columns = int.from_bytes(data[base + 45:base + 47], "little")
    logical = int.from_bytes(data[base + 47:base + 51], "little")
    names = base + 63 + physical * 12 + columns * 25
    for _ in range(columns):
        size = int.from_bytes(data[names:names + 2], "little")
        names += size + 2
    start = names + physical * 52
    if start + logical * LOGICAL_BYTES > base + PAGE_BYTES:
        raise RuntimeError("Logical fixture definitions cross page boundary")
    return {int.from_bytes(data[at + 4:at + 8], "little"): at
        for at in range(start, start + logical * LOGICAL_BYTES, LOGICAL_BYTES)}


def compare(args, path, table):
    expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
        "docs/testing/ForeignIndexOracle.java", path, table)
    actual = run(args.example, path, table)
    if sorted(actual.splitlines()) != sorted(expected.splitlines()):
        raise RuntimeError("Exact reciprocal-index metadata differs: " + table)
    return "".join(line + "\n" for line in sorted(actual.splitlines()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "example", "ace2016", "ace2019", "sample"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path, code in ((args.ace2016, 5), (args.ace2019, 6)):
        original = path.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Expected original ACE 16 family")
        canonical = "".join(compare(args, path, table) for table in TABLES)
        if len(canonical.splitlines()) != 14:
            raise RuntimeError("Expected fourteen reciprocal links")
        ids = catalog_ids(args, path)
        child = logical_offsets(original, ids["RestrictChildren"])
        foreign = next(at for at in child.values() if original[at + 23] == 2)
        target_page = int.from_bytes(original[foreign + 17:foreign + 21], "little")
        target_index = int.from_bytes(original[foreign + 13:foreign + 17], "little")
        parent = logical_offsets(original, target_page)[target_index]
        cases = []
        for start, offset, size, value in [
            (foreign, 17, 4, 0), (foreign, 17, 4, len(original) // PAGE_BYTES),
            (foreign, 17, 4, 3), (foreign, 13, 4, 99),
            (foreign, 13, 4, 0xffffffff), (foreign, 12, 1, 0),
            (foreign, 12, 1, 3), (foreign, 21, 1, 2),
            (foreign, 22, 1, 3), (foreign, 22, 1, 4),
            (parent, 13, 4, 0), (parent, 17, 4, 3),
            (parent, 12, 1, 2), (parent, 21, 1, 1),
            (parent, 22, 1, 1), (parent, 23, 1, 0),
        ]:
            changed = bytearray(original)
            changed[start + offset:start + offset + size] = value.to_bytes(size, "little")
            cases.append(("RestrictChildren", changed))
        relation_owner = ids["MSysRelationships"]
        relation_fields = fields(original, relation_owner)
        maximum = int.from_bytes(original[relation_owner * PAGE_BYTES + 41:relation_owner * PAGE_BYTES + 43], "little")
        name_column = relation_fields["szRelationship"][3]
        slots = roots(original, relation_owner)
        slot = next(slot for slot in slots if original[variable_start(original, slot, name_column, maximum):].startswith("RestrictLink".encode("utf-16-le")))
        flag_offset = column_layout(original, relation_owner, "grbit")[0]
        for flags in (0x100, 0x1000, 0x2000, 2, 4, 0x80000000):
            changed = bytearray(original)
            at = slot[0] + 2 + flag_offset
            changed[at:at + 4] = flags.to_bytes(4, "little")
            cases.append(("RestrictChildren", changed))
        for name in ("szObject", "szReferencedObject", "szColumn", "szReferencedColumn"):
            changed = bytearray(original)
            at = variable_start(original, slot, relation_fields[name][3], maximum)
            changed[at:at + 2] = "Z".encode("utf-16-le")
            cases.append(("RestrictChildren", changed))
        # Both references may agree with each other while disagreeing with the catalog.
        changed = bytearray(original)
        changed[foreign + 21] = changed[parent + 21] = 1
        cases.append(("RestrictChildren", changed))
        # Removing both logical foreign types cannot hide an enforced catalog edge.
        changed = bytearray(original)
        changed[foreign + 23] = changed[parent + 23] = 0
        cases.append(("RestrictChildren", changed))
        composite = [slot for slot in slots if original[variable_start(original, slot, name_column, maximum):].startswith("CompositeLink".encode("utf-16-le"))]
        if len(composite) != 2:
            raise RuntimeError("Expected two composite relationship rows")
        changed = bytearray(original)
        for slot in composite:
            at = variable_start(original, slot, relation_fields["szColumn"][3], maximum)
            changed[at:at + 2] = ("Y" if original[at:at + 2] == b"X\0" else "X").encode("utf-16-le")
        cases.append(("CompositeChildren", changed))
        one_page = ids["OneChildren"]
        one_logical = logical_offsets(original, one_page)
        one_foreign = next(at for at in one_logical.values() if original[at + 23] == 2)
        physical_count = int.from_bytes(original[one_page * PAGE_BYTES + 51:one_page * PAGE_BYTES + 55], "little")
        data_number = int.from_bytes(original[one_foreign + 8:one_foreign + 12], "little")
        flag = min(one_logical.values()) - physical_count * 52 + data_number * 52 + 46
        changed = bytearray(original); changed[flag] &= ~1
        cases.append(("OneChildren", changed))
        catalog_fields = fields(original, 2)
        catalog_maximum = int.from_bytes(original[2 * PAGE_BYTES + 41:2 * PAGE_BYTES + 43], "little")
        alias_row = next(slot for slot in roots(original, 2) if original[variable_start(original, slot, catalog_fields["Name"][3], catalog_maximum):].startswith("LooseChildren".encode("utf-16-le")))
        at = alias_row[0] + 2 + column_layout(original, 2, "Id")[0]
        changed = bytearray(original)
        changed[at:at + 4] = (0x01000000 | ids["RestrictChildren"]).to_bytes(4, "little")
        cases.append(("RestrictChildren", changed))
        with tempfile.TemporaryDirectory(prefix="tablecraft-foreign-links-") as directory:
            copy = Path(directory) / "copy.accdb"
            for table, changed in cases:
                copy.write_bytes(changed)
                result = subprocess.run([str(args.example), str(copy), table],
                    capture_output=True, text=True)
                if result.returncode == 0 or result.stdout:
                    raise RuntimeError("Corrupt/unqualified link published partial output")
            # Declaration ordinal order may differ from physical key order.
            changed = bytearray(original)
            ordinal_offset = column_layout(original, relation_owner, "icolumn")[0]
            for slot in composite:
                at = slot[0] + 2 + ordinal_offset
                ordinal = int.from_bytes(original[at:at + 4], "little")
                changed[at:at + 4] = (1 - ordinal).to_bytes(4, "little")
            copy.write_bytes(changed)
            compare(args, copy, "CompositeChildren")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        print(path.name, "links", 14, "corrupt copies", len(cases), "original",
            hashlib.sha256(original).hexdigest(), "links-sha256",
            hashlib.sha256(canonical.encode()).hexdigest(), flush=True)
    original = args.sample.read_bytes()
    canonical = "".join(compare(args, args.sample, table) for table in ("TAG_GRP_TB", "TAG_NME_TB"))
    if len(canonical.splitlines()) != 2 or args.sample.read_bytes() != original:
        raise RuntimeError("Registered sample link evidence differs")
    print("Registered sample links", 2, "links-sha256", hashlib.sha256(canonical.encode()).hexdigest())
    print("Reciprocal metadata and relationship catalogs agree; row membership, foreign-index writes and native Access remain unqualified.")


if __name__ == "__main__":
    main()
