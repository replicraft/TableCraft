"""Original integer/IEEE model controls for the explicit Basic Rnd profile.

The recurrence is checked with unbounded Python integer arithmetic, including
Single seed bits. Getters provide native Int32 fields without evaluating Rnd.
This does not establish Windows Access generator or query invocation behavior.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import struct
import subprocess

from compare_browsing import RECIPES, run

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def single(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def model(state, argument):
    argument = single(1 if argument is None else argument)
    if argument < 0:
        bits = struct.unpack("<I", struct.pack("<f", argument))[0]
        state = (bits + bits // 2**24) % 2**24
    if argument != 0:
        state = (1140671485 * state + 12820163) % 2**24
    return state, state / 2**24


def compare(row, expected):
    assert len(row) == len(expected)
    for text, wanted in zip(row, expected):
        assert struct.pack("<f", float(text)) == struct.pack("<f", wanted), (text, wanted)


def execute(cli, mode, sql, seed=327680, database=None, success=True):
    arguments = [str(cli), "query", mode, sql, "--random-seed", str(seed)]
    if database:
        arguments += ["--database", str(database)]
    result = subprocess.run(arguments, capture_output=True, text=True, check=False)
    if not success:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []
    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    seeds = [0, 1, 327680, 16777215] + [rng.randrange(2**24) for _ in range(64)]
    count = 0
    for mode in ["eval", "run"]:
        for seed in seeds:
            state = seed
            expressions = []
            expected = []
            for argument in [0, None, 0, 1, 100, -1, 0, None, -rng.randrange(1, 100000), None, -0.5, 0]:
                expressions.append("Rnd()" if argument is None else f"Rnd({argument})")
                state, value = model(state, argument)
                expected.append(value)
            compare(execute(args.cli, mode, "SELECT " + ",".join(expressions), seed)[0], expected)
            count += len(expected)
        for expression in ["Rnd(Null)", "Rnd(1/0)", "Rnd(1/0,1)", "Rnd(*)"]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            count += 1
        for seed in [-1, 16777216]:
            execute(args.cli, mode, "SELECT Rnd()", seed, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines()]
        expected = []
        for identity in identities:
            state, first = model(0, -identity)
            _, repeated = model(state, 0)
            _, second = model(state, 1)
            expected.append([identity, first, repeated, second])
        rows = execute(args.cli, "run", "SELECT ID,Rnd(-ID),Rnd(0),Rnd(1) FROM ScalarSamples ORDER BY ID", database=database)
        assert len(rows) == len(expected)
        for row, wanted in zip(rows, expected):
            assert int(row[0]) == wanted[0]
            compare(row[1:], wanted[1:])
            count += 3
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} Rnd state/seed/Single/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
