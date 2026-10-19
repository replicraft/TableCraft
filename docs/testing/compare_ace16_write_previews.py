"""Compare disposable ACE 16 physical candidates; no native qualification claim."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from compare_composite_constraints import run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "example", "ace2016", "ace2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path, code in ((args.ace2016, 5), (args.ace2019, 6)):
        original = path.read_bytes()
        if original[20:24] != bytes([code, 0, 0, 0]):
            raise RuntimeError("Expected original ACE 16 read family")
        def inspect(source):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/UpdateFixture.java", "--inspect", source)
        expected = "1\t10\t8400\t12288\n2\t20\t8400\t12288\n3\t30\t8400\t12288\n"
        if inspect(path) != expected:
            raise RuntimeError("Unexpected original inert fixture")
        metadata = run(args.cli, "inspect", path, "--table", "Items", "--indexes")
        root = int(next(line.split("\t")[3] for line in metadata.splitlines()
            if line.startswith("PK_Items\t")))
        page = original[root * 4096:(root + 1) * 4096]
        if page[:2] != bytes([4, 1]) or page[24:26] != bytes(2):
            raise RuntimeError("Expected uncompressed original root leaf")
        end = 4096 - int.from_bytes(page[2:4], "little")
        reference = int.from_bytes(page[494:498], "big")
        data, slot = reference >> 8, reference & 255
        row = int.from_bytes(original[data * 4096 + 14 + slot * 2:
            data * 4096 + 16 + slot * 2], "little") & 0x1fff
        id_field = data * 4096 + row + 2
        if original[id_field:id_field + 8] != (2).to_bytes(4, "little") + (20).to_bytes(4, "little"):
            raise RuntimeError("Expected original Id/Score carrier bytes")
        with tempfile.TemporaryDirectory(prefix="tablecraft-ace16-write-preview-") as directory:
            for column, old, new in (("Score", 20, 27), ("Id", 2, 27)):
                output = Path(directory) / (column + ".accdb")
                command = [str(args.example), str(path), str(output), "Items", column, "1", str(old), str(new)]
                result = subprocess.run(command, capture_output=True, text=True)
                if (result.returncode or result.stdout != "candidate_changed_rows=1\n"
                        or "qualification pending" not in result.stderr):
                    raise RuntimeError("Candidate creation/qualification output failed")
                after = output.read_bytes()
                spans = [range(id_field + (4 if column == "Score" else 0),
                    id_field + (8 if column == "Score" else 4))]
                if column == "Id":
                    spans.append(range(root * 4096 + 480, root * 4096 + end))
                if len(after) != len(original) or any(a != b and not any(i in span for span in spans)
                        for i, (a, b) in enumerate(zip(original, after))):
                    raise RuntimeError("Candidate changed bytes outside exact field/index spans")
                changed = expected.replace("2\t20\t", "2\t27\t" if column == "Score" else "27\t20\t")
                if inspect(output) != changed:
                    raise RuntimeError("Independent candidate rows/long-value lengths differ")
                ordered = run("java", "-cp", args.jar, "docs/testing/IndexRowsOracle.java",
                    output, "Items", "PK_Items", "Id")
                if ordered != ("1\n2\n3\n" if column == "Score" else "1\n3\n27\n"):
                    raise RuntimeError("Independent candidate index references differ")
                report = run(args.cli, "inspect", output, "--table", "Items", "--check")
                if "unsupported" in report.lower() or "checked" not in report.lower():
                    raise RuntimeError("Candidate structural/constraint coverage incomplete")
                retained = subprocess.run(command, capture_output=True)
                if retained.returncode == 0 or retained.stdout or output.read_bytes() != after:
                    raise RuntimeError("Existing candidate was overwritten")
                print(path.name, column, "candidate", hashlib.sha256(after).hexdigest())
            for number, (column, old, new) in enumerate((("Score", 20, 101), ("Score", 99, 27), ("Id", 2, 1))):
                output = Path(directory) / f"rejected-{number}.accdb"
                result = subprocess.run([str(args.example), str(path), str(output), "Items", column,
                    "1", str(old), str(new)], capture_output=True)
                if result.returncode == 0 or result.stdout or output.exists():
                    raise RuntimeError("Rule/stale/duplicate rejection published candidate")
            output = Path(directory) / "ordinary-update.accdb"
            result = subprocess.run([str(args.cli), "query", "update", "UPDATE Items SET Score=27 WHERE Id=2",
                "--database", str(path), "--output", str(output)], capture_output=True)
            if result.returncode != 2 or result.stdout or output.exists():
                raise RuntimeError("Ordinary ACE write gate changed")
        if path.read_bytes() != original:
            raise RuntimeError("Original fixture changed")
        print(path.name, "original", hashlib.sha256(original).hexdigest())
    print("Independent candidate rows/indexes and byte preservation pass; macro semantics/native Access remain unqualified.")


if __name__ == "__main__":
    main()
