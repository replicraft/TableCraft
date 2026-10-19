"""Compare exact reciprocal-index metadata and reject corrupt disposable copies."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run
from compare_saved_queries import catalog_ids


TABLES = ("RestrictParents", "RestrictChildren", "SecondChildren", "CascadeParents",
    "CascadeChildren", "SetNullParents", "SetNullChildren", "CompositeParents",
    "CompositeChildren", "SelfLinks")
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
    return "\n".join(sorted(actual.splitlines())) + "\n"


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
        if len(canonical.splitlines()) != 12:
            raise RuntimeError("Expected twelve reciprocal links")
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
            cases.append(changed)
        with tempfile.TemporaryDirectory(prefix="tablecraft-foreign-links-") as directory:
            copy = Path(directory) / "copy.accdb"
            for changed in cases:
                copy.write_bytes(changed)
                result = subprocess.run([str(args.example), str(copy), "RestrictChildren"],
                    capture_output=True, text=True)
                if result.returncode == 0 or result.stdout:
                    raise RuntimeError("Corrupt/unqualified link published partial output")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        print(path.name, "links", 12, "corrupt copies", len(cases), "original",
            hashlib.sha256(original).hexdigest(), "links-sha256",
            hashlib.sha256(canonical.encode()).hexdigest(), flush=True)
    original = args.sample.read_bytes()
    canonical = "".join(compare(args, args.sample, table) for table in ("TAG_GRP_TB", "TAG_NME_TB"))
    if len(canonical.splitlines()) != 2 or args.sample.read_bytes() != original:
        raise RuntimeError("Registered sample link evidence differs")
    print("Registered sample links", 2, "links-sha256", hashlib.sha256(canonical.encode()).hexdigest())
    print("Reciprocal metadata matches; row/catalog agreement, foreign-index writes and native Access remain unqualified.")


if __name__ == "__main__":
    main()
