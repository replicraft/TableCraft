"""Independent numeric/date formatter controls with rounded Text options.

Reuse original Python Decimal/Gregorian reference models, never TableCraft's typed
formatter as the oracle. Native getters remain read-only and source hashes checked.
"""

import argparse
import datetime as dt
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute, quoted
from compare_styled_number import expected as numeric_model
from compare_date_format import expected as date_model


def number_text(number, profile):
    text = str(number)
    return text.replace(".", ",") if profile == "de-de" else text


def rounded(text):
    return int(Decimal(text).to_integral_value(rounding=ROUND_HALF_EVEN))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    count = 0
    options = [("2.5", "-1", "0", "0"), ("-1.25", "-2", "-2", "-2"), ("0.5", "-0.5", "-1.25", "-0.5"), ("2.5", "1", "0", "0")]
    values = [Decimal(text) for text in ["-1234.125", "-0.125", "0", "0.125", "1.005", "10000"]]
    for profile in ["en-us", "de-de"]:
        for value in values:
            for kind in ["FormatNumber", "FormatPercent", "FormatCurrency"]:
                for settings in options:
                    places, leading, parentheses, grouping = map(rounded, settings)
                    valid = places >= -1 and places <= 255 and all(value in [-2, -1, 0] for value in [leading, parentheses, grouping])
                    places = 2 if places == -1 else places
                    leading = True if leading == -2 else bool(leading)
                    parentheses = kind == "FormatCurrency" and profile == "en-us" if parentheses == -2 else bool(parentheses)
                    grouping = True if grouping == -2 else bool(grouping)
                    expected = numeric_model(value, kind, profile, places, leading, parentheses, grouping) if valid else None
                    operands = ",".join(quoted(number_text(option, profile)) for option in settings)
                    sql = f"SELECT {kind}(CDec({quoted(number_text(value, profile))}),{operands})"
                    for mode in ["eval", "run"]:
                        result = execute(args.cli, mode, sql, controls=("--number-format", profile), success=valid)
                        if valid:
                            assert result == [[quoted(expected)]], (sql, result, expected)
                        count += 1
        value = dt.datetime(2025, 1, 2, 3, 4, 5)
        for date_profile in ["iso", "en-us", "de-de"]:
            for selector in ["-0.5", "0.5", "1.5", "2.5", "3.5", "4.5", "5.5"]:
                code = rounded(selector)
                sql = f"SELECT FormatDateTime(#2025-01-02 03:04:05#,{quoted(number_text(selector, profile))})"
                for mode in ["eval", "run"]:
                    result = execute(args.cli, mode, sql, controls=("--number-format", profile, "--date-format", date_profile), success=0 <= code <= 4)
                    if 0 <= code <= 4:
                        assert result == [[quoted(date_model(value, date_profile, code))]], (sql, result)
                    count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        references = []
        for line in tagged.splitlines():
            fields = line.split("\t")
            if fields[-1] != "N":
                references.append((int(fields[1].removeprefix("I32:")), dt.datetime.fromisoformat(fields[-1].removeprefix("Y"))))
        for profile in ["en-us", "de-de"]:
            selector = quoted(number_text("2.5", profile))
            sql = f"SELECT ID,FormatDateTime(Stamp,{selector}),FormatNumber(ID,{selector},'-1','0','0'),FormatCurrency(ID,{selector},'-1','0','0') FROM ScalarSamples WHERE Stamp IS NOT NULL ORDER BY ID"
            expected = [[str(n), quoted(date_model(stamp, "iso", 2)), quoted(numeric_model(Decimal(n), "FormatNumber", profile, 2, True, False, False)), quoted(numeric_model(Decimal(n), "FormatCurrency", profile, 2, True, False, False))] for n, stamp in references]
            assert execute(args.cli, "run", sql, database, controls=("--number-format", profile, "--date-format", "iso")) == expected
            count += len(references) * 3
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent numeric/date/options/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
