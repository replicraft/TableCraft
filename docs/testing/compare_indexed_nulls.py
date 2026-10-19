"""Compare indexed-NULL candidates, page geometry and unchanged unrelated bytes."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_attachment_columns import fields
from compare_composite_constraints import roots, run
from compare_nonunique_updates import tree_spans, oracle, PAGE_BYTES, INDEXES
from compare_saved_queries import catalog_ids


CASES = (("Ordinary", 0, None, 10), ("Ordinary", 2, 20, None),
    ("Ordinary", 4, 30, None), ("Ordinary", 6, None, 0),
    ("Ordinary", 0, None, None), ("Ordinary", 2, 20, 20),
    ("Tree", 0, None, 30), ("Tree", 1, 1, None), ("Tree", 998, 6, None),
    ("Tree", 16, 16, None), ("Tree", 17, None, 7),
    ("Unique", 0, None, None), ("Unique", 1, 10, None),
    ("Unique", 2, None, 40), ("Unique", 3, 20, None))


def token(value):
    return "null" if value is None else str(value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "preview", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path, code in ((args.ace2016, 5), (args.ace2019, 6)):
        original = path.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Unexpected original ACE family")
        ids = catalog_ids(args, path)
        before, row_roots, layouts, index_roots, original_pages = {}, {}, {}, {}, {}
        for table in ("Ordinary", "Tree", "Unique"):
            before[table] = oracle(args, path, table)
            if run(args.rows, path, table) != before[table]:
                raise RuntimeError("Original complete rows differ")
            row_roots[table] = roots(original, ids[table])
            layouts[table] = fields(original, ids[table])["Score"]
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes").splitlines()[2:]
            index_roots[table] = {line.split("\t")[0]: int(line.split("\t")[3]) for line in metadata if line.split("\t")[0] in INDEXES}
            original_pages[table] = {}
            for root in index_roots[table].values():
                tree_spans(original, root, original_pages[table])
        hashes = []
        with tempfile.TemporaryDirectory(prefix="tablecraft-indexed-null-") as directory:
            for number, (table, ordinal, old, new) in enumerate(CASES):
                output = Path(directory) / f"candidate-{number}.accdb"
                command = list(map(str, (args.preview, path, output, table, "Score", ordinal, "int32", token(old), token(new))))
                result = subprocess.run(command, capture_output=True, text=True)
                if result.returncode or result.stdout != f"candidate_changed_rows={int(old != new)}\n" or "qualification pending" not in result.stderr:
                    raise RuntimeError("Candidate failed: " + result.stderr)
                after = output.read_bytes()
                expected = [line.split("\t") for line in before[table].splitlines()]
                if expected[ordinal][2] != ("N" if old is None else "I32:" + str(old)):
                    raise RuntimeError("Wrong expected original carrier")
                expected[ordinal][2] = "N" if new is None else "I32:" + str(new)
                expected = "".join("\t".join(row) + "\n" for row in expected)
                if oracle(args, output, table) != expected or run(args.rows, output, table) != expected:
                    raise RuntimeError("Candidate rows/long values differ")
                current = {}
                for index, root in index_roots[table].items():
                    if oracle(args, output, table, index) != run(args.rows, output, table, "--index", index):
                        raise RuntimeError("Nullable key/reference order differs")
                    _, old_refs, _ = tree_spans(original, root)
                    _, new_refs, _ = tree_spans(after, root, current)
                    if set(old_refs) != set(new_refs):
                        raise RuntimeError("Physical row-reference set changed")
                if set(current) != set(original_pages[table]):
                    raise RuntimeError("Existing page set changed")
                allowed = []
                for page, state in original_pages[table].items():
                    new_state = current[page]
                    if state["count"] != new_state["count"] or state["children"] != new_state["children"]:
                        raise RuntimeError("Entry counts or child links changed")
                    start = page * PAGE_BYTES
                    allowed.extend((range(start + 2, start + 4), range(start + 24, start + 26),
                        range(start + 27, start + max(state["end"], new_state["end"]))))
                field, _, column, _ = layouts[table]
                offset = int.from_bytes(original[field + 21:field + 23], "little")
                row, end = row_roots[table][ordinal]
                maximum = int.from_bytes(original[row:row + 2], "little")
                mask = end - (maximum + 7) // 8 + column // 8
                bit = 1 << (column % 8)
                if (original[mask] ^ after[mask]) & ~bit:
                    raise RuntimeError("Neighbor presence bits changed")
                allowed.append(range(mask, mask + 1))
                if new is not None:
                    allowed.append(range(row + 2 + offset, row + 6 + offset))
                if len(after) != len(original) or any(a != b and not any(i in span for span in allowed)
                        for i, (a, b) in enumerate(zip(original, after))):
                    raise RuntimeError("Bytes outside exact field/mask/page regions changed")
                if old == new and after != original:
                    raise RuntimeError("No-op changed image")
                report = run(args.cli, "inspect", output, "--table", table, "--check")
                if "Indexes not checked\t0\n" not in report:
                    raise RuntimeError("Candidate checks incomplete")
                retained = subprocess.run(command, capture_output=True)
                if retained.returncode == 0 or retained.stdout or output.read_bytes() != after:
                    raise RuntimeError("Existing output replaced")
                hashes.append(hashlib.sha256(after).hexdigest())
            rejected = (("Unique", "Score", 0, "null", "20"), ("Required", "Score", 0, "10", "null"),
                ("Rules", "Score", 0, "10", "null"), ("Ordinary", "Score", 0, "0", "10"),
                ("Ordinary", "Score", 2, "null", "10"), ("Ordinary", "Score", 99999, "null", "7"),
                ("Ordinary", "Id", 0, "70", "null"))
            for number, (table, column, ordinal, old, new) in enumerate(rejected):
                output = Path(directory) / f"rejected-{number}.accdb"
                result = subprocess.run(list(map(str, (args.preview, path, output, table, column, ordinal, "int32", old, new))), capture_output=True, text=True)
                if result.returncode == 0 or result.stdout or output.exists():
                    raise RuntimeError("Invalid indexed NULL request published output")
                if number < 3 and ("DuplicateKey", "NullKey", "FieldRule")[number] not in result.stderr:
                    raise RuntimeError("Expected native index/field rule guard missing")
        if path.read_bytes() != original:
            raise RuntimeError("Original input changed")
        print(path.name, "candidates", len(hashes), "noops", 3, "rejections", len(rejected),
            "original", hashlib.sha256(original).hexdigest(), "candidate-manifest", hashlib.sha256("\n".join(hashes).encode()).hexdigest(), flush=True)
    print("Indexed NULL carriers, prefixes/bitmaps/free space, unchanged topology and full independent rows match; native Access remains unqualified.")


if __name__ == "__main__":
    main()
