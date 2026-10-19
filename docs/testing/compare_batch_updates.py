"""Compare complete fixed-field batches, including simultaneous related-table edits."""
import argparse
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

from compare_attachment_columns import fields
from compare_composite_constraints import roots, run
from compare_composite_updates import declarations
from compare_nonunique_updates import PAGE_BYTES, tree_spans
from compare_saved_queries import catalog_ids


@dataclass(frozen=True)
class Cell:
    table: str
    column: str
    row: int
    kind: str
    old: object
    new: object

    def token(self, value):
        if value is None:
            return "null"
        return str(value).lower() if self.kind == "boolean" else str(value)

    def tag(self, value):
        if value is None:
            return "N"
        if self.kind == "boolean":
            return "B" + str(int(value))
        if self.kind == "decimal":
            return "Q" + str(value)
        if self.kind == "guid":
            return "G" + str(value).replace("-", "").lower()
        prefix = {"byte": "U", "int16": "I16:", "int32": "I32:", "int64": "I64:",
                  "currency-scaled": "C", "single-bits": "F", "double-bits": "D", "date-bits": "Y64:"}[self.kind]
        return prefix + str(value).lower()

    def arguments(self):
        return ("--set", self.table, self.column, str(self.row), self.kind, self.token(self.old), self.token(self.new))


GUID = "ffeeddcc-bbaa-9988-7766-554433221100"
COMPOSITE = (
    (Cell("Unique", "A", 0, "int32", 1, 2), Cell("Unique", "B", 0, "int64", 10, 20)),
    (Cell("Parents", "A", 0, "int32", 10, 11), Cell("Parents", "B", 0, "int64", 100, 101),
     Cell("Children", "A", 1, "int32", 10, 11), Cell("Children", "B", 1, "int64", 100, 101)),
    (Cell("Children", "A", 1, "int32", 10, None), Cell("Children", "B", 1, "int64", 100, None)),
    (Cell("Unique", "A", 2, "int32", 2, None), Cell("Unique", "A", 3, "int32", None, 2),
     Cell("Unique", "B", 3, "int64", None, 30)),
    (Cell("Ordinary", "C0", 0, "byte", None, 1), Cell("Ordinary", "C1", 0, "int16", None, -32768),
     Cell("Ordinary", "C2", 0, "int32", None, (1 << 31) - 1), Cell("Ordinary", "C3", 0, "int64", None, -(1 << 63)),
     Cell("Ordinary", "C4", 0, "currency-scaled", None, (1 << 63) - 1)),
    (Cell("Ten", "C0", 0, "byte", None, 0), Cell("Ten", "C5", 0, "byte", None, 255),
     Cell("Ten", "C8", 0, "int64", None, -(1 << 63)), Cell("Ten", "C9", 0, "currency-scaled", None, (1 << 63) - 1)),
    (Cell("Tree", "C0", 1, "byte", 0, None), Cell("Tree", "C1", 1, "int16", -32768, None),
     Cell("Tree", "C4", 1, "currency-scaled", -(1 << 63), None)),
    (Cell("Ordinary", "C3", 3, "int64", 9007199254740992, 9007199254740992),
     Cell("Ten", "C8", 1, "int64", -(1 << 63), -(1 << 63))),
)
MASK = (
    (Cell("Items", "B1", 0, "boolean", False, True), Cell("Items", "Tiny", 0, "byte", 1, None),
     Cell("Items", "Narrow", 0, "int16", -32768, None), Cell("Items", "Large", 0, "int64", -(1 << 63), None),
     Cell("Items", "G", 0, "guid", "00112233-4455-6677-8899-aabbccddeeff", GUID),
     Cell("Items", "B7", 0, "boolean", True, False), Cell("Items", "B8", 0, "boolean", False, True),
     Cell("Items", "B13", 0, "boolean", True, False), Cell("Items", "B14", 0, "boolean", False, True)),
    (Cell("Parents", "Key", 0, "int64", 1, 2), Cell("Children", "ForeignKey", 0, "int64", None, 2),
     Cell("Children", "ForeignKey", 1, "int64", 1, 2)),
    (Cell("Items", "Tiny", 1, "byte", None, 0), Cell("Items", "Narrow", 1, "int16", None, -32768),
     Cell("Items", "Large", 1, "int64", None, (1 << 63) - 1), Cell("Items", "G", 1, "guid", None, GUID),
     Cell("Items", "B13", 1, "boolean", True, False), Cell("Items", "S", 1, "single-bits", None, "3f800001"),
     Cell("Items", "D", 1, "double-bits", None, "3ff0000000000001"), Cell("Items", "Moment", 1, "date-bits", None, "4004000000000000")),
)


def compare(args, profiles, request=lambda cells: cells, cascade=False):
    with tempfile.TemporaryDirectory(prefix="tablecraft-batch-oracle-") as classes:
        run("javac", "-cp", args.jar, "-d", classes, "docs/testing/TableDataOracle.java")
        def oracle(path, *arguments):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar) + os.pathsep + classes,
                       "TableDataOracle", path, "--date-bits", *arguments)
        for path, code, cases, invalid in profiles:
            original = path.read_bytes()
            if original[20:24] != bytes([code, 0, 0, 0]):
                raise RuntimeError("Expected original ACE 16 family")
            ids, cache = catalog_ids(args, path), {}
            names = {cell.table for case in cases for cell in case}
            for table in names:
                before = oracle(path, table)
                if run(args.rows, path, table, "--date-bits") != before:
                    raise RuntimeError("Original exact values differ")
                layout = fields(original, ids[table])
                cache[table] = before, layout, roots(original, ids[table]), declarations(args, path, table, layout, original)
            def command(output, cells):
                return [str(args.preview), str(path), str(output), *(("--cascade-updates",) if cascade else ()), *(part for cell in request(cells) for part in cell.arguments())]
            hashes, noops = [], 0
            with tempfile.TemporaryDirectory(prefix="tablecraft-batch-candidates-") as directory:
                for number, cells in enumerate(cases):
                    output = Path(directory) / f"candidate-{number}.accdb"
                    changed = {(cell.table, cell.row) for cell in cells if cell.tag(cell.old) != cell.tag(cell.new)}
                    result = subprocess.run(command(output, cells), capture_output=True, text=True)
                    if result.returncode or result.stdout != f"candidate_changed_rows={len(changed)}\n" or "qualification pending" not in result.stderr:
                        raise RuntimeError("Batch failed: " + result.stderr)
                    after = output.read_bytes()
                    allowed, masks = [], {}
                    for table in names:
                        before, layout, records, indexes = cache[table]
                        expected = [line.split("\t") for line in before.splitlines()]
                        selected = {cell.column for cell in cells if cell.table == table and cell.tag(cell.old) != cell.tag(cell.new)}
                        for cell in (cell for cell in cells if cell.table == table):
                            position = list(layout).index(cell.column) + 1
                            if expected[cell.row][position] != cell.tag(cell.old):
                                raise RuntimeError("Expected original batch carrier differs")
                            expected[cell.row][position] = cell.tag(cell.new)
                            if cell.tag(cell.old) == cell.tag(cell.new):
                                continue
                            at, _, column, variable = layout[cell.column]
                            width = 0 if original[at] == 1 else int.from_bytes(original[at + 23:at + 25], "little")
                            offset = int.from_bytes(original[at + 21:at + 23], "little")
                            row, end = records[cell.row]
                            count = int.from_bytes(original[row:row + 2], "little")
                            mask = end - (count + 7) // 8 + column // 8
                            masks[mask] = masks.get(mask, 0) | (1 << (column % 8))
                            if cell.new is not None and width:
                                if original[at] == 16:
                                    mask_start = end - (count + 7) // 8
                                    count_at = mask_start - 2
                                    variables = int.from_bytes(original[count_at:count_at + 2], "little")
                                    if variable >= variables:
                                        raise RuntimeError("Missing original Decimal slot")
                                    first = count_at - 2 * (variable + 1)
                                    last = first - 2
                                    start = row + int.from_bytes(original[first:first + 2], "little")
                                    finish = row + int.from_bytes(original[last:last + 2], "little")
                                    if finish - start != width or not row <= start <= finish <= count_at - 2 * (variables + 1):
                                        raise RuntimeError("Decimal requires original full-width slot")
                                    allowed.append(range(start, finish))
                                else:
                                    allowed.append(range(row + 2 + offset, row + 2 + offset + width))
                        expected = "".join("\t".join(row) + "\n" for row in expected)
                        if oracle(output, table) != expected or run(args.rows, output, table, "--date-bits") != expected:
                            raise RuntimeError("Batch exact values or unrelated/long values differ")
                        current, previous = {}, {}
                        for label, (root, columns) in indexes.items():
                            if oracle(output, "--index", table, label) != run(args.rows, output, table, "--date-bits", "--index", label):
                                raise RuntimeError("Batch index/key/row-reference order differs")
                            if not selected.intersection(name for name, _ in columns):
                                continue
                            widths = [width for _, width in columns]
                            _, old_refs, _ = tree_spans(original, root, previous, key_width=widths)
                            _, new_refs, _ = tree_spans(after, root, current, key_width=widths)
                            if set(old_refs) != set(new_refs):
                                raise RuntimeError("Batch physical row references changed")
                        if set(previous) != set(current):
                            raise RuntimeError("Batch allocated index pages changed")
                        for page, state in previous.items():
                            current_state = current[page]
                            if state["count"] != current_state["count"] or state["children"] != current_state["children"]:
                                raise RuntimeError("Batch topology or entry counts changed")
                            at = page * PAGE_BYTES
                            allowed.extend((range(at + 2, at + 4), range(at + 24, at + 26),
                                            range(at + 27, at + max(state["end"], current_state["end"]))))
                        report = run(args.checker, output, table)
                        if report != f"rows={len(before.splitlines())}\nindexes_checked={len(indexes)}\nqualified_constraints=TRUE\n":
                            raise RuntimeError("Incomplete final batch validation")
                    for mask, bits in masks.items():
                        if (original[mask] ^ after[mask]) & ~bits:
                            raise RuntimeError("Unselected row-mask bits changed")
                        allowed.append(range(mask, mask + 1))
                    if len(original) != len(after) or any(a != b and not any(at in span for span in allowed)
                            for at, (a, b) in enumerate(zip(original, after))):
                        raise RuntimeError("Bytes outside selected field/mask/index spans changed")
                    if not changed:
                        if original != after:
                            raise RuntimeError("No-op batch changed image")
                        noops += 1
                    reverse = Path(directory) / f"reverse-{number}.accdb"
                    reordered = subprocess.run(command(reverse, tuple(reversed(cells))), capture_output=True, text=True)
                    if reordered.returncode or reverse.read_bytes() != after or reordered.stdout != result.stdout:
                        raise RuntimeError("Batch order changed candidate/counts")
                    repeated = subprocess.run(command(output, cells), capture_output=True)
                    if repeated.returncode == 0 or repeated.stdout or output.read_bytes() != after:
                        raise RuntimeError("Existing batch output overwritten")
                    hashes.append(hashlib.sha256(after).hexdigest())
                for number, (cells, reason) in enumerate(invalid):
                    output = Path(directory) / f"rejected-{number}.accdb"
                    result = subprocess.run(command(output, cells), capture_output=True)
                    if result.returncode == 0 or result.stdout or output.exists() or reason not in result.stderr:
                        raise RuntimeError("Expected atomic batch rejection is missing")
            if path.read_bytes() != original:
                raise RuntimeError("Original batch input changed")
            print(path.name, "candidates", len(hashes), "noops", noops, "rejections", len(invalid),
                  "original", hashlib.sha256(original).hexdigest(), "candidate-manifest",
                  hashlib.sha256("\n".join(hashes).encode()).hexdigest(), flush=True)
    print("Complete batch values, index topology and preserved bytes match; native Access remains unqualified.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "preview", "checker", "ace2016", "ace2019", "mask2016", "mask2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    composite_invalid = ((COMPOSITE[1][:2], b"MissingParent"),
        ((Cell("Unique", "B", 1, "int64", 20, 10), COMPOSITE[4][0]), b"DuplicateKey"),
        ((COMPOSITE[0][0], Cell("Unique", "B", 0, "int64", 99, 20)), b"Conflict"),
        ((COMPOSITE[0][0], COMPOSITE[0][0]), b"Conflict"))
    mask_invalid = ((MASK[1][:1], b"MissingParent"),
        ((MASK[0][0], Cell("Items", "MustHave", 0, "int32", 12, -1)), b"FieldRule"),
        ((MASK[0][0], Cell("Items", "Large", 0, "int32", None, 1)), b"Column"))
    compare(args, ((args.ace2016, 5, COMPOSITE, composite_invalid), (args.ace2019, 6, COMPOSITE, composite_invalid),
                   (args.mask2016, 5, MASK, mask_invalid), (args.mask2019, 6, MASK, mask_invalid)))


if __name__ == "__main__":
    main()
