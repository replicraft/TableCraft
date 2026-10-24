"""Compare eager Switch/Choose CLI output with independent selection models.

Original scalar controls and read-only Jackcess rows provide independent evidence;
this recipe does not execute Microsoft Access SQL or qualify Variant coercion.
"""
import argparse
import subprocess
import struct
from decimal import Decimal, ROUND_HALF_EVEN
from pathlib import Path

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest
from compare_union_types import JAR_SHA256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "database"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256, "Unreviewed engineering artifact"
    before = digest(args.database)
    controls = 0
    for mode in ["eval", "run"]:
        for index in [
            "0.5", "1.5", "2.5", "3.5", "-1", "9007199254740993", "1e30",
            "0.50000001", "1.49999999", "2.50000001", "3.49999999",
            "0.5000001", "1.4999999", "2.5000002", "3.4999998",
        ]:
            single = struct.unpack("<f", struct.pack("<f", float(Decimal(index))))[0]
            rounded = int(Decimal.from_float(single).to_integral_value(rounding=ROUND_HALF_EVEN))
            expected = display("T" + ["first", "second", "third"][rounded-1].encode().hex()) if 1 <= rounded <= 3 else "NULL"
            expressions = [index, f"'{index}'", f"CSng('{index}')"]
            if abs(Decimal(index)) <= Decimal("79228162514264337593543950335"):
                expressions.append(f"CDec('{index}')")
            for expression in expressions:
                sql = f"SELECT Choose({expression},'first','second','third') AS Choice"
                assert run(args.cli, "query", mode, sql, "--number-format", "invariant") == "Choice\n" + expected + "\n", (mode, index)
                controls += 1
        for expression, diagnostic in [
            ("Choose(1e100,1)", "numeric overflow"),
            ("Choose('1e1000',1)", "numeric overflow"),
            ("Choose(1e100,1,1/0)", "division by zero"),
        ]:
            result = subprocess.run([
                str(args.cli), "query", mode, "SELECT 1," + expression,
                "--number-format", "invariant",
            ], capture_output=True, text=True)
            assert result.returncode == 2 and not result.stdout
            assert diagnostic in result.stderr, (mode, expression, result.stderr)
            controls += 1
        for sql, expected in [
            ("SELECT Switch(False,1,True,CInt(7),True,9) AS Choice", "7"),
            ("SELECT Switch(True,NULL,True,9) AS Choice", "NULL"),
            ("SELECT Switch(NULL,1,False,2) AS Choice", "NULL"),
        ]:
            assert run(args.cli, "query", mode, sql) == "Choice\n" + expected + "\n"
            controls += 1
        for sql in ["SELECT Choose(1,7,1/0)", "SELECT Switch(True,7,False,1/0)", "SELECT Choose('bad',7,1/0)"]:
            result = subprocess.run([str(args.cli), "query", mode, sql], capture_output=True, text=True)
            assert result.returncode != 0 and result.stdout == "", (mode, sql)
            controls += 1
    for index in [None, "Number"]:
        cursor = ["--index", "ROULETTE_TB", index] if index else ["ROULETTE_TB"]
        text = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                   RECIPES / "TableDataOracle.java", args.database, *cursor)
        numbers = sorted(int(line.split("\t")[1].removeprefix("I32:")) for line in text.splitlines())
        options = ["--database", args.database]
        if index:
            options += ["--index", "ROULETTE_TB", index]
        sql = "SELECT ORDR_NBR AS N,Switch(ORDR_NBR<10,1,ORDR_NBR<20,2,True,3) AS S,Choose((ORDR_NBR Mod 3)+1,10,20,30) AS C FROM ROULETTE_TB ORDER BY N"
        expected = "N\tS\tC\n" + "".join(f"{n}\t{1 if n < 10 else 2 if n < 20 else 3}\t{[10,20,30][n%3]}\n" for n in numbers)
        assert run(args.cli, "query", "run", sql, *options) == expected, index
        controls += 1
        sql = "SELECT ORDR_NBR AS N,Choose(CDbl(ORDR_NBR Mod 3)+0.49999999,10,20,30) AS C FROM ROULETTE_TB ORDER BY N"
        expected = "N\tC\n" + "".join(
            f"{n}\t{['NULL', '20', '20'][n % 3]}\n" for n in numbers
        )
        assert run(args.cli, "query", "run", sql, *options) == expected, index
        controls += 1
    assert digest(args.database) == before
    print(f"Passed {controls} scalar/reference-row controls; unchanged source {before}")


if __name__ == "__main__":
    main()
