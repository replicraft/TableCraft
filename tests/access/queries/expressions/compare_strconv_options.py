"""Independent StrConv option rounding, ASCII representation and locale controls.

Decimal and fixed ASCII/locale models supply expectations. Only TableCraft runs
SQL; native references use inert typed ID getters and unchanged source hashes.
"""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute, quoted


VALID_FLAGS = {0, 1, 2, 3, 64, 65, 66, 67, 128, 129, 130, 131}
SUPPORTED_PROBES = {1032, 1033, 1055}


def nearest(number):
    return int(number.to_integral_value(rounding=ROUND_HALF_EVEN))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    flags = [Decimal(text) for text in ["-1.5", "-0.5", "0.5", "1.5", "2.5", "3.5", "63.5", "64.5", "65.5", "127.5", "128.5", "129.5", "191.5", "32767.5"]]
    locales = [Decimal(text) for text in ["1033", "1055", "1032.5", "1033.5"]]
    count = 0
    for profile in ["en-US", "de-DE"]:
        for flag in flags:
            for locale in locales:
                code, lcid = nearest(flag), nearest(locale)
                flag_text, locale_text = str(flag), str(locale)
                if profile == "de-DE":
                    flag_text, locale_text = flag_text.replace(".", ","), locale_text.replace(".", ",")
                arguments = [(f"CDate({flag})", f"CDate({locale})"), (f"'{flag_text}'", f"'{locale_text}'")]
                for mode_flag, locale_id in arguments:
                    valid = code in VALID_FLAGS and lcid in SUPPORTED_PROBES
                    bytes_count = 4 if code >= 128 else 16 if code >= 64 else 8
                    first_byte = 97 if code % 4 == 2 else 65
                    sql = f"SELECT LenB(StrConv('Ab C',{mode_flag},{locale_id})),AscB(StrConv('Ab C',{mode_flag},{locale_id}))"
                    for mode in ["eval", "run"]:
                        result = execute(args.cli, mode, sql, controls=("--number-format", profile), success=valid)
                        if valid:
                            assert result == [[str(bytes_count), str(first_byte)]], (sql, result)
                        count += 2
        for sql in ["SELECT StrConv(NULL,'bad',1/0)","SELECT StrConv(NULL,'4','1033')","SELECT StrConv(NULL,'1','1041')"]:
            execute(args.cli, "run", sql, controls=("--number-format", profile), success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = sorted(int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines())
        expected = [[str(n), "8", str(97 if n == 2 else 65), quoted("ı" if n == 2 else "i")] for n in identities]
        sql = "SELECT ID,LenB(StrConv('Ab C',CStr(ID),'1033')),AscB(StrConv('Ab C',CDate(ID),CDate(1033))),StrConv('I','1.5',CStr(IIf(ID=2,1055,1033))) FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database, controls=("--number-format", "en-US")) == expected
        count += len(identities) * 3
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent StrConv/rounding/representation/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
