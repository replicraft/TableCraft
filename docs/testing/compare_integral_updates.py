"""Compare exact indexed integral candidates with independent rows and page proofs."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_attachment_columns import fields
from compare_composite_constraints import roots, run
from compare_nonunique_updates import INDEXES, PAGE_BYTES, oracle, tree_spans
from compare_saved_queries import catalog_ids


KINDS = (("BYTE", "byte", 1, "U", 0, 255),
         ("INT", "int16", 2, "I16:", -32768, 32767),
         ("BIG_INT", "int64", 8, "I64:", -(1 << 63), (1 << 63) - 1),
         ("MONEY", "currency-scaled", 8, "C", -(1 << 63), (1 << 63) - 1))


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
            raise RuntimeError("Expected original ACE 16 family")
        ids = catalog_ids(args, path)
        hashes, rejections, noops = [], 0, 0
        with tempfile.TemporaryDirectory(prefix="tablecraft-integral-updates-") as directory:
            for name, kind, width, tag, minimum, maximum in KINDS:
                adjacent = 9007199254740992 if width == 8 else 32
                cases = (("Leaves", 0, None, minimum), ("Leaves", 1, minimum, maximum),
                         ("Leaves", 2, maximum, None), ("Leaves", 6, adjacent + (1 if width == 8 else -1), adjacent),
                         ("Leaves", 4, 20, 20), ("Tree", 0, None, 30), ("Tree", 1, 1, None))
                cached = {}
                for suffix in ("Leaves", "Tree"):
                    table = name + suffix
                    before = oracle(args, path, table)
                    if run(args.rows, path, table) != before:
                        raise RuntimeError("Original exact rows/long values differ")
                    metadata = run(args.cli, "inspect", path, "--table", table, "--indexes").splitlines()[2:]
                    selected = {cells[0]: int(cells[3]) for line in metadata if (cells := line.split("\t"))[0] in INDEXES}
                    if set(selected) != set(INDEXES):
                        raise RuntimeError("Missing original integral indexes")
                    pages = {}
                    for root in selected.values():
                        tree_spans(original, root, pages, key_width=width)
                        if suffix == "Tree" and original[root * PAGE_BYTES] != 3:
                            raise RuntimeError("Expected original multi-page node")
                    cached[table] = before, selected, pages, roots(original, ids[table]), fields(original, ids[table])["Score"]
                for suffix, ordinal, old, new in cases:
                    table = name + suffix
                    before, selected, pages, rows, layout = cached[table]
                    output = Path(directory) / f"{name}-{suffix}-{ordinal}.accdb"
                    command = list(map(str, (args.preview, path, output, table, "Score", ordinal, kind, token(old), token(new))))
                    result = subprocess.run(command, capture_output=True, text=True)
                    if result.returncode or result.stdout != f"candidate_changed_rows={int(old != new)}\n" or "qualification pending" not in result.stderr:
                        raise RuntimeError("Candidate failed: " + result.stderr)
                    after = output.read_bytes()
                    expected = [line.split("\t") for line in before.splitlines()]
                    if expected[ordinal][2] != ("N" if old is None else tag + str(old)):
                        raise RuntimeError("Unexpected original exact carrier")
                    expected[ordinal][2] = "N" if new is None else tag + str(new)
                    expected = "".join("\t".join(row) + "\n" for row in expected)
                    if oracle(args, output, table) != expected or run(args.rows, output, table) != expected:
                        raise RuntimeError("Candidate exact values/long values differ")
                    current = {}
                    for index, root in selected.items():
                        if oracle(args, output, table, index) != run(args.rows, output, table, "--index", index):
                            raise RuntimeError("Exact indexed key/reference order differs")
                        _, old_refs, _ = tree_spans(original, root, key_width=width)
                        _, new_refs, _ = tree_spans(after, root, current, key_width=width)
                        if set(old_refs) != set(new_refs):
                            raise RuntimeError("Physical row references changed")
                    if set(current) != set(pages):
                        raise RuntimeError("Tree page allocation changed")
                    allowed = []
                    for page, state in pages.items():
                        changed = current[page]
                        if state["count"] != changed["count"] or state["children"] != changed["children"]:
                            raise RuntimeError("Node topology or entry counts changed")
                        start = page * PAGE_BYTES
                        allowed.extend((range(start + 2, start + 4), range(start + 24, start + 26),
                                        range(start + 27, start + max(state["end"], changed["end"]))))
                    field, _, column, _ = layout
                    offset = int.from_bytes(original[field + 21:field + 23], "little")
                    row, end = rows[ordinal]
                    maximum_columns = int.from_bytes(original[row:row + 2], "little")
                    mask = end - (maximum_columns + 7) // 8 + column // 8
                    if (original[mask] ^ after[mask]) & ~(1 << (column % 8)):
                        raise RuntimeError("Neighbor presence bits changed")
                    allowed.append(range(mask, mask + 1))
                    if new is not None:
                        allowed.append(range(row + 2 + offset, row + 2 + offset + width))
                    if len(original) != len(after) or any(a != b and not any(at in span for span in allowed)
                            for at, (a, b) in enumerate(zip(original, after))):
                        raise RuntimeError("Bytes outside admitted field/mask/index regions changed")
                    if old == new:
                        if original != after:
                            raise RuntimeError("No-op changed image")
                        noops += 1
                    report = run(args.cli, "inspect", output, "--table", table, "--check")
                    if "Indexes not checked\t0\n" not in report:
                        raise RuntimeError("Candidate index checks incomplete")
                    repeated = subprocess.run(command, capture_output=True)
                    if repeated.returncode == 0 or repeated.stdout or output.read_bytes() != after:
                        raise RuntimeError("Existing output overwritten")
                    hashes.append(hashlib.sha256(after).hexdigest())
                for suffix, ordinal, old, new in (("Unique", 1, 10, 20), ("Unique", 0, None, 20),
                                                  ("Required", 0, 10, None), ("Leaves", 1, maximum, 20)):
                    output = Path(directory) / f"reject-{rejections}.accdb"
                    command = list(map(str, (args.preview, path, output, name + suffix, "Score", ordinal, kind, token(old), token(new))))
                    result = subprocess.run(command, capture_output=True)
                    if result.returncode == 0 or result.stdout or output.exists():
                        raise RuntimeError("Invalid integral request published output")
                    rejections += 1
                print(path.name, name, "exact candidates and rejections match", flush=True)
        if path.read_bytes() != original:
            raise RuntimeError("Original input changed")
        print(path.name, "candidates", len(hashes), "noops", noops, "rejections", rejections,
              "original", hashlib.sha256(original).hexdigest(), "candidate-manifest",
              hashlib.sha256("\n".join(hashes).encode()).hexdigest(), flush=True)
    print("Integral/Currency values, indexes, page topology and preserved bytes match; native Access remains unqualified.")


if __name__ == "__main__":
    main()
