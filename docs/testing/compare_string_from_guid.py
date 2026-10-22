"""Original provisional StringFromGUID model and native-field controls.

Python UUID formats the independent expected value/byte order. Native getters
supply stored GUIDs without evaluating expressions. This is not an Access oracle.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess
import uuid

from compare_browsing import RECIPES, run

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def execute(cli, mode, sql, database=None, success=True):
    command = [str(cli), "query", mode, sql]
    if database:
        command += ["--database", str(database)]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if not success:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []
    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]


def spelling(value):
    return "{guid {" + str(value).upper() + "}}"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    values = [uuid.UUID(int=0), uuid.UUID(int=(1 << 128) - 1)]
    values += [uuid.UUID(int=rng.getrandbits(128)) for _ in range(128)]
    count = 0
    for mode in ["eval", "run"]:
        for value in values:
            literal = spelling(value)
            expected = '"' + literal + '"'
            expressions = [
                f"StringFromGUID({literal})",
                f"StringFromGUID(GUIDFromString('{value}'))",
                f"StringFromGUID(GUIDFromString('{{{str(value).upper()}}}'))",
                f"StringFromGUID(GUIDFromString('{literal}'))",
                f"GUIDFromString(StringFromGUID({literal}))",
                f"VarType(StringFromGUID({literal}))",
            ]
            assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == [
                [expected] * 4 + ["0x" + value.bytes_le.hex(), "8"]
            ]
            count += len(expressions)
        for expression in [
            "StringFromGUID(Null)", "StringFromGUID(1)", "StringFromGUID('not bytes')",
            "StringFromGUID(*)", "StringFromGUID()", "StringFromGUID(1,1/0)",
            "StringFromGUID(1/0)", "VBA.StringFromGUID(1)",
            "StringFromGUID(GUIDFromString('00000000-0000-0000-0000-000000000000')),1/0",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        expected = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            if fields[12] == "N":
                continue
            assert fields[12].startswith("G")
            value = uuid.UUID(hex=fields[12][1:])
            expected.append([fields[1].removeprefix("I32:"), '"' + spelling(value) + '"', "0x" + value.bytes_le.hex()])
        sql = "SELECT ID,StringFromGUID(G),GUIDFromString(StringFromGUID(G)) FROM ScalarSamples WHERE G IS NOT NULL ORDER BY ID"
        assert execute(args.cli, "run", sql, database) == expected
        count += 2 * len(expected)
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent provisional GUID-text/byte/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
