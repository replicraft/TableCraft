"""Check the current numeric Format profile against independent Decimal models.

Original HyperlinkFixture files supply native Int32/NULL fields through Jackcess
getters. These controls do not execute Microsoft Access or qualify its NLS rules.
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


def fixed(value, places, grouped=False, percent=False, german=False):
    if percent:
        value *= 100
    rounded = abs(value).quantize(Decimal(1).scaleb(-places), rounding=ROUND_HALF_UP)
    text = format(rounded, f"{',' if grouped else ''}.{places}f")
    if german:
        text = text.translate(str.maketrans({",": ".", ".": ","}))
    if value < 0 and rounded:
        text = "-" + text
    return text + ("%" if percent else "")


def execute(cli, mode, sql, profile="en-US", database=None, succeeds=True):
    arguments = [str(cli), "query", mode, sql, "--number-format", profile]
    if database is not None:
        arguments += ["--database", str(database)]
    result = subprocess.run(arguments, capture_output=True, text=True, check=False)
    if not succeeds:
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
    count = 0
    rng = random.Random(20261021)
    values = [Decimal(value) for value in ["0", "-0.0001", "2.5", "-2.5", "999.995", "0.125", "1234.5678"]]
    values += [Decimal(rng.randrange(-100_000_000, 100_000_000)).scaleb(-4) for _ in range(128)]
    for mode in ["eval", "run"]:
        for value in values:
            expressions = []
            expected = []
            for places in [0, 1, 2, 4]:
                for grouped in [False, True]:
                    mask = ("#,##0" if grouped else "0") + ("." + "0" * places if places else "")
                    expressions.append(f"Format({value},'{mask}')")
                    expected.append(spelling(fixed(value, places, grouped)))
            expressions.append(f"Format({value},'0.00%')")
            expected.append(spelling(fixed(value, 2, percent=True)))
            assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == [expected], value
            count += len(expected)
        for value, mask, expected in [
            ("0.125", r"0.00\%", "0.13%"),
            ("7", '"000"0', "0007"),
            ("7", '"🦀; "0', "🦀; 7"),
        ]:
            sql = f"SELECT Format({value},'{mask}')"
            assert execute(args.cli, mode, sql) == [[spelling(expected)]]
            count += 1
        for profile, german in [("en-US", False), ("de-DE", True)]:
            expected = [spelling(fixed(Decimal("1234.5"), 2, grouped=True, german=german))]
            assert execute(args.cli, mode, "SELECT Format(1234.5,'#,##0.00')", profile) == [expected]
            count += 1
        for value, expected in [("12", "12.00"), ("-12", "(12.00)"), ("0", "zero"), ("Null", "missing")]:
            sql = f"SELECT Format({value},'0.00;(0.00);\"zero\";\"missing\"')"
            assert execute(args.cli, mode, sql) == [[spelling(expected)]]
            count += 1
        for sql in ["SELECT Format(23)", "SELECT Format(23,,1,1)", "SELECT Format(23,'',,1)"]:
            assert execute(args.cli, mode, sql) == [[spelling("23")]]
            count += 1
        for expression in ["Format(1,'0E+00')", "Format(1,'0,,')", "Format(1,'0-0')", "Format(1,'yyyy-mm-dd')", "Format(1,'0',1,1,1/0)", "Format(Null,'0',1/0)", "Format(1,Null)", "Format(*)"]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "Hyperlinks")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines()]
        expected = [[str(identity), spelling(str(identity).zfill(3)), spelling(f"({identity})"), spelling(fixed(Decimal(identity) / 8, 1, percent=True))] for identity in identities]
        sql = "SELECT ID,Format(ID,'000'),Format(-ID,'0;(0)'),Format(ID/8.0,'0.0%') FROM Hyperlinks ORDER BY ID"
        assert execute(args.cli, "run", sql, database=database) == expected
        count += 3 * len(expected)
        null_ids = [line.split("\t")[1].removeprefix("I32:") for line in tagged.splitlines() if line.split("\t")[2] == "N"]
        sql = "SELECT ID,Format(Link,'0;0;0;\"missing\"') FROM Hyperlinks WHERE Link Is Null ORDER BY ID"
        assert execute(args.cli, "run", sql, database=database) == [[identity, spelling("missing")] for identity in null_ids]
        count += len(null_ids)
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent numeric Format/section/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    with localcontext() as context:
        context.prec = 80
        main()
