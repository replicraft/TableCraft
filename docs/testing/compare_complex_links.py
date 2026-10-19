"""Compare scoped parent/child memberships and corrupt only disposable copies."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run


def column_layout(data, owner, name):
    page = data[owner * 4096:(owner + 1) * 4096]
    count = int.from_bytes(page[45:47], "little")
    physical = int.from_bytes(page[51:55], "little")
    start = 63 + physical * 12
    names = start + count * 25
    for index in range(count):
        size = int.from_bytes(page[names:names + 2], "little")
        label = page[names + 2:names + 2 + size].decode("utf-16-le")
        names += size + 2
        if label == name:
            field = page[start + index * 25:start + (index + 1) * 25]
            assert field[15] & 1 and int.from_bytes(field[23:25], "little") == 4
            return int.from_bytes(field[21:23], "little"), int.from_bytes(field[5:7], "little"), int.from_bytes(page[41:43], "little")
    raise AssertionError("Expected fixed field not found")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--ace2007", type=Path)
    args = parser.parse_args()
    for path in [args.ace2016, args.ace2019] + ([args.ace2007] if args.ace2007 else []):
        original = path.read_bytes()
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            "docs/testing/ComplexRowsFixture.java", "--links", path).splitlines()
        assert len(oracle) == 9
        catalog = run(args.cli, "inspect", path, "--catalog").splitlines()
        start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        fields = catalog[start].split("\t")
        ids = {cells[fields.index("Name")]: int(cells[fields.index("Native ID")])
            for cells in (row.split("\t") for row in catalog[start + 1:])}
        declarations = run(args.cli, "inspect", path, "--table", "Collections", "--complex").splitlines()[2:]
        links = []
        cases = []
        for declaration in declarations:
            column, _, _, flat, foreign, identity, _ = declaration.split("\t")
            output = run(args.cli, "inspect", path, "--table", "Collections", "--complex-links", column)
            rows = output.splitlines()[4:]
            links.extend(column + "\t" + row for row in rows)
            assert len(rows) == 3 and [int(row.split("\t")[2]) for row in rows] == [2, 1, 0]
            child = roots(original, ids[flat])
            parent = roots(original, ids["Collections"])
            fk_offset, fk_number, max_columns = column_layout(original, ids[flat], foreign)
            id_offset, _, _ = column_layout(original, ids[flat], identity)
            parent_offset, _, _ = column_layout(original, ids["Collections"], column)
            changed = bytearray(original)
            changed[child[1][0] + 2 + fk_offset:child[1][0] + 6 + fk_offset] = (999).to_bytes(4, "little")
            cases.append((column, changed, "child has no parent binding"))
            changed = bytearray(original)
            changed[child[1][0] + 2 + id_offset:child[1][0] + 6 + id_offset] = original[child[0][0] + 2 + id_offset:child[0][0] + 6 + id_offset]
            cases.append((column, changed, "duplicate native complex child identity"))
            changed = bytearray(original)
            mask = child[1][1] - (max_columns + 7) // 8
            changed[mask + fk_number // 8] &= ~(1 << (fk_number % 8))
            cases.append((column, changed, "NULL native complex child reference"))
            changed = bytearray(original)
            changed[parent[1][0] + 2 + parent_offset:parent[1][0] + 6 + parent_offset] = original[parent[0][0] + 2 + parent_offset:parent[0][0] + 6 + parent_offset]
            cases.append((column, changed, "ambiguous native complex parent reference"))
        assert sorted(links) == sorted(oracle)
        with tempfile.TemporaryDirectory(prefix="tablecraft-complex-links-") as directory:
            copy = Path(directory) / "copy.accdb"
            for column, data, error in cases:
                copy.write_bytes(data)
                result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", "Collections",
                    "--complex-links", column], capture_output=True, text=True, check=False)
                assert result.returncode == 2 and not result.stdout and error in result.stderr, result.stderr
        assert path.read_bytes() == original
        normalized = "\n".join(sorted(links)) + "\n"
        print(path.name, "parent links", 9, "corrupt copies", len(cases), "file",
            hashlib.sha256(original).hexdigest(), "links", hashlib.sha256(normalized.encode()).hexdigest())
    print("Independent scoped key/count checks and atomic corruption rejection pass; native Access, query expansion and attachment extraction unqualified.")


if __name__ == "__main__":
    main()
