"""Compare checked reciprocal-index candidates and preserve related tables/bytes."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

from compare_attachment_columns import fields
from compare_composite_constraints import roots, run
from compare_nonunique_updates import PAGE_BYTES, tree_spans
from compare_saved_queries import catalog_ids


KINDS = (("BYTE", "byte", 1, "U", 0, 255),
         ("INT", "int16", 2, "I16:", -32768, 32767),
         ("LONG", "int32", 4, "I32:", -(1 << 31), (1 << 31) - 1),
         ("BIG_INT", "int64", 8, "I64:", -(1 << 63), (1 << 63) - 1),
         ("MONEY", "currency-scaled", 8, "C", -(1 << 63), (1 << 63) - 1))


def token(value):
    return "null" if value is None else str(value)


def indexes(args, path, table):
    lines = run(args.cli, "inspect", path, "--table", table, "--indexes").splitlines()
    labels = lines[1].split("\t")
    return [(cells[0], int(cells[labels.index("Root page")]), cells[labels.index("Column")])
            for line in lines[2:] if (cells := line.split("\t"))]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "preview", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    # Compile only our inert oracle once; repeated comparisons still reopen every
    # candidate independently without source-launch compilation overhead.
    with tempfile.TemporaryDirectory(prefix="tablecraft-foreign-oracle-") as classes:
        run("javac", "-cp", args.jar, "-d", classes, "docs/testing/TableDataOracle.java")
        def oracle(path, *arguments):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar) + os.pathsep + classes,
                       "TableDataOracle", path, *arguments)
        for path, code in ((args.ace2016, 5), (args.ace2019, 6)):
            original = path.read_bytes()
            if original[20:24] != bytes([code, 0, 0, 0]):
                raise RuntimeError("Expected original ACE 16 family")
            ids = catalog_ids(args, path)
            hashes, noops, rejections = [], 0, 0
            with tempfile.TemporaryDirectory(prefix="tablecraft-foreign-updates-") as directory:
                for prefix, kind, width, tag, minimum, maximum in (*KINDS, ("SelfLinks", "int32", 4, "I32:", 1, 3)):
                    self_link = prefix == "SelfLinks"
                    tables = [prefix] if self_link else [prefix + suffix for suffix in ("Parents", "Children", "One", "Required")]
                    before, layouts, records, declarations = {}, {}, {}, {}
                    for table in tables:
                        before[table] = oracle(path, table)
                        if run(args.rows, path, table) != before[table]:
                            raise RuntimeError("Original exact rows/long values differ")
                        layouts[table] = fields(original, ids[table])
                        records[table] = roots(original, ids[table])
                        declarations[table] = indexes(args, path, table)
                    if self_link:
                        cases = ((prefix, "Parent", 0, None, 2), (prefix, "Parent", 1, 1, None),
                                 (prefix, "Id", 2, 3, 4), (prefix, "Parent", 0, None, None))
                        invalid = ((prefix, "Parent", 0, None, 99), (prefix, "Id", 0, 1, 4))
                    else:
                        adjacent = 9007199254740992 if width == 8 else 32
                        cases = ((prefix + "Children", "Parent", 0, None, maximum),
                                 (prefix + "Children", "Parent", 1, minimum, 10),
                                 (prefix + "Children", "Parent", 2, maximum, None),
                                 (prefix + "Children", "Parent", 6, adjacent + (1 if width == 8 else -1), adjacent),
                                 (prefix + "Children", "Parent", 0, None, None),
                                 (prefix + "One", "Parent", 0, None, maximum),
                                 (prefix + "Parents", "Id", 4, 30, 40))
                        invalid = ((prefix + "Children", "Parent", 0, None, 99),
                                   (prefix + "Parents", "Id", 2, 10, 40),
                                   (prefix + "Parents", "Id", 0, minimum, None),
                                   (prefix + "One", "Parent", 0, None, 20),
                                   (prefix + "Required", "Parent", 0, 10, None))
                    for table, field, ordinal, old, new in cases:
                        output = Path(directory) / f"candidate-{len(hashes)}.accdb"
                        command = list(map(str, (args.preview, path, output, table, field, ordinal, kind, token(old), token(new))))
                        result = subprocess.run(command, capture_output=True, text=True)
                        if result.returncode or result.stdout != f"candidate_changed_rows={int(old != new)}\n" or "qualification pending" not in result.stderr:
                            raise RuntimeError("Foreign candidate failed: " + table + "/" + field + " " + result.stderr)
                        after = output.read_bytes()
                        allowed, old_pages, new_pages = [], {}, {}
                        for label, root, columns in declarations[table]:
                            if oracle(output, "--index", table, label) != run(args.rows, output, table, "--index", label):
                                raise RuntimeError("Foreign/ordinary key and row order differ")
                            # Only indexes containing the selected single column
                            # may change. The generated fixture has no composites.
                            if columns.split()[0] != field:
                                continue
                            _, old_refs, _ = tree_spans(original, root, old_pages, key_width=width)
                            _, new_refs, _ = tree_spans(after, root, new_pages, key_width=width)
                            if set(old_refs) != set(new_refs):
                                raise RuntimeError("Foreign row-reference set changed")
                        if not old_pages or set(old_pages) != set(new_pages):
                            raise RuntimeError("Affected index page allocation changed")
                        for page, state in old_pages.items():
                            current = new_pages[page]
                            if state["children"] != current["children"] or state["count"] != current["count"]:
                                raise RuntimeError("Foreign tree topology changed")
                            at = page * PAGE_BYTES
                            allowed.extend((range(at + 2, at + 4), range(at + 24, at + 26),
                                            range(at + 27, at + max(state["end"], current["end"]))))
                        field_at, _, column, _ = layouts[table][field]
                        offset = int.from_bytes(original[field_at + 21:field_at + 23], "little")
                        row, end = records[table][ordinal]
                        count = int.from_bytes(original[row:row + 2], "little")
                        mask = end - (count + 7) // 8 + column // 8
                        if (original[mask] ^ after[mask]) & ~(1 << (column % 8)):
                            raise RuntimeError("Neighbor presence bits changed")
                        allowed.append(range(mask, mask + 1))
                        if new is not None:
                            allowed.append(range(row + 2 + offset, row + 2 + offset + width))
                        if len(after) != len(original) or any(a != b and not any(at in span for span in allowed)
                                for at, (a, b) in enumerate(zip(original, after))):
                            raise RuntimeError("Bytes outside selected field/mask/index regions changed")
                        for related in tables:
                            expected = before[related]
                            if related == table:
                                lines = [line.split("\t") for line in expected.splitlines()]
                                position = list(layouts[table]).index(field) + 1
                                if lines[ordinal][position] != ("N" if old is None else tag + str(old)):
                                    raise RuntimeError("Expected original exact carrier differs")
                                lines[ordinal][position] = "N" if new is None else tag + str(new)
                                expected = "".join("\t".join(line) + "\n" for line in lines)
                            if oracle(output, related) != expected or run(args.rows, output, related) != expected:
                                raise RuntimeError("Related rows or preserved long values differ")
                        for mode in ("--check", "--constraints"):
                            report = run(args.cli, "inspect", output, "--table", table, mode)
                            if "unsupported" in report.lower():
                                raise RuntimeError("Candidate validation has incomplete coverage")
                        if old == new:
                            if original != after:
                                raise RuntimeError("Foreign no-op changed the image")
                            noops += 1
                        retained = subprocess.run(command, capture_output=True)
                        if retained.returncode == 0 or retained.stdout or output.read_bytes() != after:
                            raise RuntimeError("Existing output overwritten")
                        hashes.append(hashlib.sha256(after).hexdigest())
                    for table, field, ordinal, old, new in invalid:
                        output = Path(directory) / f"reject-{rejections}.accdb"
                        result = subprocess.run(list(map(str, (args.preview, path, output, table, field, ordinal, kind, token(old), token(new)))), capture_output=True)
                        if result.returncode == 0 or result.stdout or output.exists():
                            raise RuntimeError("Invalid foreign request published output")
                        reason = (b"NullKey" if field == "Id" and new is None else
                                  b"Required" if table.endswith("Required") else
                                  b"DuplicateKey" if table.endswith("One") else b"MissingParent")
                        if reason not in result.stderr:
                            raise RuntimeError("Expected foreign constraint rejection is missing")
                        rejections += 1
                    print(path.name, prefix, "related rows and index candidates match", flush=True)
            if path.read_bytes() != original:
                raise RuntimeError("Original foreign fixture changed")
            print(path.name, "candidates", len(hashes), "noops", noops, "rejections", rejections,
                  "original", hashlib.sha256(original).hexdigest(), "candidate-manifest",
                  hashlib.sha256("\n".join(hashes).encode()).hexdigest(), flush=True)
    print("Checked foreign candidates preserve membership, keys and unrelated bytes; cascades and native Access remain unqualified.")


if __name__ == "__main__":
    main()
