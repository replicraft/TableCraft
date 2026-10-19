"""Compare complete composite index candidates, tuple NULLs and preserved bytes."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

from compare_attachment_columns import fields
from compare_composite_constraints import roots, run
from compare_nonunique_updates import PAGE_BYTES, GUID_PAYLOAD_BYTES, tree_spans
from compare_saved_queries import catalog_ids


CASES = (("Ordinary", "C0", 0, "byte", None, 255, "U"),
         ("Ordinary", "C1", 1, "int16", -32768, 32767, "I16:"),
         ("Ordinary", "C2", 2, "int32", (1 << 31) - 1, None, "I32:"),
         ("Ordinary", "C3", 3, "int64", 9007199254740992, 9007199254740993, "I64:"),
         ("Ordinary", "C4", 3, "currency-scaled", 9007199254740993, 9007199254740992, "C"),
         ("Ordinary", "C3", 3, "int64", 9007199254740992, 9007199254740992, "I64:"),
         ("Tree", "C0", 1, "byte", 0, None, "U"),
         ("Tree", "C3", 0, "int64", None, -(1 << 63), "I64:"),
         ("Tree", "C4", 3, "currency-scaled", 9007199254740993, None, "C"),
         ("Ten", "C9", 0, "currency-scaled", None, (1 << 63) - 1, "C"),
         ("Ten", "C8", 1, "int64", -(1 << 63), None, "I64:"),
         ("Ten", "C0", 1, "byte", 0, 255, "U"),
         ("Unique", "A", 2, "int32", 2, None, "I32:"),
         ("Unique", "B", 0, "int64", 10, None, "I64:"),
         ("Unique", "A", 3, "int32", None, None, "I32:"),
         ("Children", "A", 1, "int32", 10, 20, "I32:"),
         ("Children", "B", 1, "int64", 100, 200, "I64:"),
         ("Parents", "A", 2, "int32", 30, 40, "I32:"))
REJECTIONS = (("Unique", "B", 1, "int64", 20, 10, b"DuplicateKey"),
              ("Required", "B", 0, "int64", 10, None, b"NullKey"),
              ("Parents", "A", 0, "int32", 10, 99, b"MissingParent"),
              ("Children", "A", 1, "int32", 10, 99, b"MissingParent"),
              ("Children", "A", 0, "int32", None, 10, b"MissingParent"),
              ("Parents", "A", 0, "int32", 10, None, b"NullKey"),
              ("Ordinary", "C3", 3, "int64", 99, 1, b"Conflict"))


def token(value):
    return "null" if value is None else str(value)


def declarations(args, path, table, layout, original):
    lines = run(args.cli, "inspect", path, "--table", table, "--indexes").splitlines()
    labels, indexes = lines[1].split("\t"), {}
    for line in lines[2:]:
        cells = line.split("\t")
        root, columns = indexes.setdefault(cells[0], (int(cells[labels.index("Root page")]), []))
        name = cells[labels.index("Column")]
        at = layout[name][0]
        width = int.from_bytes(original[at + 23:at + 25], "little")
        columns.append((name, GUID_PAYLOAD_BYTES if original[at] == 15 else width))
    return indexes


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "preview", "checker", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="tablecraft-tuple-oracle-") as classes:
        run("javac", "-cp", args.jar, "-d", classes, "docs/testing/TableDataOracle.java")
        def oracle(path, *arguments):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar) + os.pathsep + classes,
                       "TableDataOracle", path, *arguments)
        for path, code in ((args.ace2016, 5), (args.ace2019, 6)):
            original = path.read_bytes()
            if original[20:24] != bytes([code, 0, 0, 0]):
                raise RuntimeError("Expected ACE 16 family")
            ids, cache = catalog_ids(args, path), {}
            for table in sorted({case[0] for case in CASES} | {case[0] for case in REJECTIONS}):
                before = oracle(path, table)
                if run(args.rows, path, table) != before:
                    raise RuntimeError("Original tuple rows/long values differ")
                layout = fields(original, ids[table])
                cache[table] = before, layout, roots(original, ids[table]), declarations(args, path, table, layout, original)
            hashes, noops = [], 0
            with tempfile.TemporaryDirectory(prefix="tablecraft-composite-update-") as directory:
                for table, field, ordinal, kind, old, new, tag in CASES:
                    before, layout, records, indexes = cache[table]
                    output = Path(directory) / f"candidate-{len(hashes)}.accdb"
                    command = list(map(str, (args.preview, path, output, table, field, ordinal, kind, token(old), token(new))))
                    result = subprocess.run(command, capture_output=True, text=True)
                    if result.returncode or result.stdout != f"candidate_changed_rows={int(old != new)}\n" or "qualification pending" not in result.stderr:
                        raise RuntimeError("Tuple candidate failed: " + table + "/" + field + " " + result.stderr)
                    after = output.read_bytes()
                    current, previous, allowed = {}, {}, []
                    for label, (root, columns) in indexes.items():
                        if oracle(output, "--index", table, label) != run(args.rows, output, table, "--index", label):
                            raise RuntimeError("Tuple/index/row-reference order differs")
                        if field not in [name for name, _ in columns]:
                            continue
                        widths = [width for _, width in columns]
                        _, old_refs, _ = tree_spans(original, root, previous, key_width=widths)
                        _, new_refs, _ = tree_spans(after, root, current, key_width=widths)
                        if set(old_refs) != set(new_refs):
                            raise RuntimeError("Tuple physical row references changed")
                        if table == "Tree" and original[root * PAGE_BYTES] != 3:
                            raise RuntimeError("Expected original multi-page tuple tree")
                    if not previous or set(previous) != set(current):
                        raise RuntimeError("Tuple page allocation changed")
                    for page, state in previous.items():
                        changed = current[page]
                        if state["children"] != changed["children"] or state["count"] != changed["count"]:
                            raise RuntimeError("Tuple child links or per-page counts changed")
                        at = page * PAGE_BYTES
                        allowed.extend((range(at + 2, at + 4), range(at + 24, at + 26),
                                        range(at + 27, at + max(state["end"], changed["end"]))))
                    at, _, column, _ = layout[field]
                    width = int.from_bytes(original[at + 23:at + 25], "little")
                    offset = int.from_bytes(original[at + 21:at + 23], "little")
                    row, end = records[ordinal]
                    count = int.from_bytes(original[row:row + 2], "little")
                    mask = end - (count + 7) // 8 + column // 8
                    if (original[mask] ^ after[mask]) & ~(1 << (column % 8)):
                        raise RuntimeError("Neighbor mask bits changed")
                    allowed.append(range(mask, mask + 1))
                    if new is not None:
                        allowed.append(range(row + 2 + offset, row + 2 + offset + width))
                    if len(after) != len(original) or any(a != b and not any(at in span for span in allowed)
                            for at, (a, b) in enumerate(zip(original, after))):
                        raise RuntimeError("Bytes outside admitted tuple/field/mask spans changed")
                    related = ("Parents", "Children") if table in ("Parents", "Children") else (table,)
                    for name in related:
                        expected = cache[name][0]
                        if name == table:
                            lines = [line.split("\t") for line in expected.splitlines()]
                            position = list(layout).index(field) + 1
                            if lines[ordinal][position] != ("N" if old is None else tag + str(old)):
                                raise RuntimeError("Unexpected original tuple carrier")
                            lines[ordinal][position] = "N" if new is None else tag + str(new)
                            expected = "".join("\t".join(line) + "\n" for line in lines)
                        if oracle(output, name) != expected or run(args.rows, output, name) != expected:
                            raise RuntimeError("Exact tuples, related rows or long values differ")
                    report = run(args.checker, output, table)
                    expected_report = f"rows={len(before.splitlines())}\nindexes_checked={len(indexes)}\nqualified_constraints=TRUE\n"
                    if report != expected_report:
                        raise RuntimeError("Incomplete tuple constraint coverage")
                    if old == new:
                        if original != after:
                            raise RuntimeError("Tuple no-op changed image")
                        noops += 1
                    retained = subprocess.run(command, capture_output=True)
                    if retained.returncode == 0 or retained.stdout or output.read_bytes() != after:
                        raise RuntimeError("Existing tuple output overwritten")
                    hashes.append(hashlib.sha256(after).hexdigest())
                for number, (table, field, ordinal, kind, old, new, reason) in enumerate(REJECTIONS):
                    output = Path(directory) / f"rejected-{number}.accdb"
                    result = subprocess.run(list(map(str, (args.preview, path, output, table, field, ordinal, kind, token(old), token(new)))), capture_output=True)
                    if result.returncode == 0 or result.stdout or output.exists() or reason not in result.stderr:
                        raise RuntimeError("Expected tuple rejection is missing")
            if path.read_bytes() != original:
                raise RuntimeError("Original tuple input changed")
            print(path.name, "candidates", len(hashes), "noops", noops, "rejections", len(REJECTIONS),
                  "original", hashlib.sha256(original).hexdigest(), "candidate-manifest",
                  hashlib.sha256("\n".join(hashes).encode()).hexdigest(), flush=True)
    print("Exact composite candidates, related rows and unchanged topology match; native Access remains unqualified.")


if __name__ == "__main__":
    main()
