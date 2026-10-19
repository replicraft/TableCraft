"""Compare exact Currency/integer foreign keys and reject corrupted disposable copies."""
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
            ("MoneyParents", 12, "MoneyKeys"), ("MoneyChildren", 12, "MoneyKeys"),
            ("MoneyUniqueChildren", 4, "UniqueMoneyKeys"),
            ("MoneyMixedParents", 8, "MoneyMixedKeys"), ("MoneyMixedChildren", 8, "MoneyMixedKeys"),
            ("MoneyMixedUniqueChildren", 4, "UniqueMoneyMixedKeys"),
        ):
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, table)
            actual = run(args.rows, path, table)
            if actual != expected or len(actual.splitlines()) != rows:
                raise RuntimeError("Exact scaled Currency/integer row mismatch")
            output = run(args.cli, "inspect", path, "--table", table, "--constraints")
            if (f"Rows checked\t{rows}\n" not in output
                    or f"{relationship}\tChecked: exact key membership\n" not in output
                    or "Unsupported" in output):
                raise RuntimeError("Currency key membership coverage incomplete")
            tagged += actual
            count += rows
        cases = []
        child = roots(original, ids["MoneyChildren"])
        mixed = roots(original, ids["MoneyMixedChildren"])
        parents = roots(original, ids["MoneyParents"])
        mixed_parents = roots(original, ids["MoneyMixedParents"])
        unique = roots(original, ids["MoneyUniqueChildren"])
        mixed_unique = roots(original, ids["MoneyMixedUniqueChildren"])
        for table, at, value in (
            ("MoneyChildren", child[0][0] + 2, 2),
            ("MoneyMixedChildren", mixed[0][0] + 2, (1 << 63) - 1),
            ("MoneyMixedChildren", mixed[2][0] + 2, 9_007_199_254_740_993),
        ):
            changed = bytearray(original)
            changed[at:at + 8] = value.to_bytes(8, "little", signed=True)
            cases.append((table, changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[mixed[4][0] + 10:mixed[4][0] + 14] = (-(1 << 31)).to_bytes(4, "little", signed=True)
        cases.append(("MoneyMixedChildren", changed, "foreign key has no referenced record"))
        for table, slots, destination, source, error in (
            ("MoneyParents", parents, 1, 0, "referenced key is not unique"),
            ("MoneyMixedParents", mixed_parents, 5, 4, "referenced key is not unique"),
            ("MoneyUniqueChildren", unique, 1, 0, "one-to-one relationship has duplicate dependent keys"),
            ("MoneyMixedUniqueChildren", mixed_unique, 1, 0, "one-to-one relationship has duplicate dependent keys"),
        ):
            changed = bytearray(original)
            changed[slots[destination][0] + 2:slots[destination][1]] = original[slots[source][0] + 2:slots[source][1]]
            cases.append((table, changed, error))
        with tempfile.TemporaryDirectory(prefix="tablecraft-currency-relationships-") as directory:
            copy = Path(directory) / "copy.accdb"
            for table, changed, error in cases:
                copy.write_bytes(changed)
                result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", table,
                    "--constraints"], capture_output=True, text=True)
                if result.returncode != 2 or result.stdout or error not in result.stderr:
                    raise RuntimeError("Currency corrupt-copy audit did not fail atomically")
            exempt = bytearray(original)
            exempt[mixed[0][1] - 1] = 0
            copy.write_bytes(exempt)
            if "MoneyMixedKeys\tChecked: exact key membership\n" not in run(args.cli,
                    "inspect", copy, "--table", "MoneyMixedChildren", "--constraints"):
                raise RuntimeError("All-NULL Currency tuple was not exempt")
        if path.read_bytes() != original:
            raise RuntimeError("Original Currency relationship input changed")
        print(path.name, "rows", count, "corrupt copies", len(cases), "file",
            hashlib.sha256(original).hexdigest(), "rows", hashlib.sha256(tagged.encode()).hexdigest())
    print("Exact Currency/integer tuples, partial/all-NULL rules, uniqueness and buffered failures passed; native Access and cascades unrun.")


if __name__ == "__main__":
    main()
