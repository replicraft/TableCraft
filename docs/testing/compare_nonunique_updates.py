"""Compare disposable non-unique updates, exact row ties and unrelated bytes."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_attachment_columns import fields
from compare_composite_constraints import roots, run
from compare_saved_queries import catalog_ids


PAGE_BYTES = 4096
ENTRY_START = 480
BITMAP_START = 27
INDEXES = ("Asc", "Desc", "Alias")
CASES = (("Leaves", 2, 20, 10), ("Leaves", 1, 10, 20), ("Leaves", 4, 30, 10),
    ("Leaves", 2, 20, 20), ("Tree", 1, 1, 30), ("Tree", 998, 6, 1), ("Tree", 16, 16, 0))


def tree_spans(data, root):
    """Independent controlled Int32/NULL tree/row-reference and prefix binding."""
    spans, references, visited = [], {}, set()
    def visit(number):
        if number == 0 or number in visited:
            raise RuntimeError("Repeated/absent original tree page")
        visited.add(number)
        base = number * PAGE_BYTES
        page = data[base:base + PAGE_BYTES]
        if page[:2] not in (b"\x03\x01", b"\x04\x01"):
            raise RuntimeError("Expected controlled original tree page")
        prefix = int.from_bytes(page[24:26], "little")
        end = PAGE_BYTES - int.from_bytes(page[2:4], "little")
        spans.append(range(base + ENTRY_START, base + end))
        ends = [i for i in range(1, end - ENTRY_START + 1)
            if page[BITMAP_START + i // 8] & (1 << (i % 8))]
        previous = 0
        first = None
        for position in ends:
            entry = page[ENTRY_START + previous:ENTRY_START + position]
            if first is None:
                first = entry
                if prefix > len(first):
                    raise RuntimeError("Invalid original entry prefix")
            else:
                entry = first[:prefix] + entry
            if entry[0] not in (0, 255, 0x7f, 0x80):
                raise RuntimeError("Unknown original key marker")
            key_bytes = 0 if entry[0] in (0, 255) else 4
            width = 1 + key_bytes + 4
            if len(entry) != width + (4 if page[0] == 3 else 0):
                raise RuntimeError("Unexpected original key width")
            reference = int.from_bytes(entry[1 + key_bytes:width], "big")
            if page[0] == 4:
                if reference in references:
                    raise RuntimeError("Repeated original row reference")
                references[reference] = entry[:1 + key_bytes]
            else:
                visit(int.from_bytes(entry[width:width + 4], "big"))
            previous = position
        if page[0] == 3:
            visit(int.from_bytes(page[20:24], "little"))
    visit(root)
    return spans, references, len(visited)


def oracle(args, path, table, index=None):
    command = (path, table) if index is None else (path, "--index", table, index)
    return run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, "docs/testing/TableDataOracle.java", *command)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "rows", "preview", "native", "ace2016", "ace2019", "jet4"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path, code in ((args.ace2016, 5), (args.ace2019, 6), (args.jet4, 1)):
        original = path.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Unexpected original family")
        ids = catalog_ids(args, path)
        before, allowed, rows, node_roots = {}, {}, {}, 0
        for table in ("Leaves", "Tree"):
            before[table] = oracle(args, path, table)
            if run(args.rows, path, table) != before[table]:
                raise RuntimeError("Original rows/long values differ")
            rows[table] = roots(original, ids[table])
            metadata = run(args.cli, "inspect", path, "--table", table, "--indexes").splitlines()[2:]
            selected = {line.split("\t")[0]: int(line.split("\t")[3]) for line in metadata if line.split("\t")[0] in INDEXES}
            if set(selected) != set(INDEXES):
                raise RuntimeError("Missing ordinary index declarations")
            allowed[table] = []
            for index, root in selected.items():
                spans, references, pages = tree_spans(original, root)
                if len(references) != len(rows[table]) or (table == "Tree" and pages < 3):
                    raise RuntimeError("Incomplete original tree/row coverage")
                node_roots += int(original[root * PAGE_BYTES] == 3)
                allowed[table].extend(spans)
        hashes = []
        with tempfile.TemporaryDirectory(prefix="tablecraft-nonunique-updates-") as directory:
            for number, (table, ordinal, old, new) in enumerate(CASES):
                output = Path(directory) / (f"candidate-{number}" + (".mdb" if code == 1 else ".accdb"))
                example = args.native if code == 1 else args.preview
                command = list(map(str, (example, path, output, table, "Score", ordinal, old, new)))
                result = subprocess.run(command, capture_output=True, text=True)
                changed = int(old != new)
                expected_output = ("changed_rows=" if code == 1 else "candidate_changed_rows=") + str(changed) + "\n"
                if result.returncode or result.stdout != expected_output or (code != 1 and "qualification pending" not in result.stderr):
                    raise RuntimeError("Candidate failed: " + result.stderr)
                expected = [line.split("\t") for line in before[table].splitlines()]
                if expected[ordinal][2] != "I32:" + str(old):
                    raise RuntimeError("Wrong original expected carrier")
                expected[ordinal][2] = "I32:" + str(new)
                expected_text = "".join("\t".join(row) + "\n" for row in expected)
                if oracle(args, output, table) != expected_text or run(args.rows, output, table) != expected_text:
                    raise RuntimeError("Candidate rows/unrelated long values differ")
                for index in INDEXES:
                    if oracle(args, output, table, index) != run(args.rows, output, table, "--index", index):
                        raise RuntimeError("Duplicate key/row-reference order differs")
                field = fields(original, ids[table])["Score"][0]
                offset = int.from_bytes(original[field + 21:field + 23], "little")
                start = rows[table][ordinal][0] + 2 + offset
                if int.from_bytes(original[start:start + 4], "little", signed=True) != old:
                    raise RuntimeError("Independent physical field binding differs")
                spans = [*allowed[table], range(start, start + 4)]
                after = output.read_bytes()
                if len(after) != len(original) or any(a != b and not any(i in span for span in spans)
                        for i, (a, b) in enumerate(zip(original, after))):
                    raise RuntimeError("Unrelated bytes changed")
                if changed == 0 and after != original:
                    raise RuntimeError("No-op changed image")
                report = run(args.cli, "inspect", output, "--table", table, "--check")
                if "Indexes not checked\t0\n" not in report:
                    raise RuntimeError("Candidate checks incomplete")
                if code == 1:
                    cli_output = Path(directory) / f"cli-{number}.mdb"
                    identity = expected[ordinal][1].removeprefix("I32:")
                    report = run(args.cli, "query", "update", f"UPDATE [{table}] SET [Score]={new} WHERE [Id]={identity}", "--database", path, "--output", cli_output)
                    if report != f"Matched rows\t1\nChanged rows\t{changed}\n" or cli_output.read_bytes() != after:
                        raise RuntimeError("Jet 4 CLI and engine copies differ")
                retained = subprocess.run(command, capture_output=True)
                if retained.returncode == 0 or retained.stdout or output.read_bytes() != after:
                    raise RuntimeError("Existing output was replaced")
                hashes.append(hashlib.sha256(after).hexdigest())
            rejections = (("Unique", 1, 20, 30), ("Rules", 1, 20, -1), ("Leaves", 2, 999, 10),
                ("Leaves", 99999, 20, 10), ("Leaves", 0, 0, 10))
            for number, (table, ordinal, old, new) in enumerate(rejections):
                output = Path(directory) / f"rejected-{number}.accdb"
                result = subprocess.run(list(map(str, (example, path, output, table, "Score", ordinal, old, new))), capture_output=True, text=True)
                if result.returncode == 0 or result.stdout or output.exists():
                    raise RuntimeError("Invalid update published output")
                if number == 0 and "DuplicateKey" not in result.stderr:
                    raise RuntimeError("Unique guard was not enforced")
            if code != 1:
                output = Path(directory) / "ordinary.accdb"
                result = subprocess.run(list(map(str, (args.cli, "query", "update", "UPDATE Leaves SET Score=10 WHERE Id=60", "--database", path, "--output", output))), capture_output=True)
                if result.returncode == 0 or result.stdout or output.exists():
                    raise RuntimeError("Engineering preview enabled ordinary ACE editing")
        if path.read_bytes() != original:
            raise RuntimeError("Original input changed")
        print(path.name, "candidates", len(hashes), "noops", 1, "rejections", len(rejections), "node roots", node_roots,
            "original", hashlib.sha256(original).hexdigest(), "candidate-manifest", hashlib.sha256("\n".join(hashes).encode()).hexdigest(), flush=True)
    print("Non-unique ties, complete rows/long values and allowed spans match independent readers; native Access/unqualified mutation profiles remain separate.")


if __name__ == "__main__":
    main()
