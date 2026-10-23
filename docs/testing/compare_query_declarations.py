"""Independent SQL integral bounds and declaration-propagation controls.

This checks the explicit engineering policy, not Microsoft Access equivalence.
"""

import argparse
import csv
import datetime
from decimal import Decimal, ROUND_HALF_EVEN
import itertools
import hashlib
from fractions import Fraction
from io import StringIO
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES
from compare_large_indexes import INPUT_HASHES
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    parser.add_argument("--jar", type=Path, help="Optional reviewed Jackcess raw getters for native controls")
    args = parser.parse_args()
    controls = 0
    if args.jar is not None:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256

    def check(expression, expected, tag=None, declared=True, source=None,
              number_format=None, error=None, random_seed=None, clock=None, like_mode=None):
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
        if random_seed is not None:
            command += ["--random-seed", str(random_seed)]
        if clock is not None:
            command += ["--clock", clock]
        if like_mode is not None:
            command += ["--like-mode", like_mode, "--collation", "ordinal"]
        if declared:
            command.append("--declared-integral")
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if expected is None:
            assert result.returncode == 2 and not result.stdout, (sql, result)
            if error is not None:
                assert error in result.stderr, (sql, result.stderr, error)
        elif isinstance(expected, float):
            assert result.returncode == 0 and not result.stderr, (sql, result)
            rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert rows[0] == (["N"] if tag is None else ["N", "T"]), (sql, rows)
            assert len(rows) == 2, (sql, rows)
            assert struct.pack(">d", float(rows[1][0])) == struct.pack(">d", expected), (sql, rows, expected)
            if tag is not None:
                assert rows[1][1] == str(tag), (sql, rows, tag)
        else:
            output = f"N\n{expected}\n" if tag is None else f"N\tT\n{expected}\t{tag}\n"
            assert result.returncode == 0 and result.stdout == output and not result.stderr, (
                sql, result, output
            )
        controls += 1

    def check_truth_quotient(expression, truth, variant, **options):
        sql = f"CInt(-32768)\\({expression})"
        if not truth:
            check(sql, None, error="division by zero", **options)
        elif variant:
            check(sql, 32768, 3, **options)
        else:
            check(sql, None, error="numeric overflow", **options)

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
    for expression, value in [("Len('a')", 2147483648), ("LenB('a')", 2147483649)]:
        check(f"{expression}+CLng(2147483647)", value, 5)
        check(f"(SELECT Q.N+CLng(2147483647) FROM (SELECT {expression} AS N) AS Q)", value, 5)
        check(f"CLng({expression})+CLng(2147483647)", None, error="numeric overflow")
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
                check(expression, None, number_format="en-us", error="division by zero" if divisor == 0 else "numeric overflow")
                continue
            quotient = abs(a) // abs(divisor)
            if (a < 0) != (divisor < 0):
                quotient = -quotient
            value = quotient if operator == "\\" else a - quotient * divisor
            if low <= value <= high:
                check(expression, value, width, number_format="en-us")
            elif not (av or bv or forced_variant):
                check(expression, None, number_format="en-us", error="numeric overflow")
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
    for function in ["CDate", "CVDate"]:
        for a, b in itertools.product([-32768, -7, 0, 7, 32767], ["-1", "-2.5", "0", "0.5", "1"]):
            rounded_case(f"CInt({a})", f"{function}({b})", a, b, 2,
                         forced_variant=function == "CVDate")
        expression = f"CInt(-32768)\\{function}(-1)"
        check(expression, 32768, 3, declared=False)
        check(f"(SELECT CInt(-32768)\\Q.N FROM (SELECT {function}(-1) AS N) AS Q)",
              None if function == "CDate" else 32768,
              None if function == "CDate" else 3,
              error="numeric overflow" if function == "CDate" else None)
    # Signature categories are independent of the same-carrier value kernel.
    for function in ["Abs", "Int", "Fix", "Round"]:
        for value, limit, cast, tag in [(255,255,"CByte",2), (32767,32767,"CInt",3)]:
            expression = f"{function}({cast}({value}))+{cast}(1)"
            check(expression, limit+1, tag)
            check(f"(SELECT Q.N+{cast}(1) FROM (SELECT {function}({cast}({value})) AS N) AS Q)",limit+1,tag)
            check(f"{cast}({function}({cast}({value})))+{cast}(1)",None,error="numeric overflow")
        maximum = struct.unpack(">f", bytes.fromhex("7f7fffff"))[0]
        check(f"{function}(CSng({maximum!r}))*CInt(2)",maximum*2,5)
    for expression in ["Sqr(CVar(1))", "Exp(CVar(0))", "Log(CVar(1))+CInt(1)",
                       "Sin(CVar(0))+CInt(1)", "Cos(CVar(0))", "Tan(CVar(0))+CInt(1)",
                       "Atn(CVar(0))+CInt(1)"]:
        check(f"CInt(-32768)\\(-({expression}))",None,error="numeric overflow")
        check(f"CInt(-32768)\\CVar(-({expression}))",32768,3)
        check(f"CInt(-32768)\\(-({expression}))",32768,3,declared=False)
    maximum = struct.unpack(">f", bytes.fromhex("7f7fffff"))[0]
    for seed in [1 << 23, 1 << 22, (1 << 24) - 1]:
        for argument in ["0", "CVar(0)"]:
            check(f"CSng({maximum!r})/Rnd({argument})",None,error="numeric overflow",random_seed=seed)
        value = float(Fraction.from_float(maximum) * (1 << 24) / seed)
        check(f"CSng({maximum!r})/CVar(Rnd(0))",value,5,random_seed=seed)
    for expression in ["Rnd(0)", "CVar(Rnd(0))"]:
        check(f"CSng({maximum!r})/{expression}",None,error="division by zero",random_seed=0)
    for expression in [
            "Pmt(CVar(0),1,-1)", "PV(CVar(0),1,-1)", "FV(CVar(0),1,-1)",
            "NPer(CVar(0),-1,1)", "IPmt(CVar(1),1,1,-1)", "PPmt(CVar(0),1,1,-1)",
            "SLN(CVar(1),0,1)", "SYD(CVar(1),0,1,1)", "DDB(CVar(1),0,1,1)",
            "Rate(CVar(1),-2,1,0,0,1)", "EuroConvert(CVar(1),'EUR','EUR')",
            "NPV(CVar(0),Array(1))", "IRR(Array(-1,2),CVar(1))", "MIRR(Array(-1,2),CVar(0),0)"]:
        check(f"CInt(-32768)\\(-({expression}))",None,error="numeric overflow")
        check(f"CInt(-32768)\\CVar(-({expression}))",32768,3)
        check(f"CInt(-32768)\\(-({expression}))",32768,3,declared=False)
        check(f"(SELECT CInt(-32768)\\(-Q.N) FROM (SELECT {expression} AS N) AS Q)",
              None,error="numeric overflow")
    for expression in ["DateSerial(1899,12,29)", "TimeSerial(-24,0,0)",
                       "DateAdd('d',-1,CDate(0))", "DateValue(CDate(-1))", "-TimeValue(CDate(0.75))"]:
        check(f"CInt(-32768)\\({expression})",32768,3)
        check(f"CInt(-32768)\\CDate({expression})",None,error="numeric overflow")
        check(f"(SELECT CInt(-32768)\\Q.N FROM (SELECT {expression} AS N) AS Q)",32768,3)
    for expression in ["IsNumeric(CVar(1))", "IsDate(CVar(CDate(1)))", "IsError(CVErr(5))", "IsArray(Array(1))"]:
        check(f"CInt(-32768)\\({expression})",None,error="numeric overflow")
        check(f"CInt(-32768)\\CVar({expression})",32768,3)
    check("CInt(-32768)\\IsEmpty(CVar(1))",None,error="division by zero")
    fixed_clock = "1899-12-29T06:00:00"
    for expression in ["Date()", "Now()", "-Time()*CInt(4)"]:
        check(f"CInt(-32768)\\({expression})",32768,3,clock=fixed_clock)
        check(f"CInt(-32768)\\CDate({expression})",None,error="numeric overflow",clock=fixed_clock)
    check("CInt(-32768)\\(-Timer()/CDbl(21600))",None,error="numeric overflow",clock=fixed_clock)
    check("CInt(-32768)\\CVar(-Timer()/CDbl(21600))",32768,3,clock=fixed_clock)
    # Independent signed/unsigned bit models. The result category follows the
    # operand declarations, not the integer value returned by the logical kernel.
    for cast, low, high, tag, widened_tag in [
            ("CByte", 0, 255, 17, 2), ("CInt", -32768, 32767, 2, 3),
            ("CLng", -2147483648, 2147483647, 3, 5)]:
        edges = sorted({low, 0, high})
        for value in edges:
            complement = (~value & 255) if cast == "CByte" else ~value
            for variant in [False, True]:
                operand = f"{cast}({value})"
                if variant:
                    operand = f"CVar({operand})"
                expression = f"(Not {operand})+{cast}(1)"
                if complement == high and not variant:
                    check(expression,None,error="numeric overflow")
                else:
                    check(expression,complement+1,widened_tag if complement == high else tag)
        for operator in ["And", "Or", "Xor", "Eqv", "Imp"]:
            for left, right in itertools.product(edges, repeat=2):
                value = {"And": left & right, "Or": left | right,
                         "Xor": left ^ right, "Eqv": ~(left ^ right), "Imp": ~left | right}[operator]
                if cast == "CByte":
                    value &= 255
                for variant_left, variant_right in [(False, False), (True, False), (False, True)]:
                    a, b = f"{cast}({left})", f"{cast}({right})"
                    if variant_left:
                        a = f"CVar({a})"
                    if variant_right:
                        b = f"CVar({b})"
                    expression = f"({a} {operator} {b})+{cast}(1)"
                    if value == high and not (variant_left or variant_right):
                        check(expression,None,error="numeric overflow")
                    else:
                        check(expression,value+1,widened_tag if value == high else tag)
    check("CInt(7)\\(Not Count(*))",None,error="declaration")
    check("CInt(7)\\(Not CVar(Count(*)))",-3,20)
    comparisons = {
        "=": lambda a, b: a == b, "<>": lambda a, b: a != b,
        "<": lambda a, b: a < b, "<=": lambda a, b: a <= b,
        ">": lambda a, b: a > b, ">=": lambda a, b: a >= b,
    }
    for operator, compare in comparisons.items():
        for left, right in [(0, 0), (0, 1), (1, 0), (-1, 0)]:
            truth = compare(left, right)
            for variant_left, variant_right in itertools.product([False, True], repeat=2):
                a, b = f"CInt({left})", f"CInt({right})"
                if variant_left:
                    a = f"CVar({a})"
                if variant_right:
                    b = f"CVar({b})"
                comparison = f"{a}{operator}{b}"
                variant = variant_left or variant_right
                for expression, active in [(comparison, truth), (f"Not ({comparison})", not truth)]:
                    sql = f"CInt(-32768)\\({expression})"
                    if not active:
                        check(sql,None,error="division by zero")
                    elif variant:
                        check(sql,32768,3)
                    else:
                        check(sql,None,error="numeric overflow")
                check(f"CInt(-32768)\\CBool({comparison})",None,
                      error="numeric overflow" if truth else "division by zero")
                check(f"CInt(-32768)\\({comparison})",32768 if truth else None,
                      3 if truth else None,declared=False,error=None if truth else "division by zero")
        for left, right in [(0, 0), (0, 65535), (65535, 0)]:
            truth = compare(left, right)
            expression = f"CVErr({left}){operator}CVErr({right})"
            check(f"CInt(-32768)\\({expression})",32768 if truth else None,
                  3 if truth else None,error=None if truth else "division by zero")
            check(f"CInt(-32768)\\CBool({expression})",None,
                  error="numeric overflow" if truth else "division by zero")
    check("CInt(-32768)\\(Count(*)=CInt(1))",None,error="declaration")
    check("CInt(-32768)\\(Count(*)=CVar(CInt(1)))",32768,3)
    for number in ["CByte(255)", "CInt(-32768)", "CLng(2147483647)",
                   "CLngLng(9223372036854775807)", "CSng(-0.0)", "CDbl(1e308)",
                   "CCur(-0.0001)", "CDec(0.0000000000000000000000000001)"]:
        for text in ["''", "'1'", "'-99999999999999999999'", "'bad'", "'1,5'"]:
            for operator, compare in comparisons.items():
                # Compare independent category ranks, not the payload spellings.
                for left, right, wanted in [(number, text, compare(0, 1)), (text, number, compare(1, 0))]:
                    check(f"CVar({left}){operator}CVar({right})","TRUE" if wanted else "FALSE",11)
    for expression in ["CVar(1)='1'", "1=CVar('1')", "1='1'", "CVar(True)=CVar('-1')"]:
        check(expression,None,error="number-format")
        check(expression,"TRUE",11,number_format="en-us")
    check("CVar(1)=CVar('1')","TRUE",11,declared=False,number_format="en-us")
    check("CVar(1)=CVar('1')",None,declared=False,error="number-format")
    for mode, star in [("ansi89", "*"), ("ansi92", "%")]:
        for label, pattern, truth in [("abc", "a" + star, True), ("z", "a" + star, False),
                                      ("", star, True), ("b", "[ab]", True), ("c", "[ab]", False)]:
            for variant_left, variant_right in itertools.product([False, True], repeat=2):
                left, right = f"'{label}'", f"'{pattern}'"
                if variant_left:
                    left = f"CVar({left})"
                if variant_right:
                    right = f"CVar({right})"
                variant = variant_left or variant_right
                for negated in [False, True]:
                    expression = f"{left} {'NOT LIKE' if negated else 'LIKE'} {right}"
                    check_truth_quotient(expression, truth != negated, variant, like_mode=mode)
                check_truth_quotient(f"CBool({left} LIKE {right})", truth, False, like_mode=mode)
                check_truth_quotient(f"{left} LIKE {right}", truth, True,
                                     declared=False, like_mode=mode)
        check("CInt(-32768)\\(First('a') LIKE 'a')",None,like_mode=mode,error="declaration")
        check("CInt(-32768)\\(First('a') LIKE CVar('a'))",32768,3,like_mode=mode)
        check("CVar(NULL) LIKE 'a'","NULL",like_mode=mode)
    check("CInt(-32768)\\('a' LIKE 'a')",None,error="wildcard mode")
    check("CInt(-32768)\\(CVar('a') LIKE '[')",None,like_mode="ansi89",error="invalid LIKE pattern")
    for expression, truth in [("IsNull(CVar(NULL))", True), ("CVar(NULL) IS NULL", True),
                              ("CVar(1) IS NOT NULL", True), ("IsNull(CVErr(0))", False),
                              ("Count(*) IS NOT NULL", True)]:
        check(f"CInt(-32768)\\({expression})",None,error="numeric overflow" if truth else "division by zero")
        check(f"CInt(-32768)\\CVar({expression})",32768 if truth else None,3 if truth else None,
              error=None if truth else "division by zero")
    if args.native_directory is not None:
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
            check("CInt(-32768)\\CDate(-ID)", None, source=source, error="numeric overflow")
            check("CInt(-32768)\\CVDate(-ID)", 32768, 3, source=source)
            if args.jar is not None:
                output = subprocess.check_output([
                    "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                    str(RECIPES / "TableDataOracle.java"), str(source), "IndexedRows",
                ], text=True, encoding="utf-8")
                stored = [line.split("\t") for line in output.splitlines()]
                assert len(stored) == 20000
                selected = [row for row in stored if row[1] == "I32:2"]
                assert len(selected) == 1
                record = selected[0]
                assert record[3] == "U255" and record[4] == "I16:32767"
                assert record[7].startswith("F")
                assert struct.unpack(">f", bytes.fromhex(record[7][1:]))[0] == 30.5
            for function in ["DLookup", "DFirst", "DLast", "DMin", "DMax"]:
                for column, operator, value, tag in [
                        ("ByteValue", "+CByte(1)", 256, 2),
                        ("ShortValue", "+CInt(1)", 32768, 3),
                        ("SingleValue", "*CInt(2)", 61, 4)]:
                    call = f"{function}('{column}','IndexedRows','ID=2')"
                    check(call + operator, value, tag, source=source)
                call = f"{function}('CInt(32767)','IndexedRows','ID=1')"
                check(f"CInt({call})+CInt(1)", None, source=source, error="numeric overflow")
            check("CInt(-32768)\\DAvg('CInt(-1)','IndexedRows','ID<=2')", 32768, 3, source=source)
            check("CInt(-32768)\\DSum('CInt(-1)','IndexedRows','ID=1')", 32768, 20, source=source)
            check("CInt(7)\\DCount('*','IndexedRows','ID<=2')", 3, 20, source=source)
            for function in ["DVar", "DVarP", "DStDev", "DStDevP"]:
                check(f"CInt(-32768)\\({function}('CInt(1)','IndexedRows','ID<=2')-CInt(1))",
                      32768, 3, source=source)
            check("DMin('ShortValue','IndexedRows','ID<0')+CInt(1)", "NULL", source=source)
            for function in ["Abs", "Int", "Fix", "Round"]:
                check(f"{function}(CLng(ID))+CLng(2147483647)",2147483648,5,source=source)
                check(f"CLng({function}(ID))+CLng(2147483647)",None,source=source,error="numeric overflow")
            for expression in ["SLN(CVar(ID),0,1)", "PV(0,1,-ID)", "NPV(0,Array(ID))"]:
                check(f"CInt(-32768)\\(-({expression}))",None,source=source,error="numeric overflow")
                check(f"CInt(-32768)\\CVar(-({expression}))",32768,3,source=source)
            for expression in ["DateAdd('d',-ID,CDate(0))", "DateValue(CDate(-ID))"]:
                check(f"CInt(-32768)\\({expression})",32768,3,source=source)
                check(f"CInt(-32768)\\CDate({expression})",None,source=source,error="numeric overflow")
            check("CInt(-32768)\\IsNumeric(CVar(ID))",None,source=source,error="numeric overflow")
            check("CInt(-32768)\\CVar(IsNumeric(CVar(ID)))",32768,3,source=source)
            for expression in ["ByteValue Or CByte(255)", "Not (ByteValue And CByte(0))"]:
                check(f"({expression})+CByte(1)",None,source=source,error="numeric overflow")
                check(f"CVar({expression})+CByte(1)",256,2,source=source)
                check(f"({expression})+CByte(1)",256,2,source=source,declared=False)
            for operator, right in [("=", 1), ("<>", 2), ("<", 2), ("<=", 1), (">", 0), (">=", 1)]:
                for left, right, variant in [
                        ("ID", f"CLng({right})", False),
                        ("CVar(ID)", f"CLng({right})", True),
                        ("ID", f"CVar(CLng({right}))", True)]:
                    expression = f"{left}{operator}{right}"
                    check(f"CInt(-32768)\\({expression})",32768 if variant else None,
                          3 if variant else None,source=source,error=None if variant else "numeric overflow")
                    check(f"CInt(-32768)\\CBool({expression})",None,source=source,error="numeric overflow")
            for column in ["ID", "ByteValue", "ShortValue", "[Money]", "SingleValue", "DoubleValue", "Amount"]:
                for operator, compare in comparisons.items():
                    check(f"CVar({column}){operator}CVar('bad')","TRUE" if compare(0, 1) else "FALSE",11,source=source)
                    check(f"CVar('bad'){operator}CVar({column})","TRUE" if compare(1, 0) else "FALSE",11,source=source)
            check('DCount(\'*\',\'IndexedRows\',\'CVar(ID)<CVar("bad")\')',20000,source=source)
            check('DCount(\'*\',\'IndexedRows\',\'CVar(ID)=CVar("1")\')',0,source=source)
            for mode, star in [("ansi89", "*"), ("ansi92", "%")]:
                for left, right, variant in [("Label", f"'{star}'", False),
                                              ("CVar(Label)", f"'{star}'", True),
                                              ("Label", f"CVar('{star}')", True)]:
                    expression = f"{left} LIKE {right}"
                    check(f"CInt(-32768)\\({expression})",32768 if variant else None,
                          3 if variant else None,source=source,like_mode=mode,error=None if variant else "numeric overflow")
                    check(f"CInt(-32768)\\CBool({expression})",None,source=source,like_mode=mode,error="numeric overflow")
            check("CInt(-32768)\\(CVar(ID) IS NOT NULL)",None,source=source,error="numeric overflow")
            check("CInt(-32768)\\CVar(ID IS NOT NULL)",32768,3,source=source)
            assert hashlib.sha256(source.read_bytes()).hexdigest() == digest
    print(f"Passed {controls} independent SQL declaration controls; native Access remains unqualified")


if __name__ == "__main__":
    main()
