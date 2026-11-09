"""Compare foreign index order/rows and complete checks; preserve original inputs."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run
from compare_foreign_indexes import TABLES, PAGE_BYTES, logical_offsets
from compare_saved_queries import catalog_ids


def foreign_indexes(args, path, table):
    lines = run(args.cli, "inspect", path, "--table", table, "--indexes").splitlines()
    labels = lines[1].split("\t")
    result = {}
    for line in lines[2:]:
        cells = line.split("\t")
        if cells[labels.index("Type")] == "2":
            result[cells[0]] = int(cells[labels.index("Root page")])
    return result


def compare(args, path, table, index):
    expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
        "docs/testing/TableDataOracle.java", path, "--index", table, index)
    actual = run(args.rows, path, table, "--index", index)
    if actual != expected:
        raise RuntimeError("Foreign index order/rows differ: " + table + "." + index)
    return actual


def rejected(args, path, table, index):
    for command in ((args.rows, path, table, "--index", index),
            (args.cli, "inspect", path, "--table", table, "--check")):
        result = subprocess.run(list(map(str, command)), capture_output=True, text=True)
        if result.returncode == 0 or result.stdout:
            raise RuntimeError("Corrupt foreign tree published a snapshot/report")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "ace2016", "ace2019", "sample", "mask2016", "mask2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    cases = (
        (args.ace2016, 5, (*TABLES, "TreeParents", "TreeChildren"), 16, 2, True),
        (args.ace2019, 6, (*TABLES, "TreeParents", "TreeChildren"), 16, 2, True),
        (args.sample, None, ("TAG_GRP_TB", "TAG_NME_TB"), 2, 0, False),
        (args.mask2016, 5, ("Parents", "Children"), 2, 0, False),
        (args.mask2019, 6, ("Parents", "Children"), 2, 0, False),
    )
    for path, code, tables, expected_indexes, minimum_nodes, corrupt_tree in cases:
        original = path.read_bytes()
        if code is not None and original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Expected original ACE 16 family")
        canonical = ""
        index_count = row_count = nodes = 0
        for table in tables:
            indexes = foreign_indexes(args, path, table)
            output = run(args.cli, "inspect", path, "--table", table, "--check")
            if code is not None and "Indexes not checked\t0\n" not in output:
                raise RuntimeError("Generated table check has unsupported coverage: " + table)
            for index, root in indexes.items():
                if index + "\tChecked\tChecked: exact unique/required\n" not in output:
                    raise RuntimeError("Foreign index was not completely checked")
                actual = compare(args, path, table, index)
                canonical += table + "\t" + index + "\n" + actual
                index_count += 1
                row_count += len(actual.splitlines())
                nodes += int(original[root * PAGE_BYTES] == 3)
        if index_count != expected_indexes or nodes < minimum_nodes:
            raise RuntimeError("Expected complete foreign index/node coverage")
        corrupt = 0
        if corrupt_tree:
            ids = catalog_ids(args, path)
            tree_indexes = foreign_indexes(args, path, "TreeChildren")
            index, root = next(iter(tree_indexes.items()))
            cases = []
            for offset, value in ((0, 0), (4, 0), (24, 255)):
                changed = bytearray(original); changed[root * PAGE_BYTES + offset] = value
                cases.append(changed)
            child = logical_offsets(original, ids["TreeChildren"])
            foreign = next(at for at in child.values() if original[at + 23] == 2)
            changed = bytearray(original)
            changed[foreign + 17:foreign + 21] = (0).to_bytes(4, "little")
            cases.append(changed)
            with tempfile.TemporaryDirectory(prefix="tablecraft-foreign-reads-") as directory:
                copy = Path(directory) / "copy.accdb"
                for changed in cases:
                    copy.write_bytes(changed)
                    rejected(args, copy, "TreeChildren", index)
                    corrupt += 1
        if path.read_bytes() != original:
            raise RuntimeError("Original input changed")
        print(path.name, "foreign indexes", index_count, "indexed rows", row_count,
            "node roots", nodes, "corrupt copies", corrupt,
            "original", hashlib.sha256(original).hexdigest(),
            "rows-sha256", hashlib.sha256(canonical.encode()).hexdigest(), flush=True)
    print("Foreign tree/row/constraint checks match independent reads; membership, writes and native Access remain separately qualified.")


if __name__ == "__main__":
    main()
