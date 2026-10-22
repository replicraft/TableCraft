"""Check byte-string functions against Python UTF-16LE bytes, not UTF-8 or native Access.

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

        # Raw slices deliberately include odd byte counts and surrogate halves.
        # Observe bytes instead of forcing invalid UTF-16 through Text output.
        for text in ["", "A", "Ā", "中", "🦀", "A中🦀"]:
            encoded = text.encode("utf-16le")
            for length in [0, 1, 2, 3, 100]:
                for start in [1, 2, 3, 100]:
                    cases = [
                        (f"LeftB('{text}',{length})", encoded[:length]),
                        (f"RightB('{text}',{length})", encoded[-length:] if length else b""),
                        (f"MidB('{text}',{start},{length})", encoded[start - 1:start - 1 + length]),
                        (f"MidB('{text}',{start})", encoded[start - 1:]),
                    ]
                    expressions = []
                    expected = []
                    for call, raw in cases:
                        expressions.append(f"LenB(CVar({call}))")
                        expected.append(str(len(raw)))
                        if raw:
                            expressions.append(f"AscB({call})")
                            expected.append(str(raw[0]))
                    assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == [expected], cases
                    count += len(expected)

        for start in range(0, 256, 16):
            expressions = []
            expected = []
            for code in range(start, start + 16):
                expressions += [f"LenB(ChrB({code}))", f"AscB(ChrB({code}))"]
                expected += ["1", str(code)]
            assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == [expected]
            count += len(expected)

        for expression in ["ChrB(65)", "LeftB('A',1)", "RightB('🦀',2)", "ChrB(256)", "MidB('A',0)", "LeftB(Null,1/0)"]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1

        haystack = "Ā中🦀A"
        raw = haystack.encode("utf-16le")
        for first in range(0, 256, 16):
            expressions = [f"InStrB(1,'{haystack}',ChrB({code}),0)" for code in range(first, first + 16)]
            expected = [str(raw.find(bytes([code])) + 1) for code in range(first, first + 16)]
            assert execute(args.cli, mode, "SELECT " + ",".join(expressions)) == [expected]
            count += len(expected)

        for haystack in ["", "A", "Ā中🦀A", "中中", "🦀🦀"]:
            raw = haystack.encode("utf-16le")
            for needle in ["", "A", "中", "🦀", "Ā"]:
                pattern = needle.encode("utf-16le")
                for start in [1, 2, 3, 100]:
                    expected = 0 if not raw or start > len(raw) else raw.find(pattern, start - 1) + 1
                    sql = f"SELECT InStrB({start},'{haystack}','{needle}',0)"
                    assert execute(args.cli, mode, sql) == [[str(expected)]]
                    count += 1

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
        slices = []
        searches = []
        for line in decoded.splitlines():
            _, identity, value = line.split("\t")
            identity = identity.removeprefix("I32:")
            text = None if value == "N" else bytes.fromhex(value.removeprefix("T")).decode("utf-8")
            lengths.append([identity, "NULL" if text is None else str(len(text.encode("utf-16le")))])
            searches.append([identity, "NULL" if text is None else str(text.encode("utf-16le").find(bytes([0])) + 1)])
            if text:
                encoded = text.encode("utf-16le")
                expected.append([identity, str(len(encoded)), str(encoded[0])])
                slices.append([identity, str(len(encoded[:3])), str(encoded[-1])])

        assert execute(args.cli, "run", "SELECT ID,LenB(Link) FROM Hyperlinks ORDER BY ID", database) == lengths
        assert execute(args.cli, "run", "SELECT ID,LenB(Link),AscB(Link) FROM Hyperlinks WHERE Link Is Not Null AND LenB(Link)>0 ORDER BY ID", database) == expected
        assert execute(args.cli, "run", "SELECT ID,LenB(LeftB(Link,3)),AscB(RightB(Link,1)) FROM Hyperlinks WHERE Link Is Not Null AND LenB(Link)>0 ORDER BY ID", database) == slices
        assert execute(args.cli, "run", "SELECT ID,InStrB(1,Link,ChrB(0),0) FROM Hyperlinks ORDER BY ID", database) == searches
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before
        count += len(lengths) + 2 * len(expected) + 2 * len(slices) + len(searches)

    print(f"Passed {count} independent UTF-16-byte/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
