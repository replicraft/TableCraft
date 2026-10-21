"""Compare packed color query results with independent arithmetic models.

Optional databases are original SavedForest-derived fixtures with AData IDs
1, 2 and 4. No native Access expression execution is claimed.
"""

import argparse
import csv
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess


def packed(channels):
    rounded = [int(Decimal(str(channel)).to_integral_value(rounding=ROUND_HALF_EVEN)) for channel in channels]
    return int.from_bytes(bytes(min(channel, 255) for channel in rounded), "little")


def basic(index):
    if index == 7:
        return packed([192, 192, 192])
    if index == 8:
        return packed([128, 128, 128])

    intensity = 255 if index >= 8 else 128
    return packed([intensity * ((index >> shift) & 1) for shift in [2, 1, 0]])


def execute(cli, mode, sql, database=None, succeeds=True):
    command = [str(cli), "query", mode, sql]
    if database is not None:
        command.extend(["--database", str(database)])

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
    args = parser.parse_args()

    rng = random.Random(20261021)
    channels = [[rng.randrange(1025) for _ in range(3)] for _ in range(128)]
    channels += [[0, 0, 0], [255, 255, 255], [1, 2, 3], [2.5, 3.5, 4.5], [999, 0, 0]]
    count = 0
    for mode in ["eval", "run"]:
        for values in channels:
            sql = "SELECT RGB(" + ",".join(map(str, values)) + ") AS C"
            assert execute(args.cli, mode, sql) == [[str(packed(values))]], sql
            count += 1

        for index in range(16):
            assert execute(args.cli, mode, f"SELECT QBColor({index})") == [[str(basic(index))]]
            count += 1

        for expression in [
            "RGB(-1,0,0)", "RGB(Null,0,0)", "RGB('1',2,3)",
            "RGB(1,2)", "RGB(1,2,3,4)", "QBColor(16)", "QBColor(-1)",
            "RGB(1,2,3),1/0", "Round(1,2,3)",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1

    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).digest()
        rows = execute(
            args.cli, "run",
            "SELECT ID,RGB(ID,ID+1,ID+2),QBColor(ID) FROM AData ORDER BY ID",
            database=database,
        )
        expected = [[str(n), str(packed([n, n + 1, n + 2])), str(basic(n))] for n in [1, 2, 4]]
        assert rows == expected, (database, rows, expected)
        assert hashlib.sha256(database.read_bytes()).digest() == before
        count += len(expected) * 2

    print(f"Passed {count} independent packed-color/palette/refusal/native-row controls; native Access unrun.")


if __name__ == "__main__":
    main()
