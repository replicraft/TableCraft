"""Original Format text-layout controls; native getters supply data, not semantics.

Models use UTF-16 positions, independent ASCII slot assignment and selected Python
Unicode mappings. They do not qualify Windows Access/NLS or VBA raw-string output.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, display, run

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def quoted(text):
    return "'" + text.replace("'", "''") + "'"


def spelling(text):
    return display("T" + text.encode("utf-8").hex())


def model(source, slots, left, separator):
    raw = source.encode("utf-16-le")
    units = [raw[index:index + 2] for index in range(0, len(raw), 2)]
    missing = max(0, len(slots) - len(units))
    assigned = units if left else [None] * missing + units
    result = bytearray()
    for index, required in enumerate(slots):
        unit = assigned[index] if index < len(assigned) else None
        result.extend(unit if unit is not None else b" \0" if required else b"")
        result.extend(separator.encode("utf-16-le"))
    for unit in assigned[len(slots):]:
        result.extend(unit)
    return result.decode("utf-16-le", errors="strict")


def execute(cli, mode, sql, database=None, success=True):
    arguments = [str(cli), "query", mode, sql, "--case-locale", "und"]
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
    count = 0
    for mode in ["eval", "run"]:
        for _ in range(160):
            source = "".join(rng.choices("aB1é中🦀", k=rng.randrange(16)))
            expressions = []
            expected = []
            for left in [False, True]:
                for separator in ["", "-"]:
                    slots = [bool(rng.randrange(2)) for _ in range(rng.randrange(1, 18))]
                    mask = ("!" if left else "") + "".join(("@" if required else "&") + separator for required in slots)
                    expression = f"Format({quoted(source)},{quoted(mask)})"
                    try:
                        value = model(source, slots, left, separator)
                    except UnicodeDecodeError:
                        execute(args.cli, mode, "SELECT 1," + expression, success=False)
                        count += 1
                    else:
                        expressions.append(expression)
                        expected.append(spelling(value))
            if expressions:
                assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == [expected]
                count += len(expected)
        for source in ["straße", "ΟΣ", "İ", "a中🦀"]:
            for marker, convert in [("<", str.lower), (">", str.upper)]:
                converted = convert(source)
                mask = marker + '"label:"' + "@" * 12
                expected = "label:" + " " * max(0, 12 - len(converted.encode("utf-16-le")) // 2) + converted
                sql = f"SELECT Format({quoted(source)},{quoted(mask)})"
                assert execute(args.cli, mode, sql) == [[spelling(expected)]]
                count += 1
        for source in ["Null", "''", "'x'"]:
            expected = "missing" if source != "'x'" else " x"
            assert execute(args.cli, mode, f"SELECT Format({source},'@@;\"missing\"')") == [[spelling(expected)]]
            count += 1

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        values = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            tag = fields[8]
            values.append((fields[1].removeprefix("I32:"), None if tag == "N" else bytes.fromhex(tag[1:]).decode("utf-8")))
        for left in [False, True]:
            mask = ("!" if left else "") + "@" * 8 + ';"missing"'
            sql = f"SELECT ID,Format(Label,{quoted(mask)}) FROM ScalarSamples ORDER BY ID"
            expected = [[identity, spelling("missing" if not value else model(value, [True] * 8, left, ""))] for identity, value in values]
            assert execute(args.cli, "run", sql, database) == expected
            count += len(values)
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} text Format layout/casing/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
