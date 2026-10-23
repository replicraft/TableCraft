"""Independent nearest-even/UTF-16 slicing controls and inert native ID queries."""

import argparse
from decimal import Decimal, ROUND_HALF_EVEN
import hashlib
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_large_indexes import INPUT_HASHES
from compare_error_messages import execute, quoted


def slice_model(function, source, number):
    position = int(number.to_integral_value(rounding=ROUND_HALF_EVEN))
    raw = source.encode("utf-16-le")
    units = len(raw) // 2
    if function == "Mid":
        if position < 1:
            return None
        start = min(position - 1, units)
        end = min(start + 2, units)
    else:
        if position < 0:
            return None
        start, end = (0, min(position, units)) if function == "Left" else (max(units - position, 0), units)
    try:
        return raw[start * 2:end * 2].decode("utf-16-le", errors="strict")
    except UnicodeDecodeError:
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    rng = random.Random(20261021)
    values = [Decimal(text) for text in ["-1.5", "-0.5", "0.5", "1.5", "2.5", "3.5", "8.5"]]
    values += [Decimal(rng.randrange(-4, 32)) / 4 for _ in range(8)]
    count = 0
    for profile in ["en-US", "de-DE"]:
        for number in values:
            text = format(number, "f")
            regional = text.replace(".", ",") if profile == "de-DE" else text
            for source in ["abcdef", "é🦀中"]:
                for function in ["Left", "Right", "Mid"]:
                    expected = slice_model(function, source, number)
                    for operand in [f"CDec({text})", f"CDate({text})", f"'{regional}'"]:
                        arguments = f"'{source}',{operand}" + (",2.5" if function == "Mid" else "")
                        sql = f"SELECT {function}({arguments})"
                        for mode in ["eval", "run"]:
                            result = execute(args.cli, mode, sql, controls=("--number-format", profile), success=expected is not None)
                            if expected is not None:
                                assert result == [[quoted(expected)]], (sql, result, expected)
                            count += 1
        for sql in ["SELECT Left(NULL,1/0)", "SELECT Mid('abc','bad',1/0)", "SELECT Left('abc',2147483647.5)"]:
            execute(args.cli, "run", sql, controls=("--number-format", profile), success=False)
            count += 1
    for mode in ["eval", "run"]:
        for expression, diagnostic in [
            ("Left(CVErr(1),1/0)", "division by zero"),
            ("Right(CVErr(1),1/0)", "division by zero"),
            ("Mid('abc',CVErr(1),1/0)", "division by zero"),
            ("Mid('abc',0,CVErr(1))", "domain"),
            ("Mid('abc',1,CVErr(1))", "type mismatch"),
            ("Left(123,CVErr(1))", "number-format"),
            ("Right(CVErr(1),0)", "type mismatch"),
        ]:
            result = subprocess.run([str(args.cli), "query", mode, "SELECT 1," + expression], capture_output=True, text=True)
            assert result.returncode == 2 and not result.stdout and diagnostic in result.stderr, result
            count += 1
        sql = "SELECT Left(NULL,CVErr(1)),Right(NULL,CVErr(1)),Mid(NULL,0,CVErr(1)),Right(CStr(CVErr(11)),2)"
        assert execute(args.cli, mode, sql) == [["NULL", "NULL", "NULL", quoted("11")]]
        count += 4
    if args.native_directory:
        controls = ["--index", "IndexedRows", "IdAsc", "--native-bytes", "67108864",
                    "--native-rows", "100000", "--native-steps", "10000000", "--execution-steps", "10000000"]
        for suffix, expected_hash in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == expected_hash
            sql = "SELECT Left(NULL,CVErr(ID)),Right(NULL,CVErr(ID)),Mid(NULL,0,CVErr(ID)),Right(CStr(CVErr(ID+10)),2) FROM IndexedRows WHERE ID=1"
            assert execute(args.cli, "run", sql, database, controls=controls) == [["NULL", "NULL", "NULL", quoted("11")]]
            count += 4
            for function in ["Left", "Right", "Mid"]:
                sql = f"SELECT 1,{function}(CVErr(ID),1/(ID-1)) FROM IndexedRows WHERE ID=1"
                result = subprocess.run([str(args.cli), "query", "run", sql, "--database", str(database), *controls], capture_output=True, text=True)
                assert result.returncode == 2 and not result.stdout and "division by zero" in result.stderr, result
                count += 1
            assert hashlib.sha256(database.read_bytes()).hexdigest() == expected_hash
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = sorted(int(line.split("\t")[1].removeprefix("I32:")) for line in tagged.splitlines())
        expected = [[str(n), quoted(slice_model("Left", "abcdef", Decimal(n) / 2)), quoted(slice_model("Right", "abcdef", Decimal(n) / 2)), quoted(slice_model("Mid", "abcdef", Decimal(n) + Decimal("0.5")))] for n in identities]
        sql = "SELECT ID,Left('abcdef',CDbl(ID)/2),Right('abcdef',CDec(ID)/2),Mid('abcdef',CStr(ID)&'.5','2.5') FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database, controls=("--number-format", "en-US")) == expected
        count += len(identities) * 3
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} independent rounded-count/UTF-16/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
