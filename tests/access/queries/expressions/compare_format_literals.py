"""Independent digit-layout and Decimal controls for interleaved numeric Format.

Expected values come from whole-number padding/slicing and Decimal rounding,
not a second parser for TableCraft masks. Optional native inputs use inert
Jackcess getters; no Access SQL or VBA is executed.
"""

import argparse
from decimal import Decimal, localcontext
import hashlib
from pathlib import Path
import random

from compare_browsing import RECIPES, run
from compare_format import JAR_SHA256, execute, fixed, spelling
from compare_format_scientific import scientific


def layout(value, left, right, places, german=False):
    text = fixed(value, places)
    sign = "-" if text.startswith("-") else ""
    whole, _, fraction = text.lstrip("-").partition(".")
    whole = whole.zfill(left + right)
    boundary = len(whole) - right
    text = sign + whole[:boundary] + "🦀;" + whole[boundary:]
    if places:
        text += ("," if german else ".") + fraction[:1] + "u" + fraction[1:]
    return text


def mask_for(left, right, places):
    text = "0" * left + '"🦀;"' + "0" * right
    return text + ('.0"u"' + "0" * (places - 1) if places else "")


def numeric(value, german=False):
    text = format(value, "f")
    return "CDec('" + (text.replace(".", ",") if german else text) + "')"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    values = [Decimal(text) for text in ["0", "-0.0001", "9.995", "-999.995", "123456.25"]]
    values += [Decimal(rng.randrange(-10**12, 10**12)).scaleb(-4) for _ in range(32)]
    count = 0
    for mode in ["eval", "run"]:
        for profile in ["en-US", "de-DE"]:
            german = profile == "de-DE"
            probes = []
            for value in values:
                for left, right, places in [(2, 2, 2), (1, 3, 3), (1, 1, 0)]:
                    mask = mask_for(left, right, places)
                    probes.append((f"Format({numeric(value, german)},'{mask}')",
                                   spelling(layout(value, left, right, places, german))))
                base = scientific(value, 1, 2, 2)
                mantissa, exponent = base.split("E")
                whole, fraction = mantissa.split(".")
                expected = whole + "m" + ("," if german else ".") + fraction[:1] + "f" + fraction[1:] + "E" + exponent
                probes.append((f"Format({numeric(value, german)},'0\"m\".0\"f\"0E+00')", spelling(expected)))
                exponent_sign, exponent_digits = exponent[:1], exponent[1:]
                decorated_exponent = exponent_sign + exponent_digits[:-1] + "z" + exponent_digits[-1:]
                decorated = expected.rsplit("E", 1)[0] + "E" + decorated_exponent
                probes.append((f"Format({numeric(value, german)},'0\"m\".0\"f\"0E+0\"z\"0')", spelling(decorated)))
            for start in range(0, len(probes), 6):
                batch = probes[start:start + 6]
                sql = "SELECT " + ",".join(expression for expression, _ in batch)
                assert execute(args.cli, mode, sql, profile) == [[expected for _, expected in batch]]
                count += len(batch)
            for value, mask, expected in [
                ("1", "0#", "01"), ("0", "0#", "00"),
                ("1", '#0"x"#', "0x1"), ("0", '#"x"#0', "x0"),
                ("12", r"0\%0", "1%2"), ("12", "0%0", "120%0"),
                ("0", '0.#"x"#', "0,x" if german else "0.x"),
            ]:
                assert execute(args.cli, mode, f"SELECT Format({value},'{mask}')", profile) == [[spelling(expected)]]
                count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        for profile in ["en-US", "de-DE"]:
            expected = []
            for line in tagged.splitlines():
                fields = line.split("\t")
                identity = int(fields[1].removeprefix("I32:"))
                money = None if fields[5] == "N" else Decimal(fields[5].removeprefix("C")).scaleb(-4)
                padded = str(identity).zfill(4)
                label = padded[:-2] + "-" + padded[-2:]
                amount = "" if money is None else layout(money, 2, 2, 2, profile == "de-DE")
                expected.append([str(identity), spelling(label), spelling(amount)])
            sql = "SELECT ID,Format(ID,'00-00'),Format([Money],'" + mask_for(2, 2, 2) + "') FROM ScalarSamples ORDER BY ID"
            assert execute(args.cli, "run", sql, profile, database) == expected
            count += 2 * len(expected)
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent numeric-layout/rounding/native-field controls; native Access unrun.")


if __name__ == "__main__":
    with localcontext() as context:
        context.prec = 1000
        main()
