"""Compare complete native compound numeric trees with independent index cursors."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run


def geometry(data, owner):
    counts = {3: 0, 4: 0}
    largest = {3: 0, 4: 0}
    prefix_max = 0
    for at in range(0, len(data), 4096):
        page = data[at:at + 4096]
        if (page[0] not in counts or page[1] != 1
                or int.from_bytes(page[4:8], "little") != owner):
            continue
        kind = page[0]
        counts[kind] += 1
        prefix = int.from_bytes(page[24:26], "little")
        prefix_max = max(prefix_max, prefix)
        last = 0
        for offset, mask in enumerate(page[27:480]):
            for bit in range(8):
                if mask & (1 << bit):
                    end = offset * 8 + bit
                    if not last < end <= 3616:
                        raise RuntimeError("Unexpected independent bitmap geometry")
                    largest[kind] = max(largest[kind], end - last + (prefix if last else 0))
                    last = end
    return counts, largest, prefix_max


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in (args.ace2016, args.ace2019):
        original = path.read_bytes()
        normalized = ""
        count = 0
        mixed_roots = []
        for table, indexes, columns, rows in (
            ("MixedIndexed", ("FiveAsc", "FiveDesc", "FiveMixed"),
                ("ID", "Tiny", "Small", "Normal", "Large", "Amount"), 1000),
            ("TenIndexed", ("TenAsc", "TenDesc", "TenMixed"),
                ("ID", *("K" + str(i) for i in range(10))), 500),
            ("PairUnique", ("UniquePair",), ("A", "B"), 10),
            ("PairPrimary", ("PrimaryPair",), ("A", "B"), 3),
            ("EmptyPair", ("EmptyPairKey",), ("A", "B"), 0),
        ):
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes")
            roots_by_name = {row.split("\t")[0]: int(row.split("\t")[3])
                for row in metadata.splitlines() if row.split("\t")[0] in indexes}
            if set(roots_by_name) != set(indexes):
                raise RuntimeError("Missing compound native root metadata")
            native_roots = [roots_by_name[index] for index in indexes]
            if rows >= 500 and any(original[root * 4096] != 3 for root in native_roots):
                raise RuntimeError("Expected actual compound node/leaf trees")
            if table == "MixedIndexed":
                mixed_roots = native_roots
            if table == "TenIndexed":
                owner = int.from_bytes(original[native_roots[0] * 4096 + 4:native_roots[0] * 4096 + 8], "little")
                counts, largest, prefix_max = geometry(original, owner)
                if largest != {3: 98, 4: 94}:
                    raise RuntimeError("Maximum-width ten-component geometry is missing")
                print(path.name, "ten-component node/leaf pages", counts[3], counts[4],
                    "largest entries", largest[3], largest[4], "largest prefix", prefix_max)
            for index in indexes:
                expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/IndexRowsOracle.java", path, table, index, *columns)
                if len(expected.splitlines()) != rows:
                    raise RuntimeError("Unexpected independent compound cursor count")
                sql = "SELECT " + ",".join("[" + column + "]" for column in columns) + " FROM " + table
                actual = run(args.cli, "query", "run", sql, "--database", path, "--index", table, index)
                if actual != "\t".join(columns) + "\n" + expected:
                    raise RuntimeError("Exact compound index key/reference order differs")
                normalized += actual
                count += rows
                if rows >= 500:
                    selected = "".join(row + "\n" for row in expected.splitlines()
                        if int(row.split("\t")[0]) < 10)
                    if run(args.cli, "query", "run", sql + " WHERE ID < 10", "--database", path,
                            "--index", table, index) != "\t".join(columns) + "\n" + selected:
                        raise RuntimeError("Compound required-index residual query differs")
            check = run(args.cli, "inspect", path, "--table", table, "--check")
            if "unsupported" in check.lower() or "checked" not in check.lower():
                raise RuntimeError("Compound numeric check coverage incomplete")
        with tempfile.TemporaryDirectory(prefix="tablecraft-composite-index-") as directory:
            copy = Path(directory) / "copy.accdb"
            # Corrupt only an owned compound tree root. A sequential fallback
            # would incorrectly succeed and expose rows despite the selected index.
            changed = bytearray(original)
            changed[mixed_roots[0] * 4096] = 0
            copy.write_bytes(changed)
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT ID FROM MixedIndexed",
                "--database", str(copy), "--index", "MixedIndexed", "FiveAsc"], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Corrupt selected compound index did not fail atomically")
        for table, index in (("SkipPair", "SkipPairKey"), ("TextPair", "TextPairKey")):
            failed = subprocess.run([str(args.cli), "query", "run", "SELECT * FROM " + table,
                "--database", str(path), "--index", table, index], capture_output=True, text=True)
            if failed.returncode != 2 or failed.stdout:
                raise RuntimeError("Unqualified compound profile silently fell back")
        if path.read_bytes() != original:
            raise RuntimeError("Original compound index fixture changed")
        print(path.name, "ordered rows", count, "file", hashlib.sha256(original).hexdigest(),
            "rows", hashlib.sha256(normalized.encode()).hexdigest())
    print("Compound numeric trees, component directions, exact NULL/unique/primary checks and failure buffering passed; native Access unrun.")


if __name__ == "__main__":
    main()
