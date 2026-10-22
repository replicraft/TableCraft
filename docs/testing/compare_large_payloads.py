"""Compare larger native payloads with independent Jackcess getters and Python bytes.

The Java recipe is original. No Microsoft Access SQL equivalence is implied.
"""
import argparse
import csv
import io
import hashlib
from pathlib import Path
import subprocess

RECIPES = Path(__file__).resolve().parent
JACKCESS_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(*arguments, succeeds=True):
    process = subprocess.run([str(value) for value in arguments], capture_output=True, check=False)
    if succeeds:
        assert process.returncode == 0, process.stderr.decode()
    else:
        assert process.returncode != 0 and not process.stdout, process.stderr.decode()
    return process.stdout.decode("utf-8")


def signature(tagged):
    if tagged == "N":
        return [-1, "NULL"]
    assert tagged[0] in "TX"
    value = bytes.fromhex(tagged[1:])
    return [len(value), digest(value)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "table-data", "attachment-data", "cli", "jet4", "ace2007", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    csv.field_size_limit(16 * 1024 * 1024)
    assert digest(args.jar.read_bytes()) == JACKCESS_SHA256
    binary = bytes((i * 31 + i // 256) % 256 for i in range(2_097_152))
    expected_values = [
        [1_048_576, digest(b"a" * 1_048_576), 2_097_152, digest(binary)],
        [300_000, digest("é🦀".encode() * 50_000), 0, digest(b"")],
        [-1, "NULL", -1, "NULL"],
    ]
    expected_files = [
        [1_048_576, digest(bytes([42]) * 1_048_576), 1],
        [1_048_576, digest(bytes((i * 17 + i // 256) % 256 for i in range(1_048_576))), 0],
    ]
    controls = 0
    for path in [args.jet4, args.ace2007, args.ace2016, args.ace2019]:
        original = digest(path.read_bytes())
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "LargePayloadFixture.java", "--inspect", path)
        values, files = [], []
        for line in oracle.splitlines():
            cells = line.split("\t")
            if cells[0] == "V":
                values.append([int(cells[2]), cells[3], int(cells[4]), cells[5]])
            else:
                assert cells[0] == "A"
                files.append([int(cells[2]), cells[3], int(cells[4])])
        assert values == expected_values
        output = run(args.table_data, path, "LargeValues", "--bytes", 67_108_864, "--steps", 10_000_000)
        rows = [line.split("\t") for line in output.splitlines()]
        assert [row[:2] for row in rows] == [["LargeValues", f"I32:{i}"] for i in [1, 2, 3]]
        assert [signature(row[2]) + signature(row[3]) for row in rows] == values
        controls += 2
        run(args.table_data, path, "LargeValues", "--bytes", 67_108_864, "--steps", 1000, succeeds=False)
        controls += 1
        options = ["--database", path, "--native-bytes", 67_108_864,
                   "--native-steps", 10_000_000, "--execution-steps", 10_000_000]
        assert run(args.cli, "query", "run", "SELECT Count([Bytes]) AS N FROM LargeValues", *options) == "N\n2\n"
        # The separate SQL Text limit remains explicit, with no partial output.
        run(args.cli, "query", "run", "SELECT [Memo] FROM LargeValues", *options, succeeds=False)
        controls += 2
        # Opt-in Text admission and output transport are separate from native reads.
        large = [*options, "--text-bytes", 1_048_576, "--output-bytes", 8_388_608, "--format", "tsv"]
        memo_output = run(args.cli, "query", "run", "SELECT [Memo] FROM LargeValues", *large)
        memo_rows = list(csv.reader(io.StringIO(memo_output), delimiter="\t"))[1:]
        memo_signatures = []
        for row in memo_rows:
            if row[0] == "NULL":
                memo_signatures.append([-1, "NULL"])
            else:
                assert row[0].startswith('"') and row[0].endswith('"')
                data = row[0][1:-1].encode("utf-8")
                memo_signatures.append([len(data), digest(data)])
        assert memo_signatures == [row[:2] for row in values]
        binary_output = run(args.cli, "query", "run", "SELECT [Bytes] FROM LargeValues", *large)
        binary_rows = list(csv.reader(io.StringIO(binary_output), delimiter="\t"))[1:]
        assert [signature("N" if row[0] == "NULL" else "X" + row[0][2:]) for row in binary_rows] == [row[2:] for row in values]
        assert run(args.cli, "query", "run", "SELECT Len([Memo]) AS N FROM LargeValues", *large) == "N\n1048576\n150000\nNULL\n"
        run(args.cli, "query", "run", "SELECT [Memo] FROM LargeValues", *options, "--text-bytes", 1_048_576, succeeds=False)
        controls += 4
        if path != args.jet4:
            assert files == expected_files
            decoded = run(args.attachment_data, path, "--table", "LargeAttachments",
                          "--bytes", 67_108_864, "--steps", 10_000_000)
            payloads = [bytes.fromhex(line.split("\t")[3]) for line in decoded.splitlines()]
            assert [[len(data), digest(data)] for data in payloads] == [row[:2] for row in files]
            controls += 2
            collection = [*options, "--collections", "--decoded-attachments"]
            sql = "SELECT Count(Files.FileData) AS N FROM LargeAttachments"
            assert run(args.cli, "query", "run", sql, *collection) == "N\n2\n"
            controls += 1
            full = run(args.cli, "query", "run", "SELECT Files.FileData AS Data FROM LargeAttachments", *collection, "--output-bytes", 8_388_608, "--format", "tsv")
            rows = list(csv.reader(io.StringIO(full), delimiter="\t"))[1:]
            assert [signature("X" + row[0][2:]) for row in rows if row[0] != "NULL"] == [row[:2] for row in files]
            assert sum(row == ["NULL"] for row in rows) == 1
            controls += 1
            run(args.cli, "query", "run", sql, *collection, "--native-rows", 1, succeeds=False)
            controls += 1
        assert digest(path.read_bytes()) == original
        controls += 1
        print(f"{path.name}: unchanged, exact larger payloads")
    print(f"Passed {controls} independent controls; native Access qualification remains pending")


if __name__ == "__main__":
    main()
