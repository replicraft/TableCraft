"""Compare exact Byte/Int16 FK tuples and failures on disposable native copies."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        catalog = run(args.cli, "inspect", path, "--catalog").splitlines()
        start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        fields = catalog[start].split("\t")
        ids = {cells[fields.index("Name")]: int(cells[fields.index("Native ID")])
            for cells in (line.split("\t") for line in catalog[start + 1:])}
        tagged = ""
        count = 0
        for table, rows, relationship in (
            ("ByteParents", 8, "ByteKeys"), ("ByteChildren", 8, "ByteKeys"),
            ("ByteUniqueChildren", 4, "ByteUniqueKeys"),
            ("ShortParents", 9, "ShortKeys"), ("ShortChildren", 9, "ShortKeys"),
            ("ShortUniqueChildren", 4, "ShortUniqueKeys"),
            ("NarrowParents", 7, "NarrowPairs"), ("NarrowChildren", 7, "NarrowPairs"),
            ("NarrowUniqueChildren", 3, "NarrowUniquePairs"),
        ):
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, table)
            actual = run(args.rows, path, table)
            if actual != expected or len(actual.splitlines()) != rows:
                raise RuntimeError("Exact narrow row carriers disagree")
            output = run(args.cli, "inspect", path, "--table", table, "--constraints")
            if f"Rows checked\t{rows}\n" not in output or f"{relationship}\tChecked: integer key membership\n" not in output or "Unsupported" in output:
                raise RuntimeError("Narrow relationship coverage incomplete")
            tagged += actual
            count += rows
        cases = []
        for table, size in (("ByteChildren", 1), ("ShortChildren", 2)):
            row = roots(original, ids[table])[0][0]
            changed = bytearray(original)
            changed[row + 2:row + 2 + size] = (42).to_bytes(size, "little", signed=True)
            cases.append((table, changed, "foreign key has no referenced record"))
        pair = roots(original, ids["NarrowChildren"])[0]
        changed = bytearray(original)
        changed[pair[0] + 3:pair[0] + 5] = (32767).to_bytes(2, "little", signed=True)
        cases.append(("NarrowChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[pair[1] - 1] &= ~1
        cases.append(("NarrowChildren", changed, "foreign key has no referenced record"))
        parents = roots(original, ids["ByteParents"])
        changed = bytearray(original)
        changed[parents[1][0] + 2:parents[1][1]] = original[parents[0][0] + 2:parents[0][1]]
        cases.append(("ByteParents", changed, "referenced key is not unique"))
        unique = roots(original, ids["NarrowUniqueChildren"])
        changed = bytearray(original)
        changed[unique[1][0] + 2:unique[1][1]] = original[unique[0][0] + 2:unique[0][1]]
        cases.append(("NarrowUniqueChildren", changed, "one-to-one relationship has duplicate dependent keys"))
        with tempfile.TemporaryDirectory(prefix="tablecraft-narrow-relationships-") as directory:
            copy = Path(directory) / "copy.accdb"
            for table, changed, error in cases:
                copy.write_bytes(changed)
                result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", table, "--constraints"], capture_output=True, text=True)
                if result.returncode != 2 or result.stdout or error not in result.stderr:
                    raise RuntimeError("Narrow corrupt-copy audit did not fail atomically")
            exempt = bytearray(original)
            exempt[pair[1] - 1] = 0
            copy.write_bytes(exempt)
            output = run(args.cli, "inspect", copy, "--table", "NarrowChildren", "--constraints")
            if "NarrowPairs\tChecked: integer key membership\n" not in output:
                raise RuntimeError("Entirely NULL tuple was not exempt")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        print(path.name, "rows", count, "corrupt copies", len(cases), "file", hashlib.sha256(original).hexdigest(), "rows", hashlib.sha256(tagged.encode()).hexdigest())
    print("Exact unsigned/signed tuples, partial/all NULL, uniqueness and failure buffering passed; native Access/cascades unrun.")


if __name__ == "__main__":
    main()
