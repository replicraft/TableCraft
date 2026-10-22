"""Independent rounded character-code and comparison-option controls.

Python character/codec and Decimal models supply expectations; only TableCraft
executes SQL. Native controls use read-only getters and unchanged source hashes.
"""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    count = 0
    codes = [Decimal(text) for text in ["-32768.5", "-1", "0", "1.5", "65.5", "66.5", "127.5", "128", "255", "255.5", "55295.5", "55296", "57343.5", "65535"]]
    for profile in ["en-US", "de-DE"]:
        for number in codes:
            text = format(number, "f")
            regional = text.replace(".", ",") if profile == "de-DE" else text
            code = int(number.to_integral_value(rounding=ROUND_HALF_EVEN))
            unit = code % 65536
            for operand in [f"CDate({text})", f"'{regional}'"]:
                probes = [
                    (f"Asc(Chr({operand}))", code if 0 <= code <= 127 else None, ()),
                    (f"AscW(Chr({operand}))", ord(bytes([code]).decode("cp1252")) if 0 <= code <= 255 else None, ("--character-code-page", "1252")),
                    (f"AscB(ChrB({operand}))", code if 0 <= code <= 255 else None, ()),
                    (f"AscW(ChrW({operand}))", unit - 65536 if unit >= 32768 else unit, ()),
                ]
                if not -32768 <= code <= 65535 or 0xD800 <= unit <= 0xDFFF:
                    expression, _, extra = probes[-1]
                    probes[-1] = (expression, None, extra)
                for expression, expected, extra in probes:
                    for mode in ["eval", "run"]:
                        result = execute(args.cli, mode, "SELECT " + expression,
                                         controls=("--number-format", profile, *extra), success=expected is not None)
                        if expected is not None:
                            assert result == [[str(expected)]], (expression, result, expected)
                        count += 1
        for number in [Decimal(text) for text in ["-1.5", "-1.25", "-0.5", "0.5", "1.5", "2.5", "3.5", "1033"]]:
            code = int(number.to_integral_value(rounding=ROUND_HALF_EVEN))
            text = str(number).replace(".", ",") if profile == "de-DE" else str(number)
            for operand in [f"CDate({number})", f"'{text}'"]:
                for function, sql, valid, expected in [
                    ("StrComp", f"StrComp('10','2',{operand})", code == 0, "-1"),
                    ("StrComp", f"StrComp(NULL,'x',{operand})", code in [-1, 0, 1, 2], "NULL"),
                    ("InStr", f"InStr(1,NULL,'x',{operand})", code in [-1, 0, 1, 2] or code > 2, "NULL"),
                    ("InStrRev", f"InStrRev(NULL,'x',-1,{operand})", code in [-1, 0, 1, 2], "NULL"),
                    ("InStrB", f"InStrB(1,NULL,'x',{operand})", code in [-1, 0, 1, 2], "NULL"),
                ]:
                    for mode in ["eval", "run"]:
                        result = execute(args.cli, mode, "SELECT " + sql, controls=("--number-format", profile), success=valid)
                        if valid:
                            assert result == [[expected]], (function, sql, result)
                        count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = sorted(int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines())
        expected = [[str(n), str(int((Decimal(n + 64) + Decimal("0.5")).to_integral_value(rounding=ROUND_HALF_EVEN))), str(int((Decimal(n + 64) + Decimal("0.5")).to_integral_value(rounding=ROUND_HALF_EVEN))), str(n + 64), "-1", "2", '"bXana"'] for n in identities]
        sql = "SELECT ID,Asc(Chr(CStr(ID+64)&'.5')),AscW(ChrW(CDate(ID+64.5))),AscB(ChrB(CStr(ID+64))),StrComp('10','2',CStr(0)),InStr(1,'abc','b',CDate(0)),Replace('banana','an','X',1,1,CStr(0)) FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database, controls=("--number-format", "en-US")) == expected
        count += len(identities) * 6
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent character/mode/rounding/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
