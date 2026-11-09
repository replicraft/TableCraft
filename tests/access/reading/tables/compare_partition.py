"""Check Partition labels with an independently enumerated numeric model.

Optional native files are original SavedForest-derived AData fixtures. This
runner does not execute expressions in Microsoft Access.
"""

import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from io import StringIO
import json
from pathlib import Path
import random
import subprocess

from compare_large_indexes import INPUT_HASHES, registered_input_hash


def label(values):
    number, start, stop, interval = [
        int(Decimal(str(value)).to_integral_value(rounding=ROUND_HALF_EVEN))
        for value in values
    ]
    if interval == 1:
        lower, upper = str(number), str(number)
    elif number < start:
        lower, upper = "", str(start - 1)
    elif number > stop:
        lower, upper = str(stop + 1), ""
    else:
        for first in range(start, stop + 1, interval):
            members = range(first, min(first + interval, stop + 1))
            if number in members:
                lower, upper = str(members.start), str(members.stop - 1)
                break

    width = max(len(str(stop + 1)), len(str(start - 1)))
    return lower.rjust(width) + ":" + upper.rjust(width)


def execute(cli, mode, sql, database=None, succeeds=True, number_format=None, extra_options=()):
    command = [str(cli), "query", mode, sql]
    if database is not None:
        command.extend(["--database", str(database)])
    if number_format:
        command.extend(["--number-format", number_format])
    command.extend(extra_options)
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if not succeeds:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []

    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t"))[1:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--database", type=Path, action="append", default=[])
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()

    rng = random.Random(20261021)
    cases = []
    for _ in range(128):
        start = rng.randrange(100)
        stop = start + rng.randrange(1, 1000)
        cases.append([rng.randrange(-10, stop + 20), start, stop, rng.randrange(1, 100)])
    cases += [[2.5, 0, 9, 2], [1010, 100, 1010, 20], [-1, 0, 1, 2], [999, 0, 9, 1]]
    count = 0
    for mode in ["eval", "run"]:
        for values in cases:
            sql = "SELECT Partition(" + ",".join(map(str, values)) + ") AS P"
            rows = execute(args.cli, mode, sql)
            assert len(rows) == 1 and json.loads(rows[0][0]) == label(values), (values, rows)
            count += 1

        for expression in [
            "Partition(1,-1,9,2)", "Partition(1,0,0,2)", "Partition(1,0,9,0)",
            "Partition('1',0,9,2)", "Partition(Null,1/0,9,2)",
            "Partition(1,,9,2)", "Partition(1,0,9)", "Partition(1,0,9,2),1/0",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1
        assert execute(args.cli, mode, "SELECT Partition(Null,-1,0,0)") == [["NULL"]]
        count += 1

    for mode in ["eval", "run"]:
        for number_format, separator in [("en-us", "."), ("de-de", ",")]:
            for values in cases:
                arguments = ",".join("'" + str(value).replace(".", separator) + "'" for value in values)
                rows = execute(args.cli, mode, "SELECT Partition(" + arguments + ")", number_format=number_format)
                assert len(rows) == 1 and json.loads(rows[0][0]) == label(values), (values, rows)
                count += 1
            for expression in [
                "Partition('2147483648',0,9,2)",
                "Partition(1,'-1','9','bad')",
                "Partition('bad',NULL,9,1/0)",
                "Partition(1,'-1',9,2)",
            ]:
                execute(args.cli, mode, "SELECT 1," + expression, succeeds=False, number_format=number_format)
                count += 1
            assert execute(args.cli, mode, "SELECT Partition('bad',NULL,9,2)", number_format=number_format) == [["NULL"]]
            count += 1

    if args.native_directory:
        for suffix, digest in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            digest = registered_input_hash(database, digest)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest
            for number_format in ["en-us", "de-de"]:
                rows = execute(
                    args.cli, "run", "SELECT Partition(CStr(ID),'0','9','2') FROM IndexedRows WHERE ID=1",
                    database=database, number_format=number_format,
                    extra_options=[
                        "--index", "IndexedRows", "IdAsc", "--native-bytes", "67108864",
                        "--native-rows", "100000", "--native-steps", "10000000",
                        "--execution-steps", "10000000",
                    ],
                )
                assert rows == [[json.dumps(label([1, 0, 9, 2]))]], rows
                count += 1
            assert hashlib.sha256(database.read_bytes()).hexdigest() == digest

    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).digest()
        rows = execute(
            args.cli, "run", "SELECT ID,Partition(ID,0,9,3) FROM AData ORDER BY ID",
            database=database,
        )
        assert [int(row[0]) for row in rows] == [1, 2, 4], rows
        assert [json.loads(row[1]) for row in rows] == [label([n, 0, 9, 3]) for n in [1, 2, 4]]
        assert hashlib.sha256(database.read_bytes()).digest() == before
        count += len(rows)

    print(f"Passed {count} independent range/rounding/refusal/native-row controls; native Access unrun.")


if __name__ == "__main__":
    main()
