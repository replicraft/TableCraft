"""Compare indexed General equality queries with independent inert key/getter answers."""

import argparse
import csv
from io import StringIO
import json
import os
from pathlib import Path
import subprocess
import tempfile

from compare_large_indexes import JAR_SHA256
from compare_large_tables import digest
from compare_null_indexes import INPUT_HASHES
from compare_text_equality_candidates import LITERALS

RECIPES = Path(__file__).resolve().parent
POOLS = [
    "--collation", "database", "--format", "tsv",
    "--native-bytes", "268435456", "--native-steps", "100000000",
    "--comparison-bytes", "134217728", "--comparison-steps", "100000000",
    "--execution-steps", "100000000",
]


def changed(options, flag, value):
    result = list(options)
    result[result.index(flag) + 1] = value
    return result


def literal_sql(value):
    return "'" + value.replace("'", "''") + "'"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "jar", "directory"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    controls = 0
    oracle_cases = 0

    def query(path, sql, options, failure=None):
        nonlocal controls
        result = subprocess.run(
            [str(args.cli), "query", "run", sql, "--database", str(path), *options],
            capture_output=True, text=True, encoding="utf-8", check=False,
        )
        controls += 1
        if failure is not None:
            assert result.returncode == 2 and not result.stdout, (sql[:100], result.stderr)
            assert failure in result.stderr.lower(), result.stderr
            return None
        assert result.returncode == 0 and not result.stderr, (sql[:100], result.stderr)
        return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))

    with tempfile.TemporaryDirectory(prefix="tablecraft-equality-query-oracle-") as temporary:
        subprocess.run([
            "javac", "-cp", str(args.jar), "-d", temporary,
            str(RECIPES / "TextEqualityCandidateOracle.java"), str(RECIPES / "IndexOracle.java"),
        ], check=True)
        java = ["java", "-Dstdout.encoding=UTF-8", "-cp", os.pathsep.join((str(args.jar), temporary))]
        for profile in ["V2000", "V2007", "V2016", "V2019"]:
            extension = "mdb" if profile == "V2000" else "accdb"
            for version in [0, 1]:
                label = f"Text-{profile}-{version}"
                path = args.directory / f"{label}.{extension}"
                original = digest(path)
                assert original == INPUT_HASHES[label]
                expected_by_literal = {}
                ordinal_a = None
                for literal in [*LITERALS, "a'b"]:
                    reference = subprocess.check_output([
                        *java, "com.healthmarketscience.jackcess.impl.TextEqualityCandidateOracle",
                        str(path), "TextAsc", str(version), literal.encode().hex(),
                    ], text=True, encoding="utf-8")
                    rows = [row.split("\t") for row in reference.splitlines()]
                    identities = sorted(int(row[1].removeprefix("I32:")) for row in rows if row[0] == "E")
                    expected = [["ID"], *[[str(value)] for value in identities]]
                    expected_by_literal[literal] = expected
                    if literal == "a":
                        ordinal_a = [["ID"], *[[str(value)] for value in sorted(
                            int(row[1].removeprefix("I32:")) for row in rows if row[0] == "C" and row[2] == "T61")]]
                    sql = "SELECT ID FROM TextIndexed WHERE Label=" + literal_sql(literal)
                    for options in [POOLS, [*POOLS, "--auto-index"],
                                    [*POOLS, "--index", "TextIndexed", "TextAsc"],
                                    [*POOLS, "--index", "TextIndexed", "TextDesc"]]:
                        assert query(path, sql, options) == expected, (label, repr(literal), options[-1])
                    parameter_sql = "PARAMETERS P TEXT; SELECT ID FROM TextIndexed WHERE Label=P"
                    assert query(path, parameter_sql, [*POOLS, "--auto-index", "--param", "P=text:" + literal]) == expected
                    oracle_cases += 1

                sql = "SELECT ID FROM TextIndexed WHERE Label='Straße'"
                expected = expected_by_literal["Straße"]
                limited = [*POOLS, "--native-rows", "999"]
                query(path, sql, limited, "limit")
                assert query(path, sql, [*limited, "--auto-index"]) == expected
                assert query(path, sql, [*limited, "--index", "TextIndexed", "TextDesc"]) == expected
                assert query(path, sql.replace("ID FROM", "TOP 3 ID FROM"), [*POOLS, "--auto-index"]) == expected[:4]
                assert query(path, "SELECT First(ID) AS F,Last(ID) AS L FROM TextIndexed WHERE Label='Straße'", [*POOLS, "--auto-index"]) == [["F", "L"], [expected[1][0], expected[-1][0]]]
                query(path, sql + " UNION ALL SELECT Count(*) FROM TextIndexed", [*limited, "--auto-index"], "limit")
                assert query(path, sql + " UNION ALL SELECT Count(*) FROM TextIndexed", [*POOLS, "--auto-index"]) == [*expected, ["1000"]]
                query(path, sql + " AND 1/0=0", [*POOLS, "--auto-index"], "division")
                query(path, sql.replace("Label=", "Label<"), [*limited, "--auto-index"], "limit")
                query(path, sql.replace("Label=", "Left(Label,1)="), [*limited, "--auto-index"], "limit")
                ordinal = changed(POOLS, "--collation", "ordinal")
                assert query(path, "SELECT ID FROM TextIndexed WHERE Label='a'", [*ordinal, "--auto-index"]) == ordinal_a
                query(path, "SELECT ID FROM TextIndexed WHERE Label='a'", [*ordinal, "--auto-index", "--native-rows", "999"], "limit")
                query(path, sql, [*changed(POOLS, "--comparison-bytes", "1"), "--auto-index"], "limit")
                query(path, sql, [*changed(POOLS, "--comparison-steps", "1"), "--auto-index"], "limit")
                assert query(path, "PARAMETERS P TEXT; SELECT ID FROM TextIndexed WHERE Label=P", [*POOLS, "--auto-index", "--param", "P=null:"]) == [["ID"]]
                long_sql = "SELECT ID FROM TextIndexed WHERE Label=" + literal_sql("x" * 256)
                assert query(path, long_sql, [*POOLS, "--auto-index"]) == [["ID"]]
                query(path, long_sql, [*limited, "--auto-index"], "limit")

                metadata = subprocess.check_output([*java, "IndexOracle", str(path), "TextIndexed"], text=True)
                root = next(int(row.split("\t")[4]) for row in metadata.splitlines() if row.split("\t")[1] == "TextAsc")
                image = bytearray(path.read_bytes())
                image[root * 4096] = 0
                with tempfile.TemporaryDirectory(prefix="tablecraft-equality-query-corrupt-") as directory:
                    candidate = Path(directory) / path.name
                    candidate.write_bytes(image)
                    query(candidate, sql, [*POOLS, "--auto-index"], "source")
                    assert query(candidate, sql, POOLS) == expected
                assert digest(path) == original
                print(label, "passed", flush=True)
    print(json.dumps({"query_controls": controls, "oracle_cases": oracle_cases, "native_access": "unrun"}))


if __name__ == "__main__":
    main()
