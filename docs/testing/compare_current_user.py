"""Original unsecured-session CurrentUser controls and native-field integration.

Admin is the declared CLI expression-session profile, not a native authentication
oracle. Getter IDs establish unchanged file/row integration without running SQL.
"""

import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run
from compare_cverr import JAR_SHA256


def execute(cli, mode, sql, database=None, controls=(), success=True):
    command = [str(cli), "query", mode, sql, *controls]
    if database:
        command += ["--database", str(database)]
    environment = dict(os.environ)
    environment.update(USER="ignored-account", LOGNAME="ignored-account", USERNAME="ignored-account")
    result = subprocess.run(command, capture_output=True, text=True, check=False, env=environment)
    if not success:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []
    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar")
    count = 0
    for mode in ["eval", "run"]:
        sql = "SELECT CurrentUser(),Eval('CurrentUser()'),TypeName(CurrentUser()),VarType(CurrentUser()),Len(CurrentUser())"
        assert execute(args.cli, mode, sql) == [['"Admin"', '"Admin"', '"String"', "8", "5"]]
        count += 5
        for sql in ["SELECT CurrentUser(1/0)", "SELECT CurrentUser(*)", "SELECT CurrentUser(),1/0", "SELECT VBA.CurrentUser()"]:
            execute(args.cli, mode, sql, success=False)
            count += 1
    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "ScalarSamples")
        identities = [line.split("\t")[1].removeprefix("I32:") for line in tagged.splitlines()]
        sql = "SELECT ID,CurrentUser(),Eval('CurrentUser()') FROM ScalarSamples ORDER BY ID"
        assert execute(args.cli, "run", sql, database) == [[identity, '"Admin"', '"Admin"'] for identity in identities]
        count += 2 * len(identities)
        sql = "SELECT CurrentUser(),Count(*) FROM ScalarSamples GROUP BY CurrentUser()"
        assert execute(args.cli, "run", sql, database, controls=("--collation", "ordinal")) == [['"Admin"', str(len(identities))]]
        count += 2
        sql = "SELECT DFirst('CurrentUser()','ScalarSamples'),DLast('CurrentUser()','ScalarSamples')"
        assert execute(args.cli, "run", sql, database) == [['"Admin"', '"Admin"']]
        count += 2
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    print(f"Passed {count} explicit-session/type/refusal/native-field controls; native Access authentication unrun.")


if __name__ == "__main__":
    main()
