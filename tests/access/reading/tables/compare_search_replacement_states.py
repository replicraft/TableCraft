"""Independent binary search/replacement models and expression-state controls.

Native integrations read registered original fixtures only; they do not execute
Microsoft Access SQL or qualify native locale/error precedence.
"""

import argparse
import hashlib
from pathlib import Path
import subprocess

from compare_error_messages import execute
from compare_large_indexes import INPUT_HASHES, registered_input_hash
from compare_replacement import model, spelling
from compare_string_search import position, quoted


def refused(cli, mode, sql, diagnostic, database=None, controls=()):
    command = [str(cli), "query", mode, sql, *controls]
    if database:
        command += ["--database", str(database)]

    result = subprocess.run(command, capture_output=True, text=True, check=False)
    assert result.returncode == 2 and not result.stdout, (sql, result)
    assert diagnostic in result.stderr, (sql, result.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--native-directory", type=Path)
    args = parser.parse_args()
    count = 0

    for mode in ["eval", "run"]:
        for source in ["", "banana", "é中", "😀a😀", "e\u0301e", "a-b'a"]:
            for needle in ["", "a", "an", "😀"]:
                sql = (
                    f"SELECT InStr(1,{quoted(source)},{quoted(needle)},0),"
                    f"InStrRev({quoted(source)},{quoted(needle)}),"
                    f"Replace({quoted(source)},{quoted(needle)},'X')"
                )
                expected = [[
                    str(position(source, needle, 1, False)),
                    str(position(source, needle, -1, True)),
                    spelling(model(source, needle, "X", 1, -1)),
                ]]
                assert execute(args.cli, mode, sql) == expected, (sql, expected)
                count += 3

        for expression, diagnostic in [
            ("InStr(CVErr(1),1/0)", "division by zero"),
            ("InStrRev(CVErr(1),1/0)", "division by zero"),
            ("InStr(NULL,CVErr(1),'a',-2)", "domain"),
            ("InStrRev(CVErr(1),'a',0)", "domain"),
            ("InStr(1,123,CVErr(1),0)", "number-format"),
            ("Replace(CVErr(1),'a',1/0)", "division by zero"),
            ("Replace(CVErr(1),'a','X',0)", "domain"),
            ("Replace('a','a','X',0,CVErr(1))", "type mismatch"),
            ("Replace(NULL,CVErr(1),'X')", "invalid use of NULL"),
            ("Replace(123,CVErr(1),'X')", "number-format"),
            ("Replace('',CVErr(1),'X')", "type mismatch"),
            ("Replace(CVErr(1),'a','X',1,-1,3)", "domain"),
        ]:
            refused(args.cli, mode, "SELECT 1," + expression, diagnostic)
            count += 1

        sql = (
            "SELECT InStr(1,CVErr(1),NULL,0),InStrRev(NULL,CVErr(1)),"
            "InStrRev(CStr(CVErr(11)),'11'),Replace(CStr(CVErr(11)),'11','5')"
        )
        assert execute(args.cli, mode, sql) == [["NULL", "NULL", "7", spelling("Error 5")]]
        count += 4

    if args.native_directory:
        controls = [
            "--index", "IndexedRows", "IdAsc",
            "--native-bytes", "67108864",
            "--native-rows", "100000",
            "--native-steps", "10000000",
            "--execution-steps", "10000000",
        ]
        for suffix, expected_hash in INPUT_HASHES.items():
            database = args.native_directory / ("index-" + suffix)
            expected_hash = registered_input_hash(database, expected_hash)
            assert hashlib.sha256(database.read_bytes()).hexdigest() == expected_hash

            sql = (
                "SELECT InStr(1,CStr(CVErr(ID+10)),'11',0),"
                "InStrRev(CStr(CVErr(ID+10)),'11'),"
                "Replace(CStr(CVErr(ID+10)),'11','5') "
                "FROM IndexedRows WHERE ID=1"
            )
            assert execute(args.cli, "run", sql, database, controls=controls) == [
                ["7", "7", spelling("Error 5")]
            ]
            count += 3

            for expression, diagnostic in [
                ("InStr(CVErr(ID),1/(ID-1))", "division by zero"),
                ("InStrRev(CVErr(ID),'a',0)", "domain"),
                ("Replace(CVErr(ID),'a',1/(ID-1))", "division by zero"),
                ("Replace(NULL,CVErr(ID),'X')", "invalid use of NULL"),
            ]:
                sql = "SELECT 1," + expression + " FROM IndexedRows WHERE ID=1"
                refused(args.cli, "run", sql, diagnostic, database, controls)
                count += 1

            assert hashlib.sha256(database.read_bytes()).hexdigest() == expected_hash

    print(f"Passed {count} independent search/replacement/state controls; native Access unrun.")


if __name__ == "__main__":
    main()
