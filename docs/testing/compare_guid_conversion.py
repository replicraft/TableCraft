"""Compare GUIDFromString bytes with Python's independent UUID bytes_le model.

Optional files are original SavedForest-derived AData fixtures, not native
Microsoft Access execution evidence.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess
import uuid


def execute(cli, mode, sql, database=None, succeeds=True):
    command = [str(cli), "query", mode, sql, "--number-format", "invariant"]
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

    rng = random.Random(20261021)
    values = [uuid.UUID(int=0), uuid.UUID(int=(1 << 128) - 1)]
    values += [uuid.UUID(int=rng.getrandbits(128)) for _ in range(64)]
    count = 0
    for mode in ["eval", "run"]:
        for value in values:
            for text in [str(value), "{" + str(value).upper() + "}", "{guid {" + str(value) + "}}"]:
                sql = f"SELECT GUIDFromString('{text}') AS G"
                assert execute(args.cli, mode, sql) == [["0x" + value.bytes_le.hex()]], text
                count += 1

        for expression in [
            "GUIDFromString(Null)", "GUIDFromString(1)", "GUIDFromString('invalid')",
            "GUIDFromString(*)", "GUIDFromString(,)", "GUIDFromString('invalid',1/0)",
            "GUIDFromString('00000000-0000-0000-0000-000000000000'),1/0",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1

    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).digest()
        rows = execute(
            args.cli, "run",
            "SELECT ID,GUIDFromString('00000000-0000-0000-0000-' & Right('000000000000' & CStr(ID),12)) FROM AData ORDER BY ID",
            database=database,
        )
        expected = [[str(n), "0x" + uuid.UUID("00000000-0000-0000-0000-" + str(n).zfill(12)).bytes_le.hex()] for n in [1, 2, 4]]
        assert rows == expected, rows
        assert hashlib.sha256(database.read_bytes()).digest() == before
        count += len(rows)

    print(f"Passed {count} independent GUID-byte/refusal/native-row controls; native Access unrun.")


if __name__ == "__main__":
    main()
