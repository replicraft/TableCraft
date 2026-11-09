"""Compare exact native GUID/integer relationship membership on inert copies."""
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
        catalog = run(args.cli, "inspect", path, "--catalog", "--show-system").splitlines()
        start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        fields = catalog[start].split("\t")
        ids = {cells[fields.index("Name")]: int(cells[fields.index("Native ID")])
            for cells in (line.split("\t") for line in catalog[start + 1:])}
        tagged = ""
        count = 0
        for table, rows, relationship in (
            ("GuidParents", 6, "GuidKeys"), ("GuidChildren", 6, "GuidKeys"),
            ("GuidUniqueChildren", 3, "UniqueGuidKeys"),
            ("GuidMixedParents", 7, "GuidMixedKeys"), ("GuidMixedChildren", 7, "GuidMixedKeys"),
            ("GuidMixedUniqueChildren", 3, "UniqueGuidMixedKeys"),
        ):
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, table)
            actual = run(args.rows, path, table)
            if actual != expected or len(actual.splitlines()) != rows:
                raise RuntimeError("Canonical GUID/exact integer row mismatch")
            output = run(args.cli, "inspect", path, "--table", table, "--constraints")
            if f"Rows checked\t{rows}\n" not in output or f"{relationship}\tChecked: exact key membership\n" not in output or "Unsupported" in output:
                raise RuntimeError("GUID key membership coverage incomplete")
            tagged += actual
            count += rows
        case_probe = run("java", "-cp", args.jar, "docs/testing/GuidRelationshipFixture.java", "--probe-case", path)
        if case_probe != "lower\tfalse\nupper\ttrue\n":
            raise RuntimeError("Reviewed Jackcess default GUID cursor case observation changed")
        cases = []
        child = roots(original, ids["GuidChildren"])
        mixed = roots(original, ids["GuidMixedChildren"])
        parents = roots(original, ids["GuidParents"])
        unique = roots(original, ids["GuidMixedUniqueChildren"])
        changed = bytearray(original)
        changed[child[0][0] + 17] ^= 1
        cases.append(("GuidChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[mixed[0][0] + 18:mixed[0][0] + 26] = (9007199254740993).to_bytes(8, "little", signed=True)
        cases.append(("GuidMixedChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[mixed[2][0] + 17] ^= 1
        cases.append(("GuidMixedChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[mixed[0][1] - 1] &= ~1
        cases.append(("GuidMixedChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[parents[1][0] + 2:parents[1][1]] = original[parents[0][0] + 2:parents[0][1]]
        cases.append(("GuidParents", changed, "referenced key is not unique"))
        changed = bytearray(original)
        changed[unique[1][0] + 2:unique[1][1]] = original[unique[0][0] + 2:unique[0][1]]
        cases.append(("GuidMixedUniqueChildren", changed, "one-to-one relationship has duplicate dependent keys"))
        with tempfile.TemporaryDirectory(prefix="tablecraft-guid-relationships-") as directory:
            copy = Path(directory) / "copy.accdb"
            for table, changed, error in cases:
                copy.write_bytes(changed)
                result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", table, "--constraints"], capture_output=True, text=True)
                if result.returncode != 2 or result.stdout or error not in result.stderr:
                    raise RuntimeError("GUID corrupted-copy audit did not fail atomically")
            exempt = bytearray(original)
            exempt[mixed[0][1] - 1] = 0
            copy.write_bytes(exempt)
            if "GuidMixedKeys\tChecked: exact key membership\n" not in run(args.cli, "inspect", copy, "--table", "GuidMixedChildren", "--constraints"):
                raise RuntimeError("All-NULL GUID tuple was not exempt")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        print(path.name, "rows", count, "corrupt copies", len(cases), "file", hashlib.sha256(original).hexdigest(), "rows", hashlib.sha256(tagged.encode()).hexdigest())
    print("Exact GUID/integer tuples, NULL/zero-GUID distinction, uniqueness, immutable inputs and buffered failures passed; GUID indexes/native Access unrun.")


if __name__ == "__main__":
    main()
