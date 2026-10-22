"""Independent Decimal/Fraction predicate models and inert native getter controls.

The reference never evaluates SQL. These compare the explicit materialized numeric
profile, not complete Access declared/Variant type inference or native execution.
"""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_error_messages import execute


def booleans(values):
    return ["TRUE" if value else "FALSE" for value in values]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    count = 0
    for profile in ["en-us", "de-de"]:
        for quarter in range(-17,18):
            value = Decimal(quarter) / 4
            text = str(value).replace(".", ",") if profile == "de-de" else str(value)
            sql = f"SELECT 2='{text}',2<>'{text}',2<'{text}',2<='{text}',2>'{text}',2>='{text}','{text}'=2"
            expected = [booleans([2 == value, 2 != value, 2 < value, 2 <= value, 2 > value, 2 >= value, value == 2])]
            for mode in ["eval", "run"]:
                assert execute(args.cli, mode, sql, controls=("--number-format",profile)) == expected
                count += 7
        for mode in ["eval", "run"]:
            exact = "0,00015" if profile == "de-de" else "0.00015"
            amount = Decimal(exact.replace(",", "."))
            text_round = amount.quantize(Decimal("0.0001"),rounding=ROUND_HALF_EVEN)
            float_round = Decimal.from_float(float(amount)).quantize(Decimal("0.0001"),rounding=ROUND_HALF_EVEN)
            assert execute(args.cli, mode, f"SELECT CCur(0.0002)='{exact}',CCur(0.0002)=CDbl(0.00015),CDec(0.1)=CDbl(0.1)", controls=("--number-format",profile)) == [booleans([Decimal("0.0002") == text_round, Decimal("0.0002") == float_round, True])]
            count += 3
            for sql in ["SELECT 2='bad'", "SELECT 'bad'=(1/0)", "SELECT 2 IN ('2',1/0)"]:
                execute(args.cli, mode, sql, controls=("--number-format",profile), success=False)
                count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        original = database.read_bytes()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp",args.jar, RECIPES / "TableDataOracle.java",database,"ScalarSamples")
        identities = [int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines()]
        for profile, threshold in [("en-us","1.5"),("de-de","1,5")]:
            controls=("--number-format",profile)
            expected=[[str(value)] for value in identities if Decimal(value)>Decimal("1.5")]
            assert execute(args.cli,"run",f"SELECT ID FROM ScalarSamples WHERE ID>'{threshold}' ORDER BY ID",database,controls=controls)==expected
            assert execute(args.cli,"run",f"SELECT ID FROM ScalarSamples WHERE ID BETWEEN '{threshold}' AND '3' ORDER BY ID",database,controls=controls)==[[str(value)] for value in identities if Decimal("1.5")<=value<=3]
            assert execute(args.cli,"run","SELECT ID FROM ScalarSamples WHERE ID IN ('1','3') ORDER BY ID",database,controls=controls)==[[str(value)] for value in identities if value in [1,3]]
            assert execute(args.cli,"run","SELECT DCount('*','ScalarSamples',\"ID>'1'\")",database,controls=controls)==[[str(sum(value>1 for value in identities))]]
            count += len(expected)+sum(Decimal("1.5")<=value<=3 for value in identities)+sum(value in [1,3] for value in identities)+1
        assert database.read_bytes()==original
        print(database.name,len(original),hashlib.sha256(original).hexdigest())
    print(f"Passed {count} independent Decimal/predicate/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
