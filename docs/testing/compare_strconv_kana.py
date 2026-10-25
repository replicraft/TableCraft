"""Compare the explicit StrConv kana profile with independent Unicode-name mappings.

Python's Unicode database supplies corresponding script character names and NFD
voiced-letter decomposition. No production mappings or native Windows APIs are
imported. This checks TableCraft's stated engineering profile, not Access/NLS
compatibility. Original binary hashes and the Python Unicode version are recorded.
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
    # argv cannot carry NUL; compose it in SQL without changing the source value.
    parts = ["'" + part.replace("'", "''") + "'" for part in text.split("\0")]
    return "(" + " & ChrW(0) & ".join(parts) + ")"


def mapped(text, mode):
    source, target = ("HIRAGANA", "KATAKANA") if mode == 16 else ("KATAKANA", "HIRAGANA")
    result = []
    for character in text:
        name = unicodedata.name(character, "")
        if not name.startswith(source + " "):
            result.append(character)
            continue
        if " LETTER " not in name and "ITERATION MARK" not in name and "DIGRAPH" not in name:
            result.append(character)
            continue
        counterpart = target + name[len(source):]
        try:
            result.append(unicodedata.lookup(counterpart))
        except KeyError:
            decomposition = unicodedata.normalize("NFD", character)
            if decomposition == character:
                raise ValueError(f"No qualified counterpart for {name}") from None
            result.append(mapped(decomposition, mode))
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    binary_before = hashlib.sha256(args.cli.read_bytes()).hexdigest()
    source_paths = [
        Path("crates/query/tablecraft-expressions/src/evaluation/strconv.rs"),
        Path("crates/query/tablecraft-expressions/src/functions/text/strconv/flags.rs"),
        Path("crates/query/tablecraft-expressions/src/functions/text/strconv/kana.rs"),
        Path("docs/testing/compare_browsing.py"),
    ]
    source_before = {
        str(path): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in source_paths
    }
    characters = [
        chr(code)
        for code in range(0x3040, 0x3100)
        if unicodedata.name(chr(code), "").startswith(("HIRAGANA LETTER ", "KATAKANA LETTER "))
        or "ITERATION MARK" in unicodedata.name(chr(code), "")
    ]
    samples = [
        "", "ひらがな カタカナ", "ゔゕゖゝゞ ヴヵヶヽヾ", "ヷヸヹヺ",
        "か\u3099 は\u309a", "カ\u3099 ハ\u309a", "ﾊﾟ ｳﾞ ｶﾞ", "ー・゛゜ Aß😀'\0",
    ]
    rng = random.Random(20261024)
    samples.extend(
        "".join(rng.choice(characters + ["A", "😀", "\u3099", "ﾊ"]) for _ in range(rng.randrange(40)))
        for _ in range(64)
    )
    samples.extend(characters)
    successes = 0
    refusals = 0
    commands = 0
    for mode in ["eval", "run"]:
        for conversion in [16, 32]:
            controls = []
            for text in samples:
                expected = display("T" + mapped(text, conversion).encode("utf-8").hex())
                sources = [
                    literal(text),
                    f"CVar(Mid({literal(text)},1))",
                    f"LeftB({literal(text)},LenB({literal(text)}))",
                ]
                for source in sources:
                    controls.append((f"StrConv({source},{conversion},1041)", expected))
            # Three cells keep even the longer raw slices within default work limits.
            for offset in range(0, len(controls), 3):
                batch = controls[offset:offset + 3]
                sql = "SELECT " + ",".join(expression for expression, _ in batch)
                expected = [[expected for _, expected in batch]]
                assert execute(args.cli, mode, sql) == expected, (mode, conversion, offset)
                successes += len(batch)
                commands += 1
        for expression in [
            "StrConv('あ',16)", "StrConv(NULL,32,1033)", "StrConv('あ',16,66577)",
            "StrConv('あ',48,1041)", "StrConv('あ',17,1041)", "StrConv('あ',80,1041)",
            "StrConv('あ',4,1041)", "StrConv('あ',16,NULL)",
            "StrConv(ChrW(55296),16,1041)", "StrConv(ChrB(65),32,1041)",
            "StrConv(ChrW(55296),16,1/0)", "StrConv(IIf(True,'あ',1/0),16,1041)",
            "StrConv('ゟ',16,1041)", "StrConv('ヿ',32,1041)",
            "StrConv('ㇰ',32,1041)", "StrConv('𛀁',16,1041)",
        ]:
            execute(args.cli, mode, "SELECT 1," + expression, succeeds=False)
            refusals += 1
            commands += 1
    binary_after = hashlib.sha256(args.cli.read_bytes()).hexdigest()
    assert binary_before == binary_after
    source_after = {
        str(path): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in source_paths
    }
    assert source_before == source_after
    report = {
        "scope": "Explicit standalone StrConv16/32 with exact Japanese LCID1041; no Windows/Access execution",
        "model": "Python Unicode character-name counterpart lookup, NFD expansion only when no precomposed counterpart exists",
        "unicode_version": unicodedata.unidata_version,
        "success_controls": successes,
        "refusal_controls": refusals,
        "cli_commands": commands,
        "cli_sha256_before": binary_before,
        "cli_sha256_after": binary_after,
        "source_hash_scope": "Checkout during replay; private SQL-only cfg gates were added after executable freeze without changing SQL operation bodies",
        "source_sha256_before": source_before,
        "source_sha256_after": source_after,
        "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
