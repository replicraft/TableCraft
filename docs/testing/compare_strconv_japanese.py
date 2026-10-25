"""Replay Japanese StrConv against original primary-index token models.

Private reference tables are inputs, not distributed data. This is WHATWG codec
and strict Unicode round-trip evidence, not Windows NLS or Access execution.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

from strconv_japanese_model import (
    decode,
    decoder_cases,
    encoder_cases,
    goldens,
    read_index,
    read_microsoft,
)

ROOT = Path(__file__).resolve().parents[2]
INDEX_HASH = "341dcde7e8b984e9c7bbf5ed75c8da7c6087d47083a1a2b3ed558bfd5bef9468"
MICROSOFT_HASH = "c9bc0b0cd42e0fbcb82a09635bb5abed86afbdd4abc9e76fa5716638217cb59f"
MAX_COLUMNS = 16
SOURCE_ROOTS = [
    "crates/query/tablecraft-expressions/src/functions/text/strconv",
    "crates/query/tablecraft-expressions/src/evaluation/strconv.rs",
    "crates/query/tablecraft-expressions/src/evaluation/strconv",
    "crates/query/tablecraft-query/src/execution/text.rs",
]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def invoke(command):
    arguments = [relative(value) if isinstance(value, Path) else str(value) for value in command]
    return subprocess.run(arguments, capture_output=True, text=True, timeout=120, cwd=ROOT)


def source_pins(revision):
    listing = invoke(["git", "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS])
    listing.check_returncode()
    pins = {}
    for name in listing.stdout.splitlines():
        source = subprocess.run(
            ["git", "show", f"{revision}:{name}"],
            check=True,
            capture_output=True,
            timeout=120,
            cwd=ROOT,
        )
        pins[name] = hashlib.sha256(source.stdout).hexdigest()
    if not any(name.endswith("strconv/code_page.rs") for name in pins):
        raise RuntimeError("Compiled revision lacks the Japanese string codec")
    return pins


def raw_expression(token):
    if len(token) == 1:
        return f"ChrB({token[0]})"
    if len(token) == 2:
        return f"ChrW({int.from_bytes(token, 'little')})"
    raise ValueError("Expected one complete codec token")


def unicode_expression(codepoint):
    return f"ChrW({codepoint})" if codepoint <= 0xffff else "'" + chr(codepoint) + "'"


def projections(index):
    for token, codepoint in decoder_cases(index):
        if codepoint is None:
            continue
        expression = f"AscW(StrConv({raw_expression(token)},64,1041))"
        signed = codepoint if codepoint < 0x8000 else codepoint - 0x10000
        yield "decode", token.hex(), [expression], [signed]
    for codepoint, token in encoder_cases(index):
        if token is None:
            continue
        encoded = f"StrConv({unicode_expression(codepoint)},128,1041)"
        expressions = [f"LenB({encoded})"]
        values = [len(token)]
        for position, byte in enumerate(token, 1):
            expressions.append(f"AscB(MidB({encoded},{position},1))")
            values.append(byte)
        yield "encode", f"U+{codepoint:04X}", expressions, values


def batches(index):
    expressions = []
    expected = []
    records = []
    for operation, label, columns, values in projections(index):
        if len(expressions) + len(columns) > MAX_COLUMNS:
            yield expressions, expected, records
            expressions, expected, records = [], [], []
        expressions.extend(columns)
        expected.extend(values)
        records.append({"operation": operation, "input": label})
    if expressions:
        yield expressions, expected, records


def refusals(index):
    for token, codepoint in decoder_cases(index):
        if codepoint is None:
            yield "decode", token.hex(), f"StrConv({raw_expression(token)},64,1041)"
    for codepoint, token in encoder_cases(index):
        if token is None:
            source = unicode_expression(codepoint)
            yield "encode", f"U+{codepoint:04X}", f"LenB(StrConv({source},128,1041))"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "index", "microsoft", "report"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    relative(args.report)
    if args.report.exists():
        raise RuntimeError("Refusing to overwrite evidence")
    for path, expected in [
        (args.cli, args.cli_sha256),
        (args.index, INDEX_HASH),
        (args.microsoft, MICROSOFT_HASH),
    ]:
        if digest(path) != expected:
            raise RuntimeError("Immutable input identity mismatch")
    inputs = [
        args.cli,
        args.index,
        args.microsoft,
        Path(__file__),
        ROOT / "docs/testing/strconv_japanese_model.py",
    ]
    before = {relative(path): digest(path) for path in inputs}
    sources = source_pins(args.revision)
    index = read_index(args.index)
    microsoft = read_microsoft(args.microsoft)
    goldens(index)
    mapping_differences = [
        {"bytes": token.hex(), "microsoft": codepoint, "whatwg": decode(token, index)}
        for token, codepoint in microsoft.items()
        if decode(token, index) != codepoint
    ]
    observations = []
    compared_cells = 0
    matched_mapping_cases = 0

    for columns, values, cases in batches(index):
        sql = "SELECT " + ",".join(columns)
        command = [args.cli, "--format", "tsv", "query", "eval", sql]
        result = invoke(command)
        actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))
        headers = [f"Expr{position}" for position in range(1, len(values) + 1)]
        expected = [headers, [str(value) for value in values]]
        matches = result.returncode == 0 and not result.stderr and actual == expected
        if matches:
            compared_cells += len(values)
            matched_mapping_cases += len(cases)
        observations.append({
            "command": [relative(value) if isinstance(value, Path) else value for value in command],
            "cases": cases,
            "exit_code": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "expected": expected,
            "matches": matches,
        })

    for operation, label, expression in refusals(index):
        command = [args.cli, "--format", "tsv", "query", "eval", "SELECT " + expression]
        result = invoke(command)
        observations.append({
            "command": [relative(value) if isinstance(value, Path) else value for value in command],
            "operation": operation,
            "input": label,
            "exit_code": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "expected_refusal": "Nonzero exit and no buffered stdout",
            "matches": result.returncode != 0 and not result.stdout,
        })

    after = {name: digest(ROOT / name) for name in before}
    if before != after or sources != source_pins(args.revision):
        raise RuntimeError("An immutable source/input pin changed")
    decoder = list(decoder_cases(index))
    encoder = list(encoder_cases(index))
    disagreements = sum(not item["matches"] for item in observations)
    report = {
        "scope": __doc__,
        "compiled_revision": args.revision,
        "compiled_source_sha256": sources,
        "hashes_before": before,
        "hashes_after": after,
        "primary_references": {
            "algorithm": "https://encoding.spec.whatwg.org/#shift_jis-decoder",
            "index": "https://encoding.spec.whatwg.org/index-jis0208.txt",
            "microsoft": "https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP932.TXT",
            "unicode_license": "https://www.unicode.org/license.txt",
            "distribution": "Reference tables stay ignored/private. Tracked tools are original algorithms/readers; no table data or upstream implementation is copied.",
        },
        "microsoft_defined_mappings": len(microsoft),
        "microsoft_vs_whatwg_differences": mapping_differences,
        "valid_decoder_cases": sum(codepoint is not None for _, codepoint in decoder),
        "refused_single_byte_tokens": sum(codepoint is None for _, codepoint in decoder),
        "valid_encoder_cases": sum(token is not None for _, token in encoder),
        "refused_encoder_cases": sum(token is None for _, token in encoder),
        "controls": len(observations),
        "matched_mapping_cases": matched_mapping_cases,
        "compared_cells": compared_cells,
        "disagreements": disagreements,
        "observations": observations,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(json.dumps({key: report[key] for key in [
        "controls", "valid_decoder_cases", "valid_encoder_cases",
        "matched_mapping_cases", "compared_cells", "disagreements",
    ]}))
    if disagreements:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
