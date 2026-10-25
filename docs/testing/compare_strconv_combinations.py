"""Independent Unicode composition replay; no native Access or NLS execution.

Unicode properties/names and selected Python casing model TableCraft's explicit
composition order. Native getters supply stored Text, never SQL answers.
"""

import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess
import unicodedata

from compare_browsing import display
from compare_declared_numeric_text import INPUTS, JAR_SHA256
from compare_strconv import proper
from compare_strconv_kana import mapped as kana_mapped
from compare_strconv_width import literal, mapped as width_mapped, pairs

ROOT = Path(__file__).resolve().parents[2]
BATCH_COLUMNS = 3
MAX_COMMANDS = 6000
CHECKPOINT_COMMANDS = 128
CODES = tuple(
    width + kana + case
    for width in (0, 4, 8)
    for kana in (0, 16, 32)
    if width or kana
    for case in range(4)
)
WIDE, NARROW = pairs()
TOOLS = (
    "docs/testing/compare_strconv_combinations.py",
    "docs/testing/compare_strconv_width.py",
    "docs/testing/compare_strconv_kana.py",
    "docs/testing/compare_strconv.py",
    "docs/testing/compare_browsing.py",
    "docs/testing/compare_declared_numeric_text.py",
    "docs/testing/TableDataOracle.java",
)
RUST_ROOTS = (
    "crates/query/tablecraft-expressions/src/functions/text/strconv",
    "crates/query/tablecraft-expressions/src/functions/text/casing",
    "crates/query/tablecraft-expressions/src/evaluation/strconv",
)
RUST_FILES = (
    "crates/query/tablecraft-expressions/src/evaluation/strconv.rs",
    "crates/query/tablecraft-expressions/src/functions/text/casing.rs",
    "Cargo.lock",
)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def json_digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=True).encode()).hexdigest()


def model(text, code):
    if text is None:
        return None
    case = code & 3
    if case == 1:
        text = text.upper()
    elif case == 2:
        text = text.lower()
    elif case == 3:
        text = proper(text, ("\u3000",))
    if code & 4:
        text = width_mapped(text, 4, WIDE, NARROW)
    if code & 16:
        text = kana_mapped(text, 16)
    elif code & 32:
        text = kana_mapped(text, 32)
    if code & 8:
        text = width_mapped(text, 8, WIDE, NARROW)
    return text


def cell(text):
    return "NULL" if text is None else display("T" + text.encode("utf-8").hex())


def cases():
    samples = [
        "", "aB ｶﾞ が", "hello WORLD", "ﬁ ß", "éCOLE 中😀",
        "ａＢ　ガ が", "ｶﾞﾊﾟｳﾞ", "ﾜﾞｦﾞ", "ガカ\u3099",
        "ABC123｡｢｣､･ｰ", "①㍑ﬁe\u0301😀", "'\0\t\nAb",
        "hello　WORLD", "ｈＥＬＬＯ　ｗＯＲＬＤ", "　hello　　WORLD　",
        "東京abc　大阪DEF", "ß　ﬁ", "a\u00a0B　C", "a\u2003B　C",
    ]
    samples += [chr(code) for code in range(0xFF61, 0xFFA0)]
    samples += [chr(code) for code in range(0x3041, 0x3097)]
    samples += [chr(code) for code in range(0x30A1, 0x30FB)]
    for index, text in enumerate(samples):
        for code in CODES:
            label = f"sample={index},flags={code}"
            expression = f"StrConv({literal(text)},{code},1041)"
            try:
                expected = model(text, code)
            except ValueError:
                yield label, expression, None, True
            else:
                yield label, expression, cell(expected), False
    for code in CODES:
        yield f"null:{code}", f"StrConv(NULL,{code},1041)", "NULL", False
        raw = "MidB(ChrW(55296)&'aｶﾞ'&ChrW(56320),3,LenB('aｶﾞ'))"
        yield f"active-raw:{code}", f"StrConv(CVar({raw}),{code},1041)", cell(model("aｶﾞ", code)), False
    for expression in (
        "StrConv(NULL,37,1033)", "StrConv('x',37)",
        "StrConv('x',37,66577)", "StrConv('x',37,NULL)",
        "StrConv('x',100,1041)", "StrConv('x',164,1041)",
        "StrConv('x',44,1041)", "StrConv('x',48,1041)",
        "StrConv(ChrW(55296),37,1041)", "StrConv(ChrB(65),37,1041)",
        "StrConv(ChrW(55296),37,1/0)", "StrConv(IIf(True,'x',1/0),37,1041)",
    ):
        yield "explicit-refusal:" + expression, expression, None, True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Report must be fresh"
    assert digest(args.jar) == JAR_SHA256
    subprocess.run(
        ["git", "diff", "--exit-code", args.revision, "--", *RUST_ROOTS, *RUST_FILES],
        cwd=ROOT,
        check=True,
        stdout=subprocess.DEVNULL,
    )
    rust = set(Path(path) for path in RUST_FILES if Path(path).exists())
    for name in RUST_ROOTS:
        rust.update(Path(name).rglob("*.rs"))
    pins = [args.cli, args.jar, *map(Path, TOOLS), *sorted(rust)]
    record = {
        "scope": __doc__,
        "unicode_version": unicodedata.unidata_version,
        "compiled_revision": args.revision,
        "pins": [{"path": str(path), "sha256": digest(path)} for path in pins],
        "inputs": [],
        "commands": [],
    }
    all_cases = list(cases())
    successes = [case for case in all_cases if not case[3]]
    refusals = [case for case in all_cases if case[3]]
    count = 2 * ((len(successes) + BATCH_COLUMNS - 1) // BATCH_COLUMNS + len(refusals)) + 2 * len(INPUTS)
    assert count <= MAX_COMMANDS, (count, MAX_COMMANDS)

    def execute(mode, sql, expected, labels, options=()):
        command = [str(args.cli), "--format", "tsv", "query", mode, sql, *map(str, options)]
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=60)
        rows = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
        passed = (result.returncode == 2 and not result.stdout) if expected is None else (
            result.returncode == 0 and not result.stderr and rows[1:] == expected
        )
        record["commands"].append({
            "mode": mode,
            "labels": labels,
            "sql_sha256": hashlib.sha256(sql.encode()).hexdigest(),
            "expected_sha256": json_digest(expected),
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "expected_cells": 0 if expected is None else sum(map(len, expected)),
            "exit_code": result.returncode,
            "stderr": result.stderr,
            "passed": passed,
        })
        if not passed or len(record["commands"]) % CHECKPOINT_COMMANDS == 0:
            args.report.write_text(json.dumps(record, indent=2) + "\n")
        assert passed, (labels, result.returncode, result.stderr, rows[:3], expected)

    goldens = (
        ("hello WORLD", 7, "Ｈｅｌｌｏ　Ｗｏｒｌｄ"),
        ("ｶﾞ", 36, "が"), ("ｶﾞ", 40, "ｶﾞ"), ("ガ", 40, "が"),
        ("aが", 24, "aｶﾞ"), ("ﬁ ß", 5, "ＦＩ　ＳＳ"),
        ("hello　WORLD", 3, "Hello　World"),
        ("ｈＥＬＬＯ　ｗＯＲＬＤ", 11, "Hello World"),
    )
    for text, code, expected in goldens:
        assert model(text, code) == expected
    record["model_goldens"] = len(goldens)
    record["scalar_success_cases"] = len(successes)
    record["scalar_refusal_cases"] = len(refusals)
    for mode in ("eval", "run"):
        for start in range(0, len(successes), BATCH_COLUMNS):
            batch = successes[start:start + BATCH_COLUMNS]
            sql = "SELECT " + ",".join(item[1] for item in batch)
            execute(mode, sql, [[item[2] for item in batch]], [item[0] for item in batch])
        for label, expression, _, _ in refusals:
            execute(mode, "SELECT 1," + expression, None, [label])
        print(mode, "scalar controls passed", flush=True)

    oracle = Path("docs/testing/TableDataOracle.java")
    for filename, registered in INPUTS.items():
        database = args.directory / filename
        assert digest(database) == registered, "Unregistered native input"
        raw = subprocess.check_output(
            ["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar), str(oracle), str(database), "ScalarSamples"],
            text=True,
            encoding="utf-8",
            timeout=60,
        )
        fields = []
        for line in raw.splitlines():
            cells = line.split("\t")
            tag = cells[8]
            assert tag == "N" or tag.startswith("T")
            text = None if tag == "N" else bytes.fromhex(tag[1:]).decode("utf-8")
            fields.append((cells[1].removeprefix("I32:"), text))
        assert [identity for identity, _ in fields] == ["1", "2", "3"]
        for label, source in (
            ("stored", "Label"),
            ("japanese-word-boundary", "IIf(IsNull(Label),NULL,Label & '　WORLD')"),
        ):
            sql = "SELECT ID," + ",".join(f"StrConv({source},{code},1041)" for code in CODES)
            sql += " FROM ScalarSamples ORDER BY ID"
            expected = []
            for identity, text in fields:
                if text is not None and label == "japanese-word-boundary":
                    text += "　WORLD"
                expected.append([identity, *[cell(model(text, code)) for code in CODES]])
            execute("run", sql, expected, [filename, label], ("--database", database))
        assert digest(database) == registered
        record["inputs"].append({
            "path": str(database), "sha256": registered,
            "getter_sha256": hashlib.sha256(raw.encode()).hexdigest(), "unchanged": True,
        })
        print(filename, "native fields passed", flush=True)

    for pin in record["pins"]:
        assert digest(pin["path"]) == pin["sha256"]
    assert len(record["commands"]) == count
    record["command_count"] = count
    record["all_pins_unchanged"] = True
    record["compared_cells"] = sum(command["expected_cells"] for command in record["commands"])
    record["expected_failures"] = sum(command["exit_code"] == 2 for command in record["commands"])
    args.report.write_text(json.dumps(record, indent=2) + "\n")
    print(count, "commands passed; native Access/NLS unrun")


if __name__ == "__main__":
    main()
