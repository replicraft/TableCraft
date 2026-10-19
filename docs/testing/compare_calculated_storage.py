"""Compare explicit stored Int32 calculations without recalculation or Access claims."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import roots, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in [args.ace2016, args.ace2019]:
        original = path.read_bytes()
        expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            "docs/testing/TableDataOracle.java", path, "Calculations")
        values = [line.split("\t")[2] for line in expected.splitlines()]
        assert values == ["I32:20", "I32:-40", "I32:0", "N", "I32:41", "I32:-2147483648", "I32:2147483647"]
        header = "Table\tCalculations\nColumn\tTwice\nValues\tStored; not recalculated\nRow ordinal\tStored Int32 value\n"
        output = run(args.cli, "inspect", path, "--table", "Calculations", "--stored-calculation", "Twice")
        assert output == header + "".join(f"{i}\t{'NULL' if value == 'N' else value[4:]}\n" for i, value in enumerate(values))
        empty = run(args.cli, "inspect", path, "--table", "EmptyCalculations", "--stored-calculation", "Twice")
        assert empty == header.replace("Calculations\n", "EmptyCalculations\n", 1)
        ordinary = subprocess.run([str(args.cli), "query", "run", "SELECT Twice FROM Calculations",
            "--database", str(path)], capture_output=True, text=True, check=False)
        assert ordinary.returncode == 2 and not ordinary.stdout
        catalog = run(args.cli, "inspect", path, "--catalog", "--show-system").splitlines()
        start = next(i for i, line in enumerate(catalog) if line.startswith("Object ID\t"))
        fields = catalog[start].split("\t")
        owner = next(int(cells[fields.index("Native ID")]) for cells in
            (line.split("\t") for line in catalog[start + 1:]) if cells[fields.index("Name")] == "Calculations")
        second = roots(original, owner)[1][0]
        with tempfile.TemporaryDirectory(prefix="tablecraft-calculated-storage-") as directory:
            copy = Path(directory) / "copy.accdb"
            for offset in [0, 16, 24]:
                changed = bytearray(original)
                changed[second + 6 + offset] ^= 1
                copy.write_bytes(changed)
                result = subprocess.run([str(args.cli), "inspect", str(copy), "--table", "Calculations",
                    "--stored-calculation", "Twice"], capture_output=True, text=True, check=False)
                error = "malformed database" if offset == 16 else "format or layout is not supported"
                assert result.returncode == 2 and not result.stdout and error in result.stderr
        assert path.read_bytes() == original
        print(path.name, "stored rows", 7, "corrupt copies", 3, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(expected.encode()).hexdigest())
    print("Stored results, deliberate stale value, strict wrappers, explicit-only reads and immutable inputs verified; native Access/recalculation unrun.")


if __name__ == "__main__":
    main()
