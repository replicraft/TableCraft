"""Compare exact Int64/mixed relationship audits and corrupt disposable copies."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "rows", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in [args.ace2016, args.ace2019]:
        original = path.read_bytes()
        catalog = run(args.cli, "inspect", path, "--catalog").splitlines()
        header = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        fields = catalog[header].split("\t")
        ids = {cells[fields.index("Name")]: int(cells[fields.index("Native ID")])
            for cells in (row.split("\t") for row in catalog[header + 1:])}
        tagged = ""
        for table, count, relationship in [
            ("BigParents", 7, "BigKeys"), ("BigChildren", 7, "BigKeys"),
            ("BigUniqueChildren", 4, "UniqueBigKeys"),
            ("MixedParents", 7, "MixedKeys"), ("MixedChildren", 7, "MixedKeys"),
            ("MixedUniqueChildren", 3, "UniqueMixedKeys"),
        ]:
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, table)
            actual = run(args.rows, path, table)
            assert actual == expected, table
            tagged += actual
            output = run(args.cli, "inspect", path, "--table", table, "--constraints")
            assert f"Rows checked\t{count}\n" in output
            assert f"{relationship}\tChecked: exact key membership\n" in output, output
        cases = []
        big = roots(original, ids["BigChildren"])
        mixed = roots(original, ids["MixedChildren"])
        parents = roots(original, ids["BigParents"])
        unique = roots(original, ids["MixedUniqueChildren"])
        changed = bytearray(original)
        changed[big[0][0] + 2:big[0][0] + 10] = (9007199254740994).to_bytes(8, "little", signed=True)
        cases.append(("BigChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[mixed[2][0] + 6:mixed[2][0] + 14] = (9007199254740992).to_bytes(8, "little", signed=True)
        cases.append(("MixedChildren", changed, "foreign key has no referenced record"))
        changed = bytearray(original)
        changed[parents[1][0] + 2:parents[1][1]] = original[parents[0][0] + 2:parents[0][1]]
        cases.append(("BigParents", changed, "referenced key is not unique"))
        changed = bytearray(original)
        changed[unique[1][0] + 2:unique[1][1]] = original[unique[0][0] + 2:unique[0][1]]
        cases.append(("MixedUniqueChildren", changed, "one-to-one relationship has duplicate dependent keys"))
        with tempfile.TemporaryDirectory(prefix="tablecraft-int64-relations-") as directory:
            copy = Path(directory) / "copy.accdb"
            for table, data, error in cases:
                copy.write_bytes(data)
                result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", table,
                    "--constraints"], capture_output=True, text=True, check=False)
                assert result.returncode == 2 and not result.stdout and error in result.stderr, (table, result.stderr)
        assert path.read_bytes() == original
        print(path.name, "rows", 35, "corrupt copies", len(cases), "file",
            hashlib.sha256(original).hexdigest(), "rows", hashlib.sha256(tagged.encode()).hexdigest())
    print("Exact Int64/mixed tuples and failure atomicity verified; native Access/cascades remain unqualified.")


if __name__ == "__main__":
    main()
