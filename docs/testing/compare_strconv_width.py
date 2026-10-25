"""Compare explicit Japanese StrConv width operations against Python Unicode data.

Width pairs come from Unicode <wide>/<narrow> decomposition properties. Only
qualified kana pairs compose/decompose; this is not whole-string NFKC, Windows
NLS, or native Access execution. No production semantic mappings are imported.
"""

import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import random
import subprocess
import unicodedata

from compare_browsing import display


def literal(text):
    parts = ["'" + part.replace("'", "''") + "'" for part in text.split("\0")]
    return "(" + " & ChrW(0) & ".join(parts) + ")"


def pairs():
    wide = {" ": "　"}
    narrow = {"　": " "}
    for code in range(0xFF01, 0xFFA0):
        character = chr(code)
        decomposition = unicodedata.decomposition(character).split()
        if not decomposition:
            continue
        tag, *units = decomposition
        assert len(units) == 1
        counterpart = chr(int(units[0], 16))
        if tag == "<wide>":
            wide[counterpart] = character
            narrow[character] = counterpart
        elif tag == "<narrow>":
            wide[character] = counterpart
            narrow[counterpart] = character
    return wide, narrow


def mapped(text, mode, wide, narrow):
    if mode == 4:
        result = []
        for character in text:
            character = wide.get(character, character)
            mark = {"゛": "\u3099", "゜": "\u309a"}.get(character, character)
            if result and "\u3041" <= result[-1] <= "\u30fa" and mark in "\u3099\u309a":
                composed = unicodedata.normalize("NFC", result[-1] + mark)
                if len(composed) == 1:
                    result[-1] = composed
                    continue
            result.append(character)
        return "".join(result)

    result = []
    for character in text:
        if character in narrow:
            result.append(narrow[character])
        elif "ァ" <= character <= "ヺ":
            decomposition = unicodedata.normalize("NFD", character)
            if len(decomposition) != 2 or any(part not in narrow for part in decomposition):
                raise ValueError("Unqualified narrow source: " + character)
            result.extend(narrow[part] for part in decomposition)
        else:
            result.append(character)
    return "".join(result)


def execute(cli, mode, sql, succeeds=True):
    result = subprocess.run(
        [str(cli), "--format", "tsv", "query", mode, sql],
        capture_output=True,
        text=True,
        check=False,
    )
    if not succeeds:
        assert result.returncode != 0 and not result.stdout, (sql, result.stdout, result.stderr)
        return None
    assert result.returncode == 0, (sql, result.stderr)
    return list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    binary_before = digest(args.cli)
    source_paths = [
        Path("Cargo.lock"),
        Path("Cargo.toml"),
        Path("crates/query/tablecraft-expressions/Cargo.toml"),
        Path("crates/query/tablecraft-expressions/src/functions/text/strconv/mod.rs"),
        Path("crates/query/tablecraft-expressions/src/evaluation/strconv.rs"),
        Path("crates/query/tablecraft-expressions/src/functions/text/strconv/flags.rs"),
        Path("crates/query/tablecraft-expressions/src/functions/text/strconv/width.rs"),
        Path("docs/testing/compare_browsing.py"),
    ]
    source_before = {str(path): digest(path) for path in source_paths}
    wide, narrow = pairs()
    halfwidth = list(wide)
    fullwidth = list(narrow)
    voiced = []
    for code in range(0x30A1, 0x30FB):
        character = chr(code)
        try:
            mapped(character, 8, wide, narrow)
        except ValueError:
            continue
        voiced.append(character)
    mixed = [
        "", "ABC 123 !~", "ガパヴヷヺ", "ｶﾞﾊﾟｳﾞﾜﾞｦﾞ", "は゛がハ゜",
        "ガパ", "カ゛ハ゜", "A゛゜", "ひらがな①㍑ﬁé😀", "'\0\t\n", "ﾞﾟ゙゚゛゜",
    ]
    rng = random.Random(20261024)
    alphabet = halfwidth + fullwidth + voiced + ["ひ", "😀", "①", "\0"]
    mixed.extend(
        "".join(rng.choice(alphabet) for _ in range(rng.randrange(24)))
        for _ in range(48)
    )
    samples = halfwidth + fullwidth + voiced + mixed
    successes = 0
    refusals = 0
    commands = 0
    for command in ["eval", "run"]:
        for mode in [4, 8]:
            controls = []
            for text in samples:
                expected = display("T" + mapped(text, mode, wide, narrow).encode("utf-8").hex())
                source = literal(text)
                for expression in [source, f"CVar(Mid({source},1))", f"LeftB({source},LenB({source}))"]:
                    controls.append((f"StrConv({expression},{mode},1041)", expected))
            for offset in range(0, len(controls), 3):
                batch = controls[offset:offset + 3]
                sql = "SELECT " + ",".join(expression for expression, _ in batch)
                actual = execute(args.cli, command, sql)
                expected = [[value for _, value in batch]]
                assert actual == expected, (command, mode, sql, actual, expected)
                successes += len(batch)
                commands += 1
        for expression in [
            "StrConv('A',4)", "StrConv(NULL,8,1033)", "StrConv('A',4,66577)",
            "StrConv('A',12,1041)", "StrConv('A',5,1041)", "StrConv('A',20,1041)",
            "StrConv('A',4,NULL)", "StrConv(ChrW(55296),4,1041)",
            "StrConv(ChrB(65),8,1041)", "StrConv(ChrW(55296),4,1/0)",
            "StrConv(IIf(True,'A',1/0),4,1041)", "StrConv('ヵ',8,1041)",
            "StrConv('ヸ',8,1041)", "StrConv('ﾠ',4,1041)", "StrConv('￥',8,1041)",
        ]:
            execute(args.cli, command, "SELECT 1," + expression, succeeds=False)
            refusals += 1
            commands += 1
    assert digest(args.cli) == binary_before
    source_after = {str(path): digest(path) for path in source_paths}
    assert source_after == source_before
    report = {
        "scope": "Explicit Japanese standalone StrConv4/8, LCID1041; no Windows or Access execution",
        "model": "Unicode width decomposition properties with local kana NFC/NFD only",
        "unicode_version": unicodedata.unidata_version,
        "success_controls": successes,
        "refusal_controls": refusals,
        "cli_commands": commands,
        "cli_sha256_before": binary_before,
        "cli_sha256_after": digest(args.cli),
        "source_sha256_before": source_before,
        "source_sha256_after": source_after,
        "runner_sha256": digest(Path(__file__)),
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
