"""Compare explicit cached pages against inert independent stored-row oracles.

Reuses the original CalculatedScalarFixture/CalculatedStorageFixture inputs.
No expressions execute. Exact DATE serials are independently recovered before
checking the CLI's documented nanosecond presentation, rather than using the
Java calendar carrier's millisecond rounding. This is not native Access evidence.
"""
import argparse
import datetime
import hashlib
import math
from pathlib import Path
import struct
import subprocess
import tempfile

from compare_browsing import display, escaped, run

NANOS_PER_SECOND = 1_000_000_000
SECONDS_PER_DAY = 86_400


def date_display(tag):
    if tag == "N":
        return "NULL"
    value = struct.unpack(">d", bytes.fromhex(tag.removeprefix("Y64:")))[0]
    day = math.trunc(value)
    nanos = round(abs(value - day) * SECONDS_PER_DAY * NANOS_PER_SECOND)
    carry, nanos = divmod(nanos, SECONDS_PER_DAY * NANOS_PER_SECOND)
    date = datetime.date(1899, 12, 30) + datetime.timedelta(days=day + carry)
    seconds, fraction = divmod(nanos, NANOS_PER_SECOND)
    hour, seconds = divmod(seconds, 3600)
    minute, second = divmod(seconds, 60)
    return f"#{date.isoformat()} {hour:02}:{minute:02}:{second:02}.{fraction:09}#"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "scalar2016", "scalar2019", "int2016", "int2019"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    cases = [(args.scalar2016, "CalculatedScalars", ["Id", "Flag", "Number", "Amount", "Moment", "Label", "Compact"]),
             (args.scalar2019, "CalculatedScalars", ["Id", "Flag", "Number", "Amount", "Moment", "Label", "Compact"]),
             (args.int2016, "Calculations", ["Score", "Twice"]),
             (args.int2019, "Calculations", ["Score", "Twice"])]
    controls = 0
    for path, table, columns in cases:
        original = path.read_bytes()
        oracle = lambda recipe, *extra: run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            Path(__file__).parent / (recipe + ".java"), path, *extra)
        rows = [line.split("\t")[1:] for line in oracle("TableDataOracle", table).splitlines()]
        dates = None
        if "Moment" in columns:
            dates = [line.split("\t")[1] for line in oracle("StoredDateBitsOracle", table, "Moment").splitlines()]
        header = "\t".join(escaped(c) + (" [cached]" if i else "") for i, c in enumerate(columns))
        for offset, limit in [(0, 100), (1, 1), (2, 2), (len(rows), 1), (4_294_967_295, 1)]:
            output = run(args.cli, "rows", path, table, "--stored-calculations", "--format", "tsv", "--offset", offset, "--limit", limit).splitlines()
            assert output[0] == header
            selected = list(enumerate(rows))[offset:offset + limit]
            assert len(output) == len(selected) + 1
            for actual_line, (index, expected) in zip(output[1:], selected):
                actual = actual_line.split("\t")
                assert len(actual) == len(expected)
                for field, (cell, tag) in enumerate(zip(actual, expected)):
                    if tag.startswith("D"):
                        assert struct.pack(">d", float(cell)).hex() == tag[1:]
                    elif columns[field] == "Moment":
                        assert cell == date_display(dates[index])
                    else:
                        assert cell == display(tag), (columns[field], cell, tag)
            controls += 1
        empty = run(args.cli, "rows", path, "Empty" + table, "--stored-calculations", "--format", "tsv")
        assert empty == header + "\n"
        controls += 1
        for command in [["rows", path, table], ["query", "run", f"SELECT * FROM {table}", "--database", path]]:
            result = subprocess.run(list(map(str, [args.cli, *command])), capture_output=True)
            assert result.returncode == 2 and not result.stdout
            controls += 1
        pretty = run(args.cli, "rows", path, table, "--stored-calculations", "--format", "table", "--limit", 1)
        assert pretty.startswith("┌") and "[cached]" in pretty
        controls += 1
        if "Moment" in columns:
            needle = bytes(16) + (1).to_bytes(4, "little") + bytes([0xff, 0, 0, 0])
            at = original.index(needle)
            with tempfile.TemporaryDirectory(prefix="tablecraft-cached-page-") as directory:
                changed = bytearray(original)
                changed[at + 20] = 1
                copy = Path(directory) / "bad.accdb"
                copy.write_bytes(changed)
                result = subprocess.run([str(args.cli), "rows", str(copy), table, "--stored-calculations"], capture_output=True)
                assert result.returncode == 2 and not result.stdout
                controls += 1
        assert path.read_bytes() == original
        print(path.name, table, len(rows), "rows; unchanged SHA-256", hashlib.sha256(original).hexdigest())
    print(controls, "page/empty/default-query-refusal/pretty/corruption controls passed; formulas and native Access unrun")


if __name__ == "__main__":
    main()
