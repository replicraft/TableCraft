"""Independent SQL integral bounds and declaration-propagation controls.

This checks the explicit engineering policy, not Microsoft Access equivalence.
"""

import argparse
import datetime
from decimal import Decimal, ROUND_HALF_EVEN
import itertools
import hashlib
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    controls = 0

    def check(expression, expected, tag=None, declared=True, source=None, number_format=None):
        nonlocal controls
        sql = f"SELECT {expression} AS N"
        if tag is not None:
            sql += f",VarType({expression}) AS T"
        if source is not None:
            sql += " FROM IndexedRows WHERE ID=1"
        command = [str(args.cli.resolve()), "query", "run", sql, "--format", "tsv"]
        if source is not None:
            command += ["--database", str(source), "--index", "IndexedRows", "IdAsc",
                        "--native-bytes", "67108864", "--native-rows", "100000",
                        "--native-steps", "10000000", "--execution-steps", "10000000"]
        if number_format is not None:
            command += ["--number-format", number_format]
        if declared:
            command.append("--declared-integral")
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
        else:
            output = f"N\n{expected}\n" if tag is None else f"N\tT\n{expected}\t{tag}\n"
            assert result.returncode == 0 and result.stdout == output and not result.stderr, (
                sql, result, output
            )
        controls += 1

    widths = [("CByte", 0, 255, 17), ("CInt", -32768, 32767, 2),
              ("CLng", -2147483648, 2147483647, 3)]
    for cast, low, high, tag in widths:
        pairs = [(high, 1), (high, 2), (low, 1), (low, -1 if low < 0 else 2),
                 (0, 0), (7, 2)]
        for (left, right), operator, variant in itertools.product(
                pairs, ["+", "-", "*", "\\", "Mod"], [False, True]):
            a, b = f"{cast}({left})", f"{cast}({right})"
            if variant:
                a = f"CVar({a})"
            expression = f"{a} {operator} {b}"
            if operator in ["\\", "Mod"]:
                if right == 0:
                    check(expression, None)
                    continue
                quotient = abs(left) // abs(right)
                if (left < 0) != (right < 0):
                    quotient = -quotient
                value = quotient if operator == "\\" else left - quotient * right
            elif operator == "+":
                value = left + right
            elif operator == "-":
                value = left - right
            else:
                value = left * right
            if low <= value <= high:
                check(expression, value, tag)
            elif not variant:
                check(expression, None)
            elif -32768 <= value <= 32767:
                check(expression, value, 2)
            elif -2147483648 <= value <= 2147483647:
                check(expression, value, 3)
            else:
                check(expression, value, 5)

    for wrapper in ["CVar({})", "IIf(True,{},CInt(0))", "Nz({},CInt(0))",
                    "Choose(1,{})", "Switch(True,{})"]:
        value = wrapper.format("CInt(32767)")
        check(f"{value}+CInt(1)", 32768, 3)
        check(f"(SELECT Q.N+CInt(1) FROM (SELECT {value} AS N) AS Q)", 32768, 3)
    check("CInt(32767)+CInt(1)", 32768, 3, declared=False)
    check("-CInt(-32768)", None)
    check("-CVar(CInt(-32768))", 32768, 3)
    check("First(CInt(1))+CInt(1)", None)
    check("CVar(NULL)+(1/0)", None)
    for expression in ["VarType(1)", "Asc('A')", "AscW('A')", "AscB('A')"]:
        check(f"{expression}+CInt(32767)", None)
        check(f"(SELECT Q.N+CInt(32767) FROM (SELECT {expression} AS N) AS Q)", None)
    for expression in ["Len('a')", "LenB('a')"]:
        check(f"{expression}+CLng(2147483647)", None)
        check(f"(SELECT Q.N+CLng(2147483647) FROM (SELECT {expression} AS N) AS Q)", None)
    for expression, value in [("Sgn(1)", 32768), ("StrComp('b','a',0)", 32768),
                              ("Year(#2026-01-01#)", 34793)]:
        check(f"{expression}+CInt(32767)", value, 3)
        check(f"(SELECT Q.N+CInt(32767) FROM (SELECT {expression} AS N) AS Q)", value, 3)
    for expression in ["InStr(1,'a','a',0)", "InStrB(1,'a','a',0)"]:
        check(f"{expression}+CLng(2147483647)", 2147483648, 5)
        check(f"(SELECT Q.N+CLng(2147483647) FROM (SELECT {expression} AS N) AS Q)", 2147483648, 5)
    for expression in ["InStrRev('abc','a',-1,0)", "RGB(1,0,0)",
                       "RGB(CVar(1),0,0)", "QBColor(1)"]:
        check(f"{expression}+CLng(2147483647)", None)
        check(f"(SELECT Q.N+CLng(2147483647) FROM (SELECT {expression} AS N) AS Q)", None)
    date = datetime.datetime(2026, 10, 22, 15, 16, 17)
    parts = [("Month", date.month), ("Day", date.day), ("Hour", date.hour),
             ("Minute", date.minute), ("Second", date.second),
             ("Weekday", (date.weekday() + 1) % 7 + 1)]
    for function, value in parts:
        expression = f"{function}(#2026-10-22 15:16:17#)"
        check(f"{expression}+CInt(32767)", 32767 + value, 3)
        check(f"(SELECT Q.N+CInt(32767) FROM (SELECT {expression} AS N) AS Q)", 32767 + value, 3)
        check(f"CInt({expression})+CInt(32767)", None)
    for expression, value, tag, limit, cast in [
            ("DatePart('yyyy',#2026-10-22#)", date.year, 3, 32767, "CInt"),
            ("DateDiff('d',#2026-10-21#,#2026-10-22#)", 1, 5, 2147483647, "CLng")]:
        check(f"{expression}+{cast}({limit})", limit + value, tag)
        check(f"(SELECT Q.N+{cast}({limit}) FROM (SELECT {expression} AS N) AS Q)", limit + value, tag)
    check("DatePart('yyyy',NULL)+CInt(1)", "NULL")
    check("InStrRev(NULL,'a')+CLng(1)", "NULL")
    check("RGB(NULL,1/0,0)+CLng(1)", None)
    # Independent decimal half-even rounding, then exact truncated integer math.
    def rounded_case(left, right, a, b, width, forced_variant=False):
        bounds = {2: (-32768, 32767), 3: (-2147483648, 2147483647)}
        low, high = bounds[width]
        for operator, av, bv in itertools.product(["\\", "Mod"], [False, True], [False, True]):
            l = f"CVar({left})" if av else left
            r = f"CVar({right})" if bv else right
            expression = f"{l} {operator} {r}"
            divisor = int(Decimal(b).to_integral_value(rounding=ROUND_HALF_EVEN))
            if not low <= divisor <= high or divisor == 0:
                check(expression, None, number_format="en-us")
                continue
            quotient = abs(a) // abs(divisor)
            if (a < 0) != (divisor < 0):
                quotient = -quotient
            value = quotient if operator == "\\" else a - quotient * divisor
            if low <= value <= high:
                check(expression, value, width, number_format="en-us")
            elif not (av or bv or forced_variant):
                check(expression, None, number_format="en-us")
            else:
                tag = 3 if -2147483648 <= value <= 2147483647 else 5
                check(expression, value, tag, number_format="en-us")

    for a in [-32768, -7, 7, 32767]:
        for right, b in [("CDbl(-1)", "-1"), ("CDbl(-2.5)", "-2.5"),
                         ("CCur(2.5)", "2.5"),
                         ("'2.50000000000000000000000000000000001'", "2.50000000000000000000000000000000001"),
                         ("'0.49'", "0.49"), ("#1900-01-01 12:00:00#", "2.5")]:
            rounded_case(f"CInt({a})", right, a, b, 2)
    for left in ["CDbl(-2147483648)", "CSng(-2147483648)", "CCur(-2147483648)",
                 "'-2147483648'", "CDec(-2147483648)"]:
        rounded_case(left, "CInt(-1)", -2147483648, "-1", 3,
                     forced_variant=left.startswith("CDec"))
    if args.native_directory is not None:
        from compare_large_indexes import INPUT_HASHES
        for suffix, digest in INPUT_HASHES.items():
            source = args.native_directory / ("index-" + suffix)
            assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
            # Original independently generated LONG ID=1; validate both its
            # carrier and the declaration policy without mutating stored bytes.
            check("ID", 1, 3, source=source)
            check("ID+CLng(2147483647)", None, source=source)
            check("CVar(ID)+CLng(2147483647)", 2147483648, 5, source=source)
            check("InStrRev('abc','a',-1,0)+ID+CLng(2147483647)", None, source=source)
            check("DatePart('d',#2026-10-22#)+ID+CLng(2147483647)", 2147483670, 5, source=source)
            check("CInt(ID)+CInt(32767)", None, source=source)
            check("CLng(-2147483648)\\CDbl(-ID)", None, source=source)
            check("CLng(-2147483648)\\CVar(CDbl(-ID))", 2147483648, 5, source=source)
            check("ID+CLng(2147483647)", 2147483648, 5, declared=False, source=source)
            assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
    print(f"Passed {controls} independent SQL declaration controls; native Access remains unqualified")


if __name__ == "__main__":
    main()
