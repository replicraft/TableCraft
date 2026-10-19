"""Compare native Boolean and nullable fixed-field masks on disposable candidates."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_attachment_columns import fields
from compare_composite_constraints import roots, run
from compare_fixed_updates import CASES
from compare_saved_queries import catalog_ids


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "example", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path, code in ((args.ace2016, 5), (args.ace2019, 6)):
        original = path.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Unexpected original ACE 16 family")
        ids = catalog_ids(args, path)
        cache = {}
        for table, count in (("Items", 4), ("Children", 2)):
            before = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, "--date-bits", table)
            if run(args.rows, path, table, "--date-bits") != before:
                raise RuntimeError("Original exact carriers differ")
            definition = ids[table]
            layout = fields(original, definition)
            records = roots(original, definition)
            columns = int.from_bytes(original[definition * 4096 + 41:definition * 4096 + 43], "little")
            if len(records) != count or len(before.splitlines()) != count or columns != len(layout):
                raise RuntimeError("Unexpected original inline geometry")
            cache[table] = (before, layout, records, columns)
        digest = hashlib.sha256()
        candidates = 0
        noops = 0
        with tempfile.TemporaryDirectory(prefix="tablecraft-mask-scalar-") as directory:
            def candidate(table, name, ordinal, kind, old, new, old_tag, new_tag):
                nonlocal candidates, noops
                before, layout, records, columns = cache[table]
                names = list(layout)
                lines = before.splitlines()
                cells = lines[ordinal].split("\t")
                position = names.index(name) + 1
                if cells[1] != f"I32:{ordinal + 1}" or cells[position] != old_tag:
                    raise RuntimeError("Unexpected original comparison row")
                cells[position] = new_tag
                lines[ordinal] = "\t".join(cells)
                expected = "\n".join(lines) + "\n"
                at = layout[name][0]
                number = layout[name][2]
                declared_width = int.from_bytes(original[at + 23:at + 25], "little")
                offset = int.from_bytes(original[at + 21:at + 23], "little")
                boolean = original[at] == 1
                width = 0 if boolean else declared_width
                if (not original[at + 15] & 1 or not 0 <= width <= 16
                        or (boolean and declared_width not in (0, 1)) or (not boolean and width == 0)):
                    raise RuntimeError("Unexpected fixed mask profile")
                row_start, row_end = records[ordinal]
                mask = row_end - (columns + 7) // 8 + number // 8
                bit = 1 << (number % 8)
                spans = [range(mask, mask + 1)]
                if not boolean and new != "null":
                    spans.append(range(row_start + 2 + offset, row_start + 2 + offset + width))
                output = Path(directory) / f"candidate-{candidates}.accdb"
                command = [str(args.example), str(path), str(output), table, name, str(ordinal), kind, old, new]
                result = subprocess.run(command, capture_output=True, text=True)
                changed = int(old_tag != new_tag)
                if (result.returncode or result.stdout != f"candidate_changed_rows={changed}\n"
                        or "qualification pending" not in result.stderr):
                    raise RuntimeError("Mask candidate failed: " + table + "/" + name)
                after = output.read_bytes()
                if (len(after) != len(original) or any(a != b and not any(i in span for span in spans)
                        for i, (a, b) in enumerate(zip(original, after)))):
                    raise RuntimeError("Bytes outside exact mask/payload spans changed")
                if ((original[mask] ^ after[mask]) & (~bit & 255)
                        or bool(after[mask] & bit) != (new_tag not in ("N", "B0"))):
                    raise RuntimeError("Mask changed neighboring bits or wrong logical state")
                if not changed and after != original:
                    raise RuntimeError("No-op changed physical bytes")
                oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                    "docs/testing/TableDataOracle.java", output, "--date-bits", table)
                if oracle != expected or run(args.rows, output, table, "--date-bits") != expected:
                    raise RuntimeError("Reopened values/long payloads differ")
                for mode in ("--check", "--constraints"):
                    report = run(args.cli, "inspect", output, "--table", table, mode)
                    if "unsupported" in report.lower():
                        raise RuntimeError("Candidate has unsupported validation coverage")
                retained = subprocess.run(command, capture_output=True)
                if retained.returncode == 0 or retained.stdout or output.read_bytes() != after:
                    raise RuntimeError("Existing output path was overwritten")
                candidates += 1
                noops += int(not changed)
                digest.update(expected.encode())
            for name, kind, old, new, tag, replacement in CASES:
                candidate("Items", name, 0, kind, old, "null", tag, "N")
                candidate("Items", name, 1, kind, "null", new, "N", replacement)
                candidate("Items", name, 2, kind, "null", "null", "N", "N")
            before = cache["Items"][0].splitlines()
            names = list(cache["Items"][1])
            for name in ("B1", "B7", "B8", "B13", "B14"):
                for ordinal in (0, 1, 2):
                    old = before[ordinal].split("\t")[names.index(name) + 1]
                    new = old if ordinal == 2 else ("B1" if old == "B0" else "B0")
                    candidate("Items", name, ordinal, "boolean", str(old == "B1").lower(),
                        str(new == "B1").lower(), old, new)
            rejections = (
                ("Items", "MustHave", 0, "int32", "12", "null"),
                ("Items", "MustHave", 0, "int32", "12", "-1"),
                ("Items", "B1", 0, "boolean", "false", "null"),
                ("Items", "B1", 0, "boolean", "null", "true"),
                ("Items", "Large", 0, "int64", "null", "9"),
                ("Items", "Large", 1, "int64", "8", "null"),
                ("Items", "Large", 1, "int32", "null", "1"),
                ("Items", "Id", 0, "int32", "1", "null"),
                ("Children", "ForeignKey", 0, "int64", "null", "1"),
                ("Children", "ForeignKey", 0, "int64", "null", "2"),
                ("IndexedBoolean", "Flag", 0, "boolean", "false", "true"),
                ("Items", "Large", 99, "int64", "null", "null"),
            )
            for i, (table, name, ordinal, kind, old, new) in enumerate(rejections):
                output = Path(directory) / f"rejected-{i}.accdb"
                result = subprocess.run([str(args.example), str(path), str(output), table, name, str(ordinal), kind, old, new], capture_output=True)
                if result.returncode == 0 or result.stdout or output.exists():
                    raise RuntimeError("Rejected mask request created output")
                if table == "Children" and b"ConstraintCoverage" not in result.stderr:
                    raise RuntimeError("Foreign-index coverage blocker changed")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        if candidates != 42 or noops != 14:
            raise RuntimeError("Coverage controls changed")
        print(path.name, "candidates", candidates, "noops", noops, "rejections", len(rejections),
            "original", hashlib.sha256(original).hexdigest(), "rows", digest.hexdigest(), flush=True)
    print("Boolean bits, nine nullable carriers, required checks and foreign-index blocking pass; native Access/macros unqualified.")


if __name__ == "__main__":
    main()
