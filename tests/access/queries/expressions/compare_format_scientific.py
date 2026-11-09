"""Original Decimal models for scientific Format and comma scaling.

Jackcess getters supply native ID/Currency/NULL fields without evaluating Format.
These controls are independent evidence, not Microsoft Access/NLS qualification.
"""

import argparse
import csv
from decimal import Decimal, ROUND_HALF_UP, localcontext
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, display, run

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def spelling(text):
    return display("T" + text.encode("utf-8").hex())


def scientific(value, whole, places, width, marker="E", sign=True, shift=0):
    value = value.scaleb(shift)
    exponent = abs(value).adjusted() - whole + 1 if value else 0
    mantissa = abs(value).scaleb(-exponent).quantize(Decimal(1).scaleb(-places), rounding=ROUND_HALF_UP)
    if mantissa >= Decimal(10) ** whole:
        mantissa /= 10
        exponent += 1
    text = format(mantissa, f"0{whole + bool(places) + places}.{places}f")
    prefix = "-" if value < 0 and mantissa else ""
    suffix = ("-" if exponent < 0 else "+" if sign else "") + str(abs(exponent)).zfill(width)
    return prefix + text + marker + suffix


def scaled(value, commas, places, percent=False):
    value = value.scaleb((2 if percent else 0) - commas * 3)
    rounded = abs(value).quantize(Decimal(1).scaleb(-places), rounding=ROUND_HALF_UP)
    return ("-" if value < 0 and rounded else "") + format(rounded, f".{places}f") + ("%" if percent else "")


def execute(cli, mode, expressions, database=None, success=True):
    sql = "SELECT " + ",".join(expressions)
    if database:
        sql += " FROM ScalarSamples ORDER BY ID"
    arguments = [str(cli), "query", mode, sql, "--number-format", "en-US"]
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
    values = [Decimal(value) for value in ["0", "-0.0001", "9.995", "99.95", "999.995", "0.0000000000000000000000000001", "79228162514264337593543950335", "4999999999999999999999999999.9", "5000000000000000000000000000.0"]]
    values += [Decimal(rng.randrange(-10**24, 10**24)).scaleb(-rng.randrange(29)) for _ in range(80)]
    count = 0
    for mode in ["eval", "run"]:
        for value in values:
            operand = f"CDec('{value:f}')"
            probes = []
            for whole, places, width, marker, sign in [(1, 0, 1, "E", True), (1, 2, 2, "E", True), (3, 4, 3, "e", False)]:
                mask = "0" * whole + ("." + "0" * places if places else "") + marker + ("+" if sign else "-") + "0" * width
                probes.append((f"Format({operand},'{mask}')", scientific(value, whole, places, width, marker, sign)))
            for commas, places, percent in [(1, 2, False), (2, 0, False), (10, 0, True), (1, 31, False)]:
                mask = "0" + "," * commas + ("." + "0" * places if places else "") + ("%" if percent else "")
                probes.append((f"Format({operand},'{mask}')", scaled(value, commas, places, percent)))
            probes.append((f"Format({operand},'0,.00E+00')", scientific(value, 1, 2, 2, shift=-3)))
            for start in range(0, len(probes), 4):
                batch = probes[start:start + 4]
                assert execute(args.cli, mode, [item[0] for item in batch]) == [[spelling(item[1]) for item in batch]], value
                count += len(batch)
        for expression in ["Format(1,'0E+')", "Format(1,'0E00')", "Format(1,'0.00,,')", "Format(1,'0E+00E+0')"]:
            execute(args.cli, mode, ["1", expression], success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        expected = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            identity = int(fields[1].removeprefix("I32:"))
            money = None if fields[5] == "N" else Decimal(fields[5].removeprefix("C")).scaleb(-4)
            expected.append([str(identity), spelling(scientific(Decimal(identity), 1, 2, 2)), spelling("" if money is None else scaled(money, 1, 2))])
        assert execute(args.cli, "run", ["ID", "Format(ID,'0.00E+00')", "Format([Money],'0,.00')"], database) == expected
        count += 2 * len(expected)
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} scientific/scaling/rounding/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    with localcontext() as context:
        context.prec = 1000
        main()
