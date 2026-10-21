"""Check LenB/AscB against Python UTF-16LE bytes, not UTF-8 or native Access.

Optional inputs are original HyperlinkFixture databases; Jackcess independently
decodes their Text fields before TableCraft evaluates byte observations.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, run

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


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
    parser.add_argument("--jar", type=Path)
    parser.add_argument("--database", type=Path, action="append", default=[])
    args = parser.parse_args()
    if args.database and not args.jar:
        parser.error("--database requires --jar for independent getters")

    rng = random.Random(20261021)
    values = ["", "A", "é", "Ā", "中", "🦀", "é中🦀", "e\u0301"]
    values += ["".join(rng.choice("AéĀ中🦀e\u0301") for _ in range(rng.randrange(25))) for _ in range(128)]
    count = 0
    for mode in ["eval", "run"]:
        for text in values:
            encoded = text.encode("utf-16le")
            if encoded:
                sql = f"SELECT LenB('{text}'),AscB('{text}')"
                expected = [str(len(encoded)), str(encoded[0])]
            else:
                sql = "SELECT LenB('')"
                expected = ["0"]
            assert execute(args.cli, mode, sql) == [expected], text
            count += len(expected)

        assert execute(args.cli, mode, "SELECT LenB(Null)") == [["NULL"]]
        count += 1
        for expression in ["AscB('')", "AscB(Null)", "LenB(1)", "LenB(*)", "AscB(,)", "LenB('x',1/0)", "LenB('x'),1/0"]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        decoded = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "Hyperlinks")
        expected = []
        lengths = []
        for line in decoded.splitlines():
            _, identity, value = line.split("\t")
            identity = identity.removeprefix("I32:")
            text = None if value == "N" else bytes.fromhex(value.removeprefix("T")).decode("utf-8")
            lengths.append([identity, "NULL" if text is None else str(len(text.encode("utf-16le")))])
            if text:
                encoded = text.encode("utf-16le")
                expected.append([identity, str(len(encoded)), str(encoded[0])])

        assert execute(args.cli, "run", "SELECT ID,LenB(Link) FROM Hyperlinks ORDER BY ID", database) == lengths
        assert execute(args.cli, "run", "SELECT ID,LenB(Link),AscB(Link) FROM Hyperlinks WHERE Link Is Not Null AND LenB(Link)>0 ORDER BY ID", database) == expected
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
        count += len(lengths) + 2 * len(expected)

    print(f"Passed {count} independent UTF-16-byte/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
