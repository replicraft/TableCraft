"""Compare existing-tree Jet 4 updates with independent Jackcess row/index reads."""

from cli_tsv import display_tsv
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile


PAGE_BYTES = 4096
ENTRY_START = 480
MASK_START = 27


def tree_spans(image, root):
    """Independently decode original uncompressed Int32 nodes/leaves and row refs."""
    spans, references, visited = [], {}, set()

    def visit(number):
        if number in visited:
            raise RuntimeError("Repeated original page")
        visited.add(number)
        start = number * PAGE_BYTES
        page = image[start:start + PAGE_BYTES]
        if page[0] not in (3, 4) or page[1] != 1 or page[24:26] != bytes(2):
            raise RuntimeError("Expected original uncompressed index page")
        end = PAGE_BYTES - int.from_bytes(page[2:4], "little")
        spans.append(range(start + ENTRY_START, start + end))
        positions = [i for i in range(PAGE_BYTES - ENTRY_START + 1)
                     if page[MASK_START + i // 8] & (1 << (i % 8))]
        leaf = page[0] == 4
        previous = 0
        for position in positions:
            entry = page[ENTRY_START + previous:ENTRY_START + position]
            if len(entry) != (9 if leaf else 13) or entry[0] not in (0x7f, 0x80):
                raise RuntimeError("Unexpected original key entry")
            raw = bytes(byte ^ (0xff if entry[0] == 0x80 else 0) for byte in entry[1:5])
            value = int.from_bytes(raw, "big") ^ 0x80000000
            if value >= 1 << 31:
                value -= 1 << 32
            reference = int.from_bytes(entry[5:9], "big")
            if leaf:
                if value in references:
                    raise RuntimeError("Duplicate original key")
                references[value] = reference
            else:
                visit(int.from_bytes(entry[9:13], "big"))
            previous = position
        if not leaf:
            visit(int.from_bytes(page[20:24], "little"))

    visit(root)
    if len(visited) < 3 or len(references) != 1000:
        raise RuntimeError("Expected a complete 1000-row multi-page tree")
    return spans, references


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "jet", "cli", "example"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    original = args.jet.read_bytes()

    def java(source, *arguments):
        return subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            "docs/testing/" + source + ".java", *map(str, arguments)], text=True)

    physical = [line.split("\t") for line in java("IndexUpdateFixture", "--inspect", args.jet).splitlines()]
    if len(physical) != 1000 or any((int(row[0]), int(row[1])) != (2 * (i + 1), i) for i, row in enumerate(physical)):
        raise RuntimeError("Unexpected original recipe rows")
    metadata = display_tsv(subprocess.check_output([str(args.cli), "inspect", str(args.jet), "--table", "Items", "--indexes"], text=True))
    roots = [int(line.split("\t")[3]) for line in metadata.splitlines() if line.startswith(("PK_Items\t", "IdDesc\t"))]
    if len(roots) != 2:
        raise RuntimeError("Expected both independent index definitions")
    trees = [tree_spans(original, root) for root in roots]
    if trees[0][1] != trees[1][1]:
        raise RuntimeError("Original index root references disagree")

    def check(path, replacements):
        expected = [row.copy() for row in physical]
        for ordinal, value in replacements.items():
            expected[ordinal][0] = str(value)
        observed = [line.split("\t") for line in java("IndexUpdateFixture", "--inspect", path).splitlines()]
        if observed != expected:
            raise RuntimeError("Independent row/memo/binary reopen differs")
        ordered = sorted((int(row[0]), int(row[1])) for row in expected)
        for name, reverse in (("PK_Items", False), ("IdDesc", True)):
            result = java("IndexRowsOracle", path, "Items", name, "Id", "Ordinal")
            pairs = [tuple(map(int, row.split("\t"))) for row in result.splitlines()]
            if pairs != sorted(ordered, reverse=reverse):
                raise RuntimeError("Independent index key/reference order differs")
        after = path.read_bytes()
        allowed = [span for spans, _ in trees for span in spans]
        for ordinal in replacements:
            original_key = int(physical[ordinal][0])
            reference = trees[0][1][original_key]
            page, slot = reference >> 8, reference & 255
            position = page * PAGE_BYTES + 14 + slot * 2
            field = page * PAGE_BYTES + (int.from_bytes(original[position:position + 2], "little") & 0x1fff) + 2
            if int.from_bytes(original[field:field + 4], "little", signed=True) != original_key:
                raise RuntimeError("Independent physical field binding differs")
            allowed.append(range(field, field + 4))
        if len(after) != len(original) or any(a != b and not any(offset in span for span in allowed)
                for offset, (a, b) in enumerate(zip(original, after))):
            raise RuntimeError("Bytes changed outside original fields and entry spans")
        subprocess.run([str(args.cli), "inspect", str(path), "--table", "Items", "--check"],
            check=True, stdout=subprocess.DEVNULL)
        return hashlib.sha256(after).hexdigest()

    with tempfile.TemporaryDirectory(prefix="tablecraft-index-tree-updates-") as directory:
        hashes = []
        for number, (ordinal, value) in enumerate(((0, 2001), (999, -1), (499, 1999))):
            output = Path(directory) / f"native-{number}.mdb"
            expected = 2 * (ordinal + 1)
            subprocess.run([str(args.example), str(args.jet), str(output), "Items", "Id", str(ordinal), str(expected), str(value)], check=True)
            hashes.append(check(output, {ordinal: value}))
            cli_output = Path(directory) / f"cli-{number}.mdb"
            report = subprocess.check_output([str(args.cli), "query", "update", f"UPDATE Items SET Id={value} WHERE Id={expected}",
                "--database", str(args.jet), "--output", str(cli_output)], text=True)
            if report != "Matched rows\t1\nChanged rows\t1\n" or cli_output.read_bytes() != output.read_bytes():
                raise RuntimeError("Native and CLI copies disagree")
        output = Path(directory) / "reversed.mdb"
        report = subprocess.check_output([str(args.cli), "query", "update", "UPDATE Items SET Id=2002-Id",
            "--database", str(args.jet), "--output", str(output)], text=True)
        if report != "Matched rows\t1000\nChanged rows\t1000\n":
            raise RuntimeError("Whole-keyspace swap did not complete atomically")
        hashes.append(check(output, {i: 2000 - 2 * i for i in range(1000)}))
        rejected = Path(directory) / "duplicate.mdb"
        result = subprocess.run([str(args.cli), "query", "update", "UPDATE Items SET Id=2000 WHERE Id=2",
            "--database", str(args.jet), "--output", str(rejected)], capture_output=True)
        if result.returncode == 0 or rejected.exists() or result.stdout:
            raise RuntimeError("Cross-leaf duplicate published output")
        print("1000-row ascending/descending existing-tree updates, keyspace swap, exact byte/memo/binary preservation and duplicate rejection passed")
        print("before", hashlib.sha256(original).hexdigest())
        print("after", *hashes)
    if args.jet.read_bytes() != original:
        raise RuntimeError("Original fixture changed")


if __name__ == "__main__":
    main()
