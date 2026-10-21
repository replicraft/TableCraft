"""Check materialized CVar subtype/value identity, without native Access execution.

Optional databases are original SavedForest-derived fixtures with AData IDs
1, 2 and 4. No active database content is executed implicitly.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess


def execute(cli, mode, sql, database=None, succeeds=True):
    command = [str(cli), "query", mode, sql]
    if database is not None:
        command.extend(["--database", str(database)])
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if not succeeds:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout)
        return []

    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t"))[1:]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()

    # Explicit VB subtype codes are independent of the implementation constants.
    controls = [
        ("Null", "NULL", 1), ("True", "TRUE", 11),
        ("CByte(255)", "255", 17), ("CInt(-1)", "-1", 2),
        ("CLng(2147483647)", "2147483647", 3),
        ("CSng(1.5)", "1.5", 4), ("CDbl(1.5)", "1.5", 5),
        ("CCur(1.25)", "1.2500", 6), ("'001'", '"001"', 8),
    ]
    rng = random.Random(20261021)
    integers = [-(1 << 63), (1 << 63) - 1, (1 << 53) + 1]
    integers += [rng.randrange(-(1 << 63), 1 << 63) for _ in range(64)]
    controls += [(f"CLngLng({number})", str(number), 20) for number in integers]
    count = 0
    for mode in ["eval", "run"]:
        for expression, expected, subtype in controls:
            sql = f"SELECT CVar({expression}) AS V,VarType(CVar({expression})) AS T"
            rows = execute(args.cli, mode, sql)
            assert rows == [[expected, str(subtype)]], (expression, rows)
            count += 2
        for expression in [
            "CVar()", "CVar(1,1/0)", "CVar(*)", "CVar(,)",
            "CVar(1/0)", "CVar(Array(1,2))", "VBA.CVar(1)",
            "IIf(False,CVar(1/0),0)", "CVar(1),1/0",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1

    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).digest()
        rows = execute(
            args.cli, "run", "SELECT CVar(ID),VarType(CVar(ID)) FROM AData ORDER BY ID",
            database=database,
        )
        assert rows == [[str(n), "3"] for n in [1, 2, 4]], rows
        assert hashlib.sha256(database.read_bytes()).digest() == before
        count += len(rows) * 2

    print(f"Passed {count} independent CVar subtype/value/refusal/native-row controls; native Access unrun.")


if __name__ == "__main__":
    main()
