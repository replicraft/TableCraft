"""Check StrConv against independent casing/code-page models and decoded fields.

Python codecs plus the reviewed WHATWG exceptions supply byte mappings. The
Unicode casing controls are selected model cases, not a Windows NLS oracle.
Optional HyperlinkFixture inputs remain unchanged; Jackcess only decodes values.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import random
import subprocess

from compare_browsing import RECIPES, display, run
from compare_character_code_pages import PAGES, decode

JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
SEPARATORS = set("\0\t\n\v\f\r ")


def literal(text):
    return "'" + text.replace("'", "''") + "'"


def spelling(text):
    return display("T" + text.encode("utf-8").hex())


def proper(text):
    # Explicit first-cased-letter model for the selected ASCII/Latin controls.
    output = []
    first = True
    for character in text:
        if character in SEPARATORS:
            first = True
            output.append(character)
        elif first and character.lower() != character.upper():
            first = False
            output.append(character.title())
        else:
            output.append(character.lower())
    return "".join(output)


def execute(cli, mode, sql, options=(), succeeds=True):
    result = subprocess.run([str(cli), "query", mode, sql, *map(str, options)], capture_output=True, text=True, check=False)
    if not succeeds:
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
        parser.error("--database requires --jar for independent getters")

    rng = random.Random(20261021)
    texts = ["", "Hello WORLD", "o'NEILL ab-CD", "straße", "éCOLE", "hELLO\tWORLD"]
    texts += ["".join(rng.choice("aAbBzZ -'") for _ in range(rng.randrange(50))) for _ in range(128)]
    count = 0
    for mode in ["eval", "run"]:
        for text in texts:
            sql = "SELECT " + ",".join(f"StrConv({literal(text)},{conversion},1033)" for conversion in [1, 2, 3])
            expected = [spelling(text.upper()), spelling(text.lower()), spelling(proper(text))]
            assert execute(args.cli, mode, sql) == [expected], text
            count += 3

        for page in PAGES:
            options = ["--character-code-page", page]
            valid = []
            for byte in range(256):
                character = decode(page, byte)
                if character is None:
                    execute(args.cli, mode, f"SELECT 1,StrConv(ChrB({byte}),64)", options, succeeds=False)
                    count += 1
                else:
                    valid.append((byte, character))
            for offset in range(0, len(valid), 16):
                expressions = []
                expected = []
                for byte, character in valid[offset:offset + 16]:
                    expressions += [f"AscW(StrConv(ChrB({byte}),64))", f"AscB(StrConv(StrConv(ChrB({byte}),64),128))"]
                    code = ord(character)
                    expected += [str(code if code < 32768 else code - 65536), str(byte)]
                assert execute(args.cli, mode, "SELECT " + ",".join(expressions), options) == [expected], (mode, page, offset)
                count += len(expected)

        for expression in [
            "StrConv('x',1)",
            "StrConv('x',4,1033)",
            "StrConv('x',1,1042)",
            "StrConv('x',192)",
            "StrConv('x',1,0)",
            "StrConv('€',128,1033)",
            "StrConv('🦀',128,1033)",
            "StrConv(Null,1,1/0)",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            count += 1
        for expression, expected in [
            ("StrConv('Iİiı',2,1055)", "ıiiı"),
            ("StrConv('ijSSEL',3,1043)", "IJssel"),
            ("StrConv(StrConv('é€',128,1033),64,1033)", "é€"),
            ("StrConv(ChrB(192),64,1049)", "А"),
        ]:
            assert execute(args.cli, mode, "SELECT " + expression) == [[spelling(expected)]]
            count += 1

    if args.jar:
        assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == JAR_SHA256
    for database in args.database:
        before = hashlib.sha256(database.read_bytes()).hexdigest()
        tagged = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "TableDataOracle.java", database, "Hyperlinks")
        fields = []
        for line in tagged.splitlines():
            _, identity, value = line.split("\t")
            identity = identity.removeprefix("I32:")
            text = None if value == "N" else bytes.fromhex(value.removeprefix("T")).decode("utf-8")
            fields.append((identity, text))
        options = ["--database", database]
        for conversion, model in [(1, str.upper), (2, str.lower), (3, proper)]:
            expected = [[identity, "NULL" if text is None else spelling(model(text))] for identity, text in fields]
            sql = f"SELECT ID,StrConv(Link,{conversion},1033) FROM Hyperlinks ORDER BY ID"
            assert execute(args.cli, "run", sql, options) == expected
            count += len(expected)
        expected = [[identity, spelling(text), str(len(text.encode("cp1252")))] for identity, text in fields if int(identity) <= 4]
        sql = "SELECT ID,StrConv(StrConv(Link,128,1033),64,1033),LenB(StrConv(Link,128,1033)) FROM Hyperlinks WHERE ID<=4 ORDER BY ID"
        assert execute(args.cli, "run", sql, options) == expected
        count += 2 * len(expected)
        execute(args.cli, "run", "SELECT ID,StrConv(Link,128,1033) FROM Hyperlinks WHERE ID=5", options, succeeds=False)
        count += 1
        assert hashlib.sha256(database.read_bytes()).hexdigest() == before

    print(f"Passed {count} independent StrConv mapping/casing/refusal/native-field controls; native Access unrun.")


if __name__ == "__main__":
    main()
