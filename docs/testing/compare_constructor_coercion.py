"""Independent Decimal/Gregorian controls for constructor argument coercion.

The existing original Python calendar and Fraction models provide expected DATE
results. Native getters only supply IDs; no native Access SQL is executed.
"""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_date_constructors import date_serial, time_serial
from compare_error_messages import execute, quoted


def rounded(value):
    return int(Decimal(value).to_integral_value(rounding=ROUND_HALF_EVEN))


def component(value, profile, carrier):
    if carrier == "date":
        return f"CDate({value})"
    return quoted(value.replace(".", ",") if profile == "de-de" else value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    date_cases = [("2024.5", "2.5", "28.5"), ("2024", "3", "-0.5"),
                  ("49", "2.5", "28.5"), ("2024", "-0.5", "1.5"),
                  ("10050", "1", "-32768"), ("9999", "12", "31")]
    time_cases = [("1.5", "-0.5", "2.5"), ("-1.5", "0", "0"),
                  ("0", "75.5", "-0.5"), ("25", "1.5", "1.5"),
                  ("-32768.5", "-32768", "-32768"), ("32767.4", "32767", "32767")]
    count = 0
    for mode in ["eval", "run"]:
        for profile in ["invariant", "en-us", "de-de"]:
            for carrier in ["text", "date"]:
                for function, cases, model in [("DateSerial", date_cases, date_serial),
                                               ("TimeSerial", time_cases, time_serial)]:
                    for values in cases:
                        operands = ",".join(component(value, profile, carrier) for value in values)
                        sql = f"SELECT {function}({operands})"
                        expected = model(*map(rounded, values))
                        result = execute(args.cli, mode, sql, controls=("--number-format", profile))
                        assert result == [[expected]], (sql, result, expected)
                        count += 1
            for last in [199, 2029, 2049, 9999]:
                for year in ["0.5", "29.5", "49.5", "98.5"]:
                    operands = ",".join(component(value, profile, "text") for value in [year, "2.5", "28.5"])
                    sql = f"SELECT DateSerial({operands})"
                    controls = ("--number-format", profile, "--two-digit-year-max", str(last))
                    expected = date_serial(rounded(year), 2, 28, last)
                    assert execute(args.cli, mode, sql, controls=controls) == [[expected]]
                    count += 1
            for expression in [
                "DateSerial(32767.5,'bad',NULL)", "DateSerial(NULL,'bad',NULL)",
                "DateSerial('bad',NULL,NULL)", "TimeSerial('32767.5',0,0)",
                "TimeSerial(CDate(32767.5),0,0)", "DateSerial(NULL,1,1/0)",
                "TimeSerial(NULL,1/0,0)", "DateSerial(100,1,0)",
            ]:
                execute(args.cli, mode, "SELECT 99," + expression,
                        controls=("--number-format", profile), success=False)
                count += 1
        for expression in ["DateSerial('2024',1,1)", "TimeSerial('1.5',0,0)"]:
            execute(args.cli, mode, "SELECT 99," + expression, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines()]
        for profile in ["en-us", "de-de"]:
            month, day, minute, second = [component(value, profile, "text")
                                          for value in ["2.5", "28.5", "1.5", "-0.5"]]
            projection = (f"SELECT ID,DateSerial(CStr(ID),{month},{day}),"
                          f"TimeSerial(CStr(ID),{minute},{second}) "
                          "FROM ScalarSamples ORDER BY ID")
            for last in [199, 2029]:
                expected = [[str(identity), date_serial(identity, 2, 28, last), time_serial(identity, 2, 0)]
                            for identity in identities]
                result = execute(args.cli, "run", projection, database,
                                 controls=("--number-format", profile, "--two-digit-year-max", str(last)))
                assert result == expected, (database, result, expected)
                count += len(expected) * 2
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent constructor-coercion/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
