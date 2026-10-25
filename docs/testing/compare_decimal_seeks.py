"""Compare exact native Decimal index points with independent getter Fractions."""

import argparse
from collections import Counter
from decimal import Decimal
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
TIMEOUT_SECONDS = 60
TABLE_SCALES = [0, 4, 28]
MODES = {
    "automatic": ["--auto-index"],
    "ascending": ["--index", "{table}", "DecimalAsc"],
    "descending": ["--index", "{table}", "DecimalDesc"],
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def invoke(arguments):
    return subprocess.run(
        [str(argument) for argument in arguments],
        capture_output=True,
        text=True,
        timeout=TIMEOUT_SECONDS,
    )


def mode_flags(mode, table):
    if mode == "sequential":
        return []
    return [argument.format(table=table) for argument in MODES[mode]]


def fixed(coefficient, scale):
    sign = int(coefficient < 0)
    digits = tuple(int(character) for character in str(abs(coefficient)))
    return Decimal((sign, digits, -scale))


class Replay:
    def __init__(self, cli):
        self.cli = cli
        self.observations = []
        self.compared_rows = 0

    def check(self, database, table, label, mode, sql, expected=None,
              parameter=None, limit=500, error=None, extra=()):
        arguments = [
            self.cli, "--format", "tsv", "query", "run", sql,
            "--database", database, "--native-rows", limit,
            *mode_flags(mode, table), *extra,
        ]
        if parameter is not None:
            arguments.extend(["--param", parameter])

        actual = invoke(arguments)
        record = {
            "profile": database.stem, "table": table, "case": label,
            "mode": mode, "sql": sql, "parameter": parameter, "native_rows": limit,
        }
        if error is not None:
            if actual.returncode == 0 or actual.stdout or error not in actual.stderr.lower():
                raise RuntimeError(f"Expected buffered {error}: {arguments}\n{actual.stderr}")
            record["refusal"] = error
            record["stderr"] = actual.stderr
        else:
            expected_text = "ID\n" + "".join(f"{identity}\n" for identity in expected)
            if actual.returncode or actual.stdout != expected_text:
                raise RuntimeError(f"Decimal point differs: {arguments}\n{actual.stderr}")
            record["rows"] = len(expected)
            record["stdout_sha256"] = hashlib.sha256(actual.stdout.encode()).hexdigest()
            self.compared_rows += len(expected)
        self.observations.append(record)

    def point(self, database, table, records, mode, value, scale, label, reverse=False,
              limit=500):
        operand = "P=[Value]" if reverse else "[Value]=P"
        sql = f"PARAMETERS P Decimal(28,{scale}); SELECT ID FROM [{table}] WHERE {operand}"
        rational = Fraction(value)
        expected = sorted(identity for identity, item in records if item == rational)
        self.check(
            database, table, label, mode, sql,
            expected=expected, parameter=f"P=decimal:{format(value, 'f')}", limit=limit,
        )


def getters(jar, database, table, scale):
    result = invoke([
        "java", "-Dstdout.encoding=UTF-8", "-cp", jar,
        "docs/testing/IndexRowsOracle.java", database, table, "DecimalAsc", "ID", "Value",
    ])
    if result.returncode:
        raise RuntimeError(result.stderr)

    records = []
    points = set()
    for line in result.stdout.splitlines():
        identity, raw = line.split("\t")
        if raw == "NULL":
            records.append((int(identity), None))
            continue
        value = Decimal(raw)
        if not value.is_finite() or value.as_tuple().exponent != -scale:
            raise RuntimeError("Independent getter lost the declared native Decimal scale")
        records.append((int(identity), Fraction(value)))
        points.add(value)

    if len(records) != 1000 or len(points) != 16:
        raise RuntimeError("Original Decimal boundary corpus has unexpected cardinality")
    return records, sorted(points), hashlib.sha256(result.stdout.encode()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--inputs", required=True, nargs=4, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Accepted report must be fresh")
    if digest(args.jar) != JAR_SHA256:
        raise RuntimeError("Independent reader differs from reviewed Jackcess 5.0.4")

    pinned = [args.cli, args.jar, Path(__file__), *args.inputs]
    pinned.extend(Path(path) for path in [
        "docs/testing/DecimalIndexFixture.java",
        "docs/testing/IndexRowsOracle.java",
        "crates/engines/tablecraft-ace/src/table/indexes/decimal.rs",
        "crates/engines/tablecraft-ace/src/table/indexes/decimal/selection.rs",
        "crates/engines/tablecraft-ace/src/table/indexes/profile.rs",
        "crates/engines/tablecraft-ace/src/table/indexes/projection.rs",
        "crates/engines/tablecraft-ace/src/table/indexes/read.rs",
        "crates/engines/tablecraft-ace/src/table/indexes/mutation/plan.rs",
        "crates/query/tablecraft-query/src/source/constraint.rs",
        "crates/query/tablecraft-query/src/execution/constraint.rs",
        "crates/backends/tablecraft-access/src/source/selection.rs",
        "crates/backends/tablecraft-access/src/source/selection/decimal.rs",
    ])
    hashes = {relative(path): digest(path) for path in pinned}
    replay = Replay(args.cli)
    getter_records = {}
    for database in args.inputs:
        for scale in TABLE_SCALES:
            table = f"Decimal{scale}"
            records, points, raw_hash = getters(args.jar, database, table, scale)
            getter_records[f"{database.name}/{table}"] = raw_hash
            for mode in MODES:
                for index, value in enumerate(points):
                    replay.point(database, table, records, mode, value, scale, f"boundary-{index}")
                replay.point(database, table, records, mode, fixed(2, scale), scale, "absent")

                nulls = sorted(identity for identity, value in records if value is None)
                replay.check(database, table, "null", mode,
                             f"SELECT ID FROM [{table}] WHERE [Value] IS NULL", expected=nulls)

                # Change the retained scale while preserving a stored value of coefficient10.
                parameter_scale = scale - 1 if scale else 1
                parameter_value = fixed(1, parameter_scale) if scale else fixed(100, 1)
                replay.point(database, table, records, mode, parameter_value, parameter_scale,
                             "equivalent-scale-reversed", reverse=True)

            replay.point(database, table, records, "sequential", fixed(0, scale), scale,
                         "complete-snapshot-control", limit=2000)

        for mode in MODES:
            for table, scale, value, label in [
                ("Decimal4", 5, "0.00001", "lossy-field-rescale"),
                ("Decimal28", 0, "1", "native-field-precision"),
            ]:
                sql = f"PARAMETERS P Decimal(28,{scale}); SELECT ID FROM [{table}] WHERE [Value]=P"
                replay.check(database, table, label, mode, sql,
                             parameter=f"P=decimal:{value}", error="structural limit")
            for predicate, label in [
                ("[Value]=CDec(0)", "conversion-call"),
                ("[Value]=0", "integral-literal"),
                ("[Value] IS NOT NULL", "not-null"),
            ]:
                replay.check(database, "Decimal4", label, mode,
                             f"SELECT ID FROM Decimal4 WHERE {predicate}", error="structural limit")

            sql = "PARAMETERS P Double; SELECT ID FROM Decimal4 WHERE [Value]=P"
            replay.check(database, "Decimal4", "double-parameter", mode, sql,
                         parameter="P=double:0", error="structural limit")
            sql = "PARAMETERS P Decimal(28,4); SELECT ID FROM Decimal4 WHERE [Value]=P And 1/0=0"
            replay.check(database, "Decimal4", "eager-conjunction", mode, sql,
                         parameter="P=decimal:0.0000", limit=2000, error="division")

        sql = "PARAMETERS P Decimal(28,4); SELECT ID FROM Decimal4 WHERE [Value]=P"
        replay.check(database, "Decimal4", "native-row-budget", "automatic", sql,
                     parameter="P=decimal:0.0000", limit=1, error="resource limit")
        replay.check(database, "Decimal4", "output-budget", "automatic", sql,
                     parameter="P=decimal:0.0000", extra=["--output-bytes", "1"], error="output")

    if hashes != {relative(path): digest(path) for path in pinned}:
        raise RuntimeError("Pinned originals, reader, executable or scoped source changed")
    refusals = Counter(item["refusal"] for item in replay.observations if "refusal" in item)
    report = {
        "date": "2026-10-25", "scope": "Independent exact Fraction getter model; native Access unrun",
        "controls": len(replay.observations), "compared_id_rows": replay.compared_rows,
        "buffered_refusals": dict(refusals), "getter_stdout_sha256": getter_records,
        "pins": hashes, "observations": replay.observations,
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {len(replay.observations)} controls; all {len(hashes)} pins unchanged")


if __name__ == "__main__":
    main()
