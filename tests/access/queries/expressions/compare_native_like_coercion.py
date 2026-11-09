"""Replay coerced LIKE using original scalar files and pinned ordinary getters.

Only TableCraft executes SQL. Expected spellings use exact integer/Fraction and
Gregorian calendar facts under the explicitly selected invariant/ISO profile.
No CStr expression is executed as an answer oracle. This is not an Access SQL
comparison, a locale qualification, or a performance benchmark.
"""

import argparse
import csv
from datetime import datetime
from fractions import Fraction
import hashlib
import io
import json
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
GETTER_SHA256 = "588a43a018a37316bfb2df9c8dff1feaa6b53f0c18d55543125f526a9d439d0a"
PROVENANCE = "docs/testing/results/declared-numeric-text-replay-2026-10-25.json"
PROVENANCE_SHA256 = "109d367935afbe8684819b3ea8c81e776d928864b5c6b94fe46ad0379adef327"
INPUT_SHA256 = {
    "Scalar-V2000.mdb": "de2e67a9f0194988d031cc9c981176ae9a76d3bf80599040729b408fc2b7be82",
    "Scalar-V2007.accdb": "4ed9555c239e56f0649040fb9357913e4e38dcbaac5c2cb668614857882922bc",
    "Scalar-V2016.accdb": "95c52665d28869a92764f0958b62ad53e465847e3a0c637829d4ff29c8f5ef37",
    "Scalar-V2019.accdb": "eef6a7d1f23c482333ad5b70b312125bd89799ea168b10a522491eb4b7625190",
}
COLUMNS = {
    "ID": 1,
    "Flag": 2,
    "Octet": 3,
    "Small": 4,
    "Money": 5,
    "Real": 6,
    "WideReal": 7,
    "Amount": 13,
    "Stamp": 14,
}
SOURCE_PATHS = (
    "crates/query/tablecraft-expressions/src/evaluation/context.rs",
    "crates/query/tablecraft-expressions/src/evaluation/like.rs",
    "crates/query/tablecraft-expressions/src/evaluation/like/preparation.rs",
    "crates/query/tablecraft-expressions/src/evaluation/mod.rs",
    "crates/query/tablecraft-query/src/execution/declarations/predicates/like.rs",
    "crates/backends/tablecraft-access/src/source/snapshot.rs",
    "crates/backends/tablecraft-access/src/source/projection.rs",
    "apps/tablecraft-cli/src/commands/query/execution_context.rs",
)
TIMEOUT_SECONDS = 60
CONTROL_COUNT = 136


def digest(path):
    checksum = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(block)
    return checksum.hexdigest()


def relative(path):
    return Path(path).resolve().relative_to(ROOT).as_posix()


def invoke(arguments):
    return subprocess.run(
        [str(argument) for argument in arguments],
        cwd=ROOT,
        capture_output=True,
        timeout=TIMEOUT_SECONDS,
        check=False,
    )


def snapshot_sources(revision):
    snapshots = {}
    for path in SOURCE_PATHS:
        result = invoke(["git", "show", f"{revision}:{path}"])
        if result.returncode:
            raise RuntimeError(result.stderr.decode())
        snapshots[path] = {
            "bytes": len(result.stdout),
            "sha256": hashlib.sha256(result.stdout).hexdigest(),
        }
    return snapshots


def finite_decimal(value):
    """Render the exact finite rational; never round through a binary float."""
    numerator, denominator = value.numerator, value.denominator
    sign = "-" if numerator < 0 else ""
    numerator = abs(numerator)
    whole, remainder = divmod(numerator, denominator)
    digits = []
    while remainder:
        if len(digits) >= 28:
            raise ValueError("Unexpected nonfinite or oversized fixture fraction")
        digit, remainder = divmod(remainder * 10, denominator)
        digits.append(str(digit))
    fraction = "." + "".join(digits) if digits else ""
    return sign + str(whole) + fraction


def spelling(tag):
    if tag == "N":
        return None
    if tag in ("B0", "B1"):
        return "True" if tag == "B1" else "False"
    if tag.startswith(("I16:", "I32:")):
        return str(int(tag.split(":", 1)[1]))
    if tag.startswith("U"):
        return str(int(tag[1:]))
    if tag.startswith("C"):
        return finite_decimal(Fraction(int(tag[1:]), 10000))
    if tag.startswith("Q"):
        coefficient, scale = map(int, tag[1:].split(":"))
        return finite_decimal(Fraction(coefficient, 10**scale))
    if tag.startswith(("F", "D")):
        format_code = ">f" if tag[0] == "F" else ">d"
        value = struct.unpack(format_code, bytes.fromhex(tag[1:]))[0]
        result = finite_decimal(Fraction.from_float(value))
        # The original uses only signed zero, 1.25 and 1.5. No general floating
        # formatter, precision threshold or scientific-notation oracle is implied.
        if result not in ("0", "1.25", "1.5"):
            raise ValueError("Unexpected fixture float outside the exact model")
        return result
    if tag.startswith("Y"):
        civil, nanos = tag[1:].split(".")
        if set(nanos) != {"0"}:
            raise ValueError("Unexpected fractional calendar second")
        value = datetime.fromisoformat(civil)
        if value.date().isoformat() == "1899-12-30":
            return value.time().isoformat()
        if value.time().isoformat() == "00:00:00":
            return value.date().isoformat()
        return value.isoformat(sep=" ")
    raise ValueError(f"Unexpected getter tag: {tag[:16]}")


def literal(value):
    return "NULL" if value is None else "'" + value.replace("'", "''") + "'"


def cases(rows):
    for column, position in COLUMNS.items():
        field = f"[{column}]"
        spellings = [spelling(row[position]) for row in rows]
        expected_pattern = literal(spellings[-1])
        for row, text in reversed(list(zip(rows[:-1], spellings[:-1]))):
            identifier = int(row[1].split(":", 1)[1])
            expected_pattern = f"IIf(ID={identifier},{literal(text)},{expected_pattern})"
        sql = (
            f"SELECT ID,{field} LIKE {expected_pattern} AS ForwardMatch,"
            f"{expected_pattern} LIKE {field} AS ReverseMatch,"
            f"{field} NOT LIKE {expected_pattern} AS NegatedMatch "
            "FROM ScalarSamples ORDER BY ID"
        )
        expected = [["ID", "ForwardMatch", "ReverseMatch", "NegatedMatch"]]
        for row, text in zip(rows, spellings):
            results = ["NULL"] * 3 if text is None else ["TRUE", "TRUE", "FALSE"]
            expected.append([row[1].split(":", 1)[1], *results])
        yield {"name": column + "-both-positions", "sql": sql, "expected": expected}

    selected = [row[1].split(":", 1)[1] for row in rows if spelling(row[1]).startswith("2")]
    yield {
        "name": "numeric-like-parent-filter",
        "sql": "SELECT ID FROM ScalarSamples WHERE ID LIKE '2*' ORDER BY ID",
        "expected": [["ID"], *[[identifier] for identifier in selected]],
    }
    yield {
        "name": "null-before-conversion-and-provider",
        "sql": "SELECT ID,NULL LIKE [Money] AS A,Stamp NOT LIKE NULL AS B FROM ScalarSamples ORDER BY ID",
        "omit": ("number", "date", "collation"),
        "expected": [["ID", "A", "B"], *[[row[1][4:], "NULL", "NULL"] for row in rows]],
    }
    for name, expression, omitted, diagnostic in (
        ("number-policy-required", "[Money] LIKE '*'", ("number",), "number-format"),
        ("date-policy-required", "Stamp LIKE '*'", ("date",), "date-format"),
        ("comparison-provider-required", "ID LIKE '1*'", ("collation",), "collation"),
        ("error-state-not-stringified", "ID LIKE CVErr(7)", (), "type mismatch"),
        ("eager-null-peer-expression", "NULL LIKE (1/(ID-ID))", (), "division by zero"),
        ("malformed-pattern-after-conversion", "ID LIKE '['", (), "pattern"),
    ):
        yield {
            "name": name,
            "sql": f"SELECT {expression} AS V FROM ScalarSamples WHERE ID=1",
            "omit": omitted,
            "diagnostic": diagnostic,
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--build-manifest", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    args = parser.parse_args()
    if args.report.exists() or args.report.with_suffix("").exists():
        parser.error("Report and private artifact directory must be fresh")
    if digest(args.cli) != args.cli_sha256 or digest(args.jar) != JAR_SHA256:
        parser.error("Executable or reviewed getter library hash mismatch")
    if digest(ROOT / PROVENANCE) != PROVENANCE_SHA256:
        parser.error("Previously reviewed getter provenance changed")

    manifest = json.loads(args.build_manifest.read_text())
    if manifest["revision"] != args.revision or manifest["sha256"] != args.cli_sha256:
        parser.error("Build manifest revision/executable mismatch")

    artifacts = args.report.with_suffix("")
    artifacts.mkdir(parents=True)
    pins = [args.cli, args.jar, args.build_manifest, Path(__file__), ROOT / PROVENANCE,
            ROOT / "docs/testing/TableDataOracle.java", ROOT / "docs/testing/ScalarFixture.java"]
    record = {
        "scope": __doc__,
        "compiled_revision": args.revision,
        "build_manifest": relative(args.build_manifest),
        "dirty_native_production_sources": manifest["dirty_production_sources"],
        "build_scope": (
            "The frozen CLI includes the committed LIKE/query sources and eight "
            "pending native check-only production changes pinned in its build manifest. "
            "No clean-full-tree executable claim is made."
        ),
        "scoped_source_snapshots": snapshot_sources(args.revision),
        "snapshot_scope": "Selected raw git-show bytes; not a complete compilation manifest.",
        "pins": {relative(path): digest(path) for path in pins},
        "inputs": [],
        "controls": [],
        "native_access_execution": "Unavailable; no native Access LIKE execution or localization qualification.",
    }

    def save():
        args.report.write_text(json.dumps(record, indent=2) + "\n")

    for name, expected_hash in INPUT_SHA256.items():
        database = args.directory / name
        if digest(database) != expected_hash:
            raise RuntimeError("Unregistered or changed original scalar input")
        getter = invoke([
            "java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
            ROOT / "docs/testing/TableDataOracle.java", database, "ScalarSamples",
        ])
        prefix = name.split(".")[0]
        raw_path = artifacts / (prefix + "-getter.tsv")
        raw_path.write_bytes(getter.stdout)
        raw_path.with_suffix(".stderr").write_bytes(getter.stderr)
        if getter.returncode or getter.stderr or hashlib.sha256(getter.stdout).hexdigest() != GETTER_SHA256:
            raise RuntimeError("Ordinary getter differs from the previously pinned transcript")
        rows = [line.split("\t") for line in getter.stdout.decode("utf-8").splitlines()]
        if len(rows) != 3 or any(len(row) != 15 or row[0] != "ScalarSamples" for row in rows):
            raise RuntimeError("Unexpected scalar getter model shape")
        model = list(cases(rows))
        if len(model) != 17:
            raise RuntimeError("Unexpected finite LIKE case matrix")
        record["inputs"].append({
            "path": relative(database), "sha256": expected_hash,
            "getter": relative(raw_path), "getter_sha256": GETTER_SHA256,
        })
        for declared in (False, True):
            for case in model:
                omitted = case.get("omit", ())
                command = [args.cli, "query", "run", case["sql"], "--database", database,
                           "--like-mode", "ansi89", "--format", "tsv"]
                if declared:
                    command.append("--declared-arithmetic")
                for key, flag, value in (
                    ("number", "--number-format", "invariant"),
                    ("date", "--date-format", "iso"),
                    ("collation", "--collation", "ordinal"),
                ):
                    if key not in omitted:
                        command.extend((flag, value))
                result = invoke(command)
                stdout = result.stdout.decode("utf-8")
                stderr = result.stderr.decode("utf-8")
                diagnostic = case.get("diagnostic")
                if diagnostic:
                    passed = result.returncode == 2 and not stdout and diagnostic in stderr.lower()
                else:
                    actual = list(csv.reader(io.StringIO(stdout), delimiter="\t", strict=True))
                    passed = result.returncode == 0 and not stderr and actual == case["expected"]
                control = {
                    "name": f"{prefix}:{'declared' if declared else 'ordinary'}:{case['name']}",
                    "command": [relative(value) if isinstance(value, Path) else value for value in command],
                    "exit_code": result.returncode, "passed": passed,
                    "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                    "stderr": stderr,
                }
                if not diagnostic:
                    control["expected"] = case["expected"]
                    control["expected_cells"] = sum(map(len, case["expected"][1:]))
                else:
                    control["expected_diagnostic"] = diagnostic
                    control["expected_cells"] = 0
                record["controls"].append(control)
                save()
                if not passed:
                    raise RuntimeError(f"LIKE disagreement: {control['name']}: {stdout!r} {stderr!r}")
        if digest(database) != expected_hash:
            raise RuntimeError("Original input changed during replay")
        record["inputs"][-1]["unchanged"] = True
        print(name, "34 controls passed", flush=True)

    if any(digest(ROOT / path) != expected for path, expected in record["pins"].items()):
        raise RuntimeError("An execution/reference pin changed during replay")
    record["control_count"] = len(record["controls"])
    if record["control_count"] != CONTROL_COUNT:
        raise RuntimeError("Unexpected completed LIKE control count")
    record["modeled_cells"] = sum(control["expected_cells"] for control in record["controls"])
    record["all_originals_and_pins_unchanged"] = True
    save()
    print(record["control_count"], "controls;", record["modeled_cells"], "modeled cells; zero disagreements")


if __name__ == "__main__":
    main()
