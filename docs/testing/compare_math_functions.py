"""Compare numeric CLI functions with independent Python integer/decimal models.

Uses original scalar inputs plus registered native rows from read-only Jackcess.
This is arithmetic/reference-row evidence, not native Access SQL qualification.
"""
import argparse
import subprocess
from decimal import Decimal, ROUND_HALF_EVEN, ROUND_FLOOR, ROUND_DOWN
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "database"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    before = digest(args.database)
    controls = 0
    for mode in ["eval", "run"]:
        for value in ["-8.4", "-2.5", "0", "1.25", "1.35", "8.4", "9007199254740993"]:
            number = Decimal(value)
            sql = f"SELECT Abs(CDec({value})) AS A,Sgn(CDec({value})) AS S,Int(CDec({value})) AS I,Fix(CDec({value})) AS F,Round(CDec({value}),1) AS R"
            values = [abs(number), int(number.compare(0)), number.to_integral_value(rounding=ROUND_FLOOR), number.to_integral_value(rounding=ROUND_DOWN), number.quantize(Decimal("0.1"), rounding=ROUND_HALF_EVEN)]
            actual = run(args.cli, "query", mode, sql).splitlines()
            assert actual[0] == "A\tS\tI\tF\tR"
            assert list(map(Decimal, actual[1].split("\t"))) == values, (mode, value)
            controls += 1
    for index in [None, "Number"]:
        cursor = ["--index", "ROULETTE_TB", index] if index else ["ROULETTE_TB"]
        text = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                   RECIPES / "TableDataOracle.java", args.database, *cursor)
        rows = [line.split("\t") for line in text.splitlines()]
        numbers = [int(row[1].removeprefix("I32:")) for row in rows]
        options = ["--database", args.database]
        if index:
            options += ["--index", "ROULETTE_TB", index]
        cases = [
            ("SELECT ORDR_NBR AS N,Abs(ORDR_NBR-20) AS A,Sgn(ORDR_NBR-20) AS S FROM ROULETTE_TB ORDER BY N",
             "N\tA\tS", [(n, abs(n-20), (n>20)-(n<20)) for n in sorted(numbers)]),
            ("SELECT ORDR_NBR AS N,Int(CDec(ORDR_NBR)/CDec(4)) AS I,Fix(CDec(ORDR_NBR)/CDec(4)) AS F,Round(CDec(ORDR_NBR)/CDec(4)) AS R FROM ROULETTE_TB ORDER BY N",
             "N\tI\tF\tR", [(n, int((Decimal(n)/4).to_integral_value(rounding=ROUND_FLOOR)), int((Decimal(n)/4).to_integral_value(rounding=ROUND_DOWN)), int((Decimal(n)/4).to_integral_value(rounding=ROUND_HALF_EVEN))) for n in sorted(numbers)]),
        ]
        for sql, header, expected in cases:
            result = run(args.cli, "query", "run", sql, *options)
            assert result == header + "\n" + "".join("\t".join(map(str, row)) + "\n" for row in expected), (index, sql)
            controls += 1
    for sql in ["SELECT 1,Abs(CInt(-32768))", "SELECT Round(NULL,1/0)", "SELECT 1,Round(2,-1)"]:
        result = subprocess.run([str(args.cli), "query", "run", sql], capture_output=True, text=True)
        assert result.returncode != 0 and result.stdout == "", sql
        controls += 1
    assert digest(args.database) == before
    print(f"Passed {controls} scalar/reference-row controls; unchanged source {before}")


if __name__ == "__main__":
    main()
