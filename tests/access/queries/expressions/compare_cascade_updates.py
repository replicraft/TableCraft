"""Compare bounded original-association cascade batches against complete literal rows."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

from compare_batch_updates import Cell
from compare_attachment_columns import fields
from compare_composite_constraints import run
from compare_composite_updates import declarations
from compare_saved_queries import catalog_ids


def cell(table, column, row, old, new, kind="int32"):
    return Cell(table, column, row, kind, old, new)


def cases():
    # Full literal closure, independently specified rather than inferred from
    # candidate bytes or the implementation's relationship-action traversal.
    root = (cell("Roots", "Key", 0, 1, 9),)
    closure = (*root, cell("Middle", "Key", 0, 1, 9),
               *(cell("Leaves", "Parent", row, 1, 9) for row in (0, 3, 6)))
    swap = (cell("Roots", "Key", 0, 1, 2), cell("Roots", "Key", 1, 2, 1))
    swapped = (*swap, cell("Middle", "Key", 0, 1, 2), cell("Middle", "Key", 1, 2, 1),
               *(cell("Leaves", "Parent", row, old, 3-old) for row, old in ((0, 1), (1, 2), (3, 1), (4, 2), (6, 1))))
    tuple_root = (cell("TupleRoots", "A", 0, 1, 9), cell("TupleRoots", "B", 0, 10, 90, "int64"))
    tuple_closure = (*tuple_root, cell("TupleMiddle", "A", 0, 1, 9), cell("TupleMiddle", "B", 0, 10, 90, "int64"),
                     *(cell("TupleLeaves", name, row, old, new, kind) for row in (0, 3)
                       for name, old, new, kind in (("A", 1, 9, "int32"), ("B", 10, 90, "int64"))))
    return ((root, closure), (swap, swapped),
            ((cell("Middle", "Key", 0, 1, 9), *root), closure),
            ((cell("CycleA", "Key", 0, 1, 9),), (cell("CycleA", "Key", 0, 1, 9), cell("CycleB", "Key", 0, 1, 9))),
            ((cell("CycleA", "Key", 0, 1, 2), cell("CycleA", "Key", 1, 2, 1)),
             tuple(cell(table, "Key", row, old, new) for table in ("CycleA", "CycleB") for row, old, new in ((0, 1, 2), (1, 2, 1)))),
            (tuple_root, tuple_closure),
            ((cell("Roots", "Key", 0, 1, 1),), (cell("Roots", "Key", 0, 1, 1),)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "preview", "checker", "ace2016", "ace2019"):
        parser.add_argument("--"+name, required=True, type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="tablecraft-cascade-oracle-") as classes:
        run("javac", "-cp", args.jar, "-d", classes, "docs/testing/TableDataOracle.java")
        def oracle(path, *extra):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar)+os.pathsep+classes,
                       "TableDataOracle", path, "--date-bits", *extra)
        for path, code in ((args.ace2016, 5), (args.ace2019, 6)):
            original = path.read_bytes()
            if original[20:24] != bytes((code, 0, 0, 0)):
                raise RuntimeError("Unexpected profile")
            ids = catalog_ids(args, path)
            names = ("Roots", "Middle", "Leaves", "CycleA", "CycleB", "Restricted", "References", "TupleRoots", "TupleMiddle", "TupleLeaves")
            cache = {}
            for name in names:
                rows = oracle(path, name)
                if run(args.rows, path, name, "--date-bits") != rows:
                    raise RuntimeError("Original reader disagreement")
                layout = fields(original, ids[name])
                cache[name] = (rows, layout, declarations(args, path, name, layout, original))
            def command(output, cells, cascade=True):
                return [str(args.preview), str(path), str(output), *(('--cascade-updates',) if cascade else ()),
                        *(part for c in cells for part in c.arguments())]
            hashes = []
            with tempfile.TemporaryDirectory(prefix="tablecraft-cascade-candidates-") as directory:
                for number, (request, closure) in enumerate(cases()):
                    output = Path(directory)/f"cascade-{number}.accdb"
                    count = len({(c.table, c.row) for c in closure if c.old != c.new})
                    result = subprocess.run(command(output, request), capture_output=True, text=True)
                    if result.returncode or result.stdout != f"candidate_changed_rows={count}\n":
                        raise RuntimeError("Cascade failed: "+result.stderr)
                    explicit = Path(directory)/f"explicit-{number}.accdb"
                    run(*command(explicit, closure, False))
                    after = output.read_bytes()
                    if after != explicit.read_bytes():
                        raise RuntimeError("Discovered closure differs from literal complete batch")
                    reverse = Path(directory)/f"reverse-{number}.accdb"
                    run(*command(reverse, request[::-1]))
                    if reverse.read_bytes() != after:
                        raise RuntimeError("Request order changes result")
                    for name in names:
                        before, layout, indexes = cache[name]
                        expected = [line.split('\t') for line in before.splitlines()]
                        for c in (c for c in closure if c.table == name):
                            position = list(layout).index(c.column)+1
                            if expected[c.row][position] != c.tag(c.old):
                                raise RuntimeError("Literal expected carrier mismatch")
                            expected[c.row][position] = c.tag(c.new)
                        expected = ''.join('\t'.join(row)+'\n' for row in expected)
                        if oracle(output, name) != expected or run(args.rows, output, name, "--date-bits") != expected:
                            raise RuntimeError("Complete/untouched rows or long values differ")
                        run(args.checker, output, name)
                        for label in indexes:
                            if oracle(output, "--index", name, label) != run(args.rows, output, name, "--date-bits", "--index", label):
                                raise RuntimeError("Independent index cursor mismatch")
                    repeat = subprocess.run(command(output, request), capture_output=True)
                    if not repeat.returncode or repeat.stdout or output.read_bytes() != after:
                        raise RuntimeError("Existing output changed")
                    hashes.append(hashlib.sha256(after).hexdigest())
                rejected = (
                    ((cell("Restricted", "Key", 0, 1, 9),), "ReferentialAction"),
                    ((cell("Restricted", "Key", 0, 1, 2), cell("Restricted", "Key", 1, 2, 1)), "ReferentialAction"),
                    ((cell("Roots", "Key", 0, 1, 9), cell("Middle", "Key", 0, 1, 8)), "Conflict"),
                    ((cell("Roots", "Key", 0, 1, 9), cell("Leaves", "Parent", 0, 1, 1)), "Conflict"),
                    ((cell("TupleRoots", "A", 0, 1, 9), cell("TupleLeaves", "B", 0, 10, 11, "int64")), "Conflict"),
                    ((cell("CycleA", "Key", 0, 1, 9), cell("CycleB", "Key", 0, 1, 8)), "Conflict"),
                    ((cell("Roots", "Key", 0, 2, 9),), "Conflict"),
                    ((cell("Roots", "Key", 0, 1, 9), cell("Roots", "Key", 0, 1, 9)), "Conflict"),
                )
                for number, (request, error) in enumerate(rejected):
                    output = Path(directory)/f"reject-{number}.accdb"
                    result = subprocess.run(command(output, request), capture_output=True, text=True)
                    if not result.returncode or result.stdout or output.exists() or error not in result.stderr:
                        raise RuntimeError("Rejected action published output or wrong error: "+result.stderr)
            if path.read_bytes() != original:
                raise RuntimeError("Original changed")
            print(path.name, "candidates", len(hashes), "reverse-controls", len(hashes), "rejections", len(rejected),
                  "original", hashlib.sha256(original).hexdigest(),
                  "candidate-manifest", hashlib.sha256('\n'.join(hashes).encode()).hexdigest())
    print("Original associations, multi-hop/overlapping/cyclic cascades and exact complete rows agree; native Access and delete actions remain unqualified.")


if __name__ == '__main__':
    main()
