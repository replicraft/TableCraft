"""Check explicit StrConv36 against existing independent Unicode models.

The oracle composes the reviewed width-property and Unicode-name models. This
checks TableCraft's stated width-then-Hiragana engineering profile, not native
Access or Windows NLS behavior. No new reference tables are distributed.
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
from compare_strconv_kana import mapped as kana_mapped
from compare_strconv_width import literal, mapped as width_mapped, pairs

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 300
BATCH_COLUMNS = 3
SOURCE_ROOTS = [
    "crates/query/tablecraft-expressions/src/functions/text/strconv",
    "crates/query/tablecraft-expressions/src/evaluation/strconv.rs",
    "crates/query/tablecraft-expressions/src/evaluation/strconv",
]
TOOL_PATHS = [
    "docs/testing/compare_strconv_wide_hiragana.py",
    "docs/testing/compare_strconv_width.py",
    "docs/testing/compare_strconv_kana.py",
    "docs/testing/compare_browsing.py",
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def model(text):
    wide, narrow = pairs()
    return kana_mapped(width_mapped(text, 4, wide, narrow), 32)


def golden_checks():
    goldens = [
        ("ｶﾞﾊﾟｳﾞ", "がぱゔ"),
        ("ｱｲｳｴｵ", "あいうえお"),
        ("ABC 123", "ＡＢＣ　１２３"),
        ("は゛ カ゛ ハ゜", "ば　が　ぱ"),
        ("ﾜﾞｦﾞ", "わ\u3099を\u3099"),
        ("ガカ\u3099ヵヶヽヾ", "ががゕゖゝゞ"),
        ("A゛゜", "Ａ゛゜"),
        ("①㍑ﬁ😀", "①㍑ﬁ😀"),
        ("｡｢｣､･ｰ", "。「」、・ー"),
        ("", ""),
    ]
    for source, expected in goldens:
        if model(source) != expected:
            raise AssertionError((source, model(source), expected))
    return len(goldens)


def controls():
    wide, _ = pairs()
    samples = [("width_single", text) for text in wide]
    for codepoint in range(0xFF61, 0xFF9E):
        for mark in ["ﾞ", "ﾟ"]:
            samples.append(("halfwidth_mark_pair", chr(codepoint) + mark))
    mixed = [
        "カ\u3099ハ\u309a",
        "カ゛ハ゜",
        "は゛か\u3099ハ゜",
        "ガカ\u3099ヵヶヽヾ",
        "ﾜﾞｦﾞ",
        "A゛゜",
        "ﾞﾟ\u3099\u309a゛゜",
        "ひらがな①㍑ﬁe\u0301😀",
        "ABC 123 !~",
        "'\0\t\n",
        "",
    ]
    samples.extend(("mixed", text) for text in mixed)
    for category, text in samples:
        yield category, f"StrConv({literal(text)},36,1041)", model(text)

    for text in ["ｶﾞ", "ﾊﾟｳﾞ", "ﾜﾞｦﾞ", "ABC", "カ゛", "①😀"]:
        source = literal(text)
        selected = f"MidB(ChrW(55296)&{source}&ChrW(56320),3,LenB({source}))"
        yield "active_raw_range", f"StrConv(CVar({selected}),36,1041)", model(text)
        raw = f"LeftB({source},LenB({source}))"
        yield "raw_whole", f"StrConv({raw},36,1041)", model(text)

    yield "null", "StrConv(NULL,36,1041)", None
    yield "reverse_order_discriminator", "StrConv(StrConv('ｶﾞ',32,1041),4,1041)", "ガ"
    yield "forward_order_discriminator", "StrConv('ｶﾞ',36,1041)", "が"


def refusal_controls():
    return [
        ("missing_locale", "StrConv('ｶﾞ',36)"),
        ("wrong_locale", "StrConv('ｶﾞ',36,1033)"),
        ("null_wrong_locale", "StrConv(NULL,36,1033)"),
        ("alternate_locale", "StrConv('ｶﾞ',36,66577)"),
        ("null_locale", "StrConv('ｶﾞ',36,NULL)"),
        ("unsupported_case_combination", "StrConv('ｶﾞ',37,1041)"),
        ("unsupported_encoding_combination", "StrConv('ｶﾞ',100,1041)"),
        ("conflicting_width", "StrConv('ｶﾞ',44,1041)"),
        ("odd_raw", "StrConv(ChrB(65),36,1041)"),
        ("invalid_utf16", "StrConv(ChrW(55296),36,1041)"),
        ("eager_source", "StrConv(IIf(True,'ｶﾞ',1/0),36,1041)"),
        ("eager_locale", "StrConv(ChrW(55296),36,1/0)"),
    ]


def source_pins(revision):
    listing = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS],
        cwd=ROOT,
        capture_output=True,
        text=True,
        check=True,
        timeout=120,
    )
    pins = {}
    for name in listing.stdout.splitlines():
        content = subprocess.run(
            ["git", "show", f"{revision}:{name}"],
            cwd=ROOT,
            capture_output=True,
            check=True,
            timeout=120,
        ).stdout
        pins[name] = hashlib.sha256(content).hexdigest()
    if not pins:
        raise ValueError("Compiled revision has no scoped StrConv sources")
    return pins


def execute(cli, mode, sql, expected, labels):
    command = [str(cli), "--format", "tsv", "query", mode, sql]
    result = subprocess.run(command, cwd=ROOT, capture_output=True, text=True, timeout=120)
    actual = None
    if result.returncode == 0:
        actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))[1:]
    if expected is None:
        matched = result.returncode != 0 and result.stdout == ""
    else:
        matched = result.returncode == 0 and actual == [expected]
    return {
        "command": [relative(cli), *command[1:]],
        "controls": labels,
        "expected_rows": None if expected is None else [expected],
        "returncode": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "actual_rows": actual,
        "matched": matched,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    args.cli = args.cli.resolve()
    args.report = args.report.resolve()
    relative(args.cli)
    relative(args.report)
    if args.report.exists():
        raise FileExistsError("Refusing to overwrite an existing report")
    if digest(args.cli) != args.cli_sha256:
        raise ValueError("Frozen executable hash mismatch")

    goldens = golden_checks()
    before = {name: digest(ROOT / name) for name in TOOL_PATHS}
    before[relative(args.cli)] = digest(args.cli)
    compiled = source_pins(args.revision)
    cases = list(controls())
    refusals = refusal_controls()
    command_count = 2 * ((len(cases) + BATCH_COLUMNS - 1) // BATCH_COLUMNS + len(refusals))
    if command_count > MAX_COMMANDS:
        raise ValueError("Replay exceeds its explicit command ceiling")

    records = []
    for mode in ["eval", "run"]:
        for offset in range(0, len(cases), BATCH_COLUMNS):
            batch = cases[offset:offset + BATCH_COLUMNS]
            sql = "SELECT " + ",".join(expression for _, expression, _ in batch)
            expected = [
                "NULL" if value is None else display("T" + value.encode("utf-8").hex())
                for _, _, value in batch
            ]
            labels = [category for category, _, _ in batch]
            records.append(execute(args.cli, mode, sql, expected, labels))
        for label, expression in refusals:
            records.append(execute(args.cli, mode, "SELECT 1," + expression, None, [label]))

    after = {name: digest(ROOT / name) for name in before}
    report = {
        "scope": "Explicit Japanese StrConv36 width-then-Hiragana engineering composition",
        "native_qualification": "No native Access or Windows NLS execution",
        "model": "Reviewed Unicode width-property then Unicode-name kana model; no production imports",
        "unicode_version": unicodedata.unidata_version,
        "model_goldens": goldens,
        "compiled_revision": args.revision,
        "compiled_source_sha256": compiled,
        "hashes_before": before,
        "hashes_after": after,
        "cli_commands": len(records),
        "success_commands": sum(record["expected_rows"] is not None for record in records),
        "refusal_commands": 2 * len(refusals),
        "success_cells": 2 * len(cases),
        "disagreements": sum(not record["matched"] for record in records),
        "pins_unchanged": before == after,
        "commands": records,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as destination:
        json.dump(report, destination, indent=2, ensure_ascii=False)
        destination.write("\n")
    print(json.dumps({key: value for key, value in report.items() if key not in [
        "commands", "compiled_source_sha256", "hashes_before", "hashes_after"
    ]}, indent=2))
    if report["disagreements"] or not report["pins_unchanged"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
