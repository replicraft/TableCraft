"""Reference-catalog and native-field integration controls for numbered Error.

The catalog comes from documented titles, not native Access execution. Jackcess
getters independently supply row IDs; no reference SQL/VBA is executed.
"""

import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256
from compare_large_indexes import INPUT_HASHES

CATALOG = RECIPES.parents[1] / "crates/query/tablecraft-expressions/data/error_messages"


def execute(cli, mode, sql, database=None, controls=(), success=True):
    command = [str(cli), "query", mode, sql, *controls]
    if database:
        command += ["--database", str(database)]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if not success:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []
    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]


def quoted(text):
    assert not any(character in text for character in '\\"\r\n\t')
    return '"' + text + '"'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    resource = (CATALOG / "en-US.ftl").read_bytes()
    provenance = json.loads((CATALOG / "provenance.json").read_text())
    assert hashlib.sha256(resource).hexdigest() == provenance["catalog_sha256"]
    messages = {}
    for line in resource.decode().splitlines():
        if line.startswith("error-"):
            key, value = line.split(" = ", 1)
            messages[key.removeprefix("error-")] = value
    assert sorted(int(key) for key in messages if key != "unknown") == [
        topic["code"] for topic in provenance["topics"]
    ]
    count = 0
    for mode in ["eval", "run"]:
        for key, message in messages.items():
            if key == "unknown":
                continue
            sql = f"SELECT Error({key}),Error$({key}),Eval('Error({key})')"
            assert execute(args.cli, mode, sql) == [[quoted(message)] * 3]
            count += 3
        for code in [1, 2, 2001, 65535, -1, -11, -65536, -2147221504, -2147483648]:
            assert execute(args.cli, mode, f"SELECT Error({code})") == [[quoted(messages["unknown"])]]
            count += 1
        assert execute(args.cli, mode, "SELECT Error(0),Error(-0.5)") == [['""', '""']]
        count += 2
        for sql in [
            "SELECT Error()", "SELECT Error(65536)", "SELECT Error(65535.5)",
            "SELECT Error(-2147483648.6)",
            "SELECT Error(NULL)", "SELECT Error(11),1/0", "SELECT Error(1,1/0)",
        ]:
            execute(args.cli, mode, sql, success=False)
            count += 1
    for mode in ["eval", "run"]:
        for function in ["Error", "Error$"]:
            for code in [0, 11, 65535]:
                result = subprocess.run([str(args.cli), "query", mode, f"SELECT 1,{function}(CVErr({code}))"], capture_output=True, text=True)
                assert result.returncode == 2 and not result.stdout and "type mismatch" in result.stderr, result
                count += 1
            result = subprocess.run([str(args.cli), "query", mode, f"SELECT 1,{function}(IIf(True,CVErr(11),1/0))"], capture_output=True, text=True)
            assert result.returncode == 2 and not result.stdout and "division by zero" in result.stderr, result
            count += 1
            assert execute(args.cli, mode, f"SELECT {function}(CLng(CVErr(11)))") == [[quoted(messages["11"])]]
            count += 1
    if args.native_directory:
        controls = ["--index", "IndexedRows", "IdAsc", "--native-bytes", "67108864",
                    "--native-rows", "100000", "--native-steps", "10000000", "--execution-steps", "10000000"]
        for suffix, expected_hash in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == expected_hash
            sql = "SELECT Error(ID+10),Error$(CLng(CVErr(ID+10))) FROM IndexedRows WHERE ID=1"
            assert execute(args.cli, "run", sql, database, controls=controls) == [[quoted(messages["11"])] * 2]
            count += 2
            for function in ["Error", "Error$"]:
                for expression, diagnostic in [("CVErr(ID)", "type mismatch"), ("IIf(True,CVErr(ID),1/(ID-1))", "division by zero"), ("First(CVErr(ID))", "type mismatch")]:
                    sql = f"SELECT 1,{function}({expression}) FROM IndexedRows WHERE ID=1"
                    result = subprocess.run([str(args.cli), "query", "run", sql, "--database", str(database), *controls], capture_output=True, text=True)
                    assert result.returncode == 2 and not result.stdout and diagnostic in result.stderr, result
                    count += 1
            assert hashlib.sha256(database.read_bytes()).hexdigest() == expected_hash
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = [line.split("\t")[1].removeprefix("I32:") for line in tagged.splitlines()]
        expected = [[identity, quoted(messages.get(str(int(identity) + 10), messages["unknown"]))]
                    for identity in identities]
        assert execute(args.cli, "run",
                       "SELECT ID,Error(ID+10) FROM ScalarSamples ORDER BY ID", database) == expected
        count += len(identities)
        assert execute(args.cli, "run",
                       "SELECT Error(11),Count(*) FROM ScalarSamples GROUP BY Error(11)",
                       database, controls=("--collation", "ordinal")) == [
                           [quoted(messages["11"]), str(len(identities))]
                       ]
        count += 2
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} reference-message/refusal/native-field controls; native Access Error execution unrun.")


if __name__ == "__main__":
    main()
