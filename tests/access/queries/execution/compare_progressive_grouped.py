"""Compare original native/CAB grouped reads with independent row models.

No fixture is authored here. Existing inert getters and CAB inspection qualify
immutable inputs; only TableCraft executes SQL. This is engineering evidence,
not native Access SQL or timing qualification.
"""

import argparse
from fractions import Fraction
import hashlib
import io
import csv
import json
from pathlib import Path
import subprocess

from build_large_deployment import digest, inspect
from compare_large_deployment import JAR_SHA256, getter_model

ROOT = Path(__file__).resolve().parents[2]
PROVENANCE_HASH = "e5c4e1f575d96bd8552ce05f610505b6987ee0e1946ac740886468cec094d56b"
MAX_COMMANDS = 100
SOURCE_ROOTS = [
    "crates/query/tablecraft-query/src/execution/progressive.rs",
    "crates/query/tablecraft-query/src/execution/progressive",
    "crates/query/tablecraft-query/src/execution/group_values.rs",
    "crates/query/tablecraft-query/src/execution/grouping.rs",
    "crates/query/tablecraft-query/src/execution/exact_keys",
    "crates/query/tablecraft-query/src/execution/aggregate/accumulator.rs",
    "crates/query/tablecraft-query/src/plan.rs",
    "crates/query/tablecraft-query/src/binding/retrieval.rs",
    "crates/backends/tablecraft-access/src/source/progressive.rs",
    "crates/backends/tablecraft-access/src/input/deployment",
    "apps/tablecraft-cli/src/commands/query/retrieval.rs",
]
TOOLS = [
    "docs/testing/compare_progressive_grouped.py",
    "docs/testing/compare_large_deployment.py",
    "docs/testing/build_large_deployment.py",
    "docs/testing/LargeDeploymentFixture.java",
]


def invoke(arguments):
    return subprocess.run(
        [str(argument) for argument in arguments],
        cwd=ROOT,
        capture_output=True,
        text=True,
        timeout=120,
    )


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def groups(key):
    """Linear first-encounter grouping independent of production directories."""
    result = []
    for identity in range(1, 1101):
        current = key(identity)
        for retained, members in result:
            if retained == current:
                members.append(identity)
                break
        else:
            result.append((current, [identity]))
    return result


def number(value):
    if value is None:
        return "NULL"
    rational = Fraction(value)
    if rational.denominator == 1:
        return str(rational.numerator)
    if rational.denominator == 2:
        return f"{rational.numerator // 2}.5"
    raise ValueError("Model expects only exact integers and half-integers")


def model_cases():
    population = groups(lambda identity: (identity % 17,))
    numeric = [["K", "N", "S", "A", "Lo", "Hi"]]
    for (key,), members in population:
        mean = Fraction(sum(members), len(members))
        assert mean == Fraction(members[0] + members[-1], 2)
        numeric.append([number(value) for value in [
            key, len(members), sum(members), mean, min(members), max(members)
        ]])
    sql = (
        "SELECT Id Mod 17 AS K,Count(*) AS N,Sum(Id) AS S,Avg(Id) AS A,"
        "Min(Id) AS Lo,Max(Id) AS Hi FROM LargeDeployment GROUP BY Id Mod 17"
    )
    for stream, batch in [(True, 1), (True, 128), (False, None)]:
        yield "numeric", sql, numeric, stream, batch, (), None

    key_expression = "IIf(Id Mod 2=0,NULL,Id Mod 5)"
    tuples = groups(lambda identity: (
        None if identity % 2 == 0 else identity % 5,
        identity % 3,
    ))
    expected = [["K", "J", "N", "S"]]
    for (key, second), members in tuples:
        expected.append([number(value) for value in [key, second, len(members), sum(members)]])
    sql = (
        f"SELECT {key_expression} AS K,Id Mod 3 AS J,Count(*) AS N,Sum(Id) AS S "
        f"FROM LargeDeployment GROUP BY {key_expression},Id Mod 3"
    )
    for streaming in [True, False]:
        yield "nullable-tuples", sql, expected, streaming, 128, (), None

    expected = [["K", "Lo", "Hi"]]
    for (key,), members in population:
        spellings = [str(identity) for identity in members]
        expected.append([str(key), json.dumps(min(spellings)), json.dumps(max(spellings))])
    sql = (
        "SELECT Id Mod 17 AS K,Min(CStr(Id)) AS Lo,Max(CStr(Id)) AS Hi "
        "FROM LargeDeployment GROUP BY Id Mod 17"
    )
    for streaming in [True, False]:
        yield "ordinal-string-extrema", sql, expected, streaming, 128, (), None

    expected = [["K", "N", "NextN", "CountType", "SumType", "MeanType"]]
    for (key,), members in population[:3]:
        expected.append([str(value) for value in [key, len(members), len(members) + 1, 20, 20, 5]])
    sql = (
        "SELECT TOP 3 Id Mod 17 AS K,Count(*) AS N,N+1 AS NextN,"
        "VarType(N) AS CountType,VarType(Sum(Id)) AS SumType,VarType(Avg(Id)) AS MeanType "
        "FROM LargeDeployment GROUP BY Id Mod 17"
    )
    for streaming in [True, False]:
        yield "aliases-top-types", sql, expected, streaming, 128, (), None

    expected = [["K", "V", "T"]]
    for (key,), members in population:
        expected.append([str(key), str(min(members)), "3"])
    for (key,), members in population:
        expected.append([str(key), str(max(members)), "5"])
    sql = (
        "SELECT Id Mod 17 AS K,Min(Id) AS V,VarType(Min(Id)) AS T "
        "FROM LargeDeployment GROUP BY Id Mod 17 UNION ALL "
        "SELECT Id Mod 17,CDbl(Max(Id)),VarType(CDbl(Max(Id))) "
        "FROM LargeDeployment GROUP BY Id Mod 17"
    )
    for streaming in [True, False]:
        yield "union-promoted-carriers", sql, expected, streaming, 128, (), None

    yield "declared-union-carriers", sql, expected, True, 128, ("--declared-arithmetic",), None

    unsupported = "SELECT Label,Count(*) FROM LargeDeployment GROUP BY Label"
    yield "text-key-refusal", unsupported, None, True, 128, (), "query feature is not implemented"
    for option, ceiling in [
        ("--execution-rows", "1"),
        ("--execution-bytes", "1"),
        ("--execution-steps", "1"),
    ]:
        yield option, sql, None, True, 128, (option, ceiling), "resource"


def source_pins(revision):
    listing = invoke(["git", "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS])
    listing.check_returncode()
    result = {}
    for name in listing.stdout.splitlines():
        if name.endswith(".rs"):
            content = subprocess.run(
                ["git", "show", f"{revision}:{name}"],
                cwd=ROOT,
                capture_output=True,
                check=True,
                timeout=120,
            ).stdout
            result[name] = hashlib.sha256(content).hexdigest()
    if not any("progressive/groups" in name for name in result):
        raise ValueError("Compiled revision lacks progressive grouping")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    relative(args.report)
    if args.report.exists():
        raise FileExistsError("Refusing to overwrite report")
    if digest(args.cli) != args.cli_sha256 or digest(args.jar) != JAR_SHA256:
        raise ValueError("Frozen executable or reviewed reader hash differs")
    if digest(args.provenance) != PROVENANCE_HASH:
        raise ValueError("Registered corpus provenance differs")
    provenance = json.loads(args.provenance.read_text())
    inputs = [args.directory / name for name in provenance["files"]]
    for path in inputs:
        if digest(path) != provenance["files"][path.name]["sha256"]:
            raise ValueError("Registered original input differs")
    cases = list(model_cases())
    if len(inputs) * len(cases) > MAX_COMMANDS:
        raise ValueError("Replay command ceiling exceeded")
    pinned = [args.cli, args.jar, args.provenance, *inputs]
    pinned.extend(ROOT / name for name in TOOLS)
    before = {relative(path): digest(path) for path in pinned}
    sources = source_pins(args.revision)
    getters = {}
    for profile in ["V2016", "V2019"]:
        original = args.directory / f"{profile}.accdb"
        getters[profile] = getter_model(args.jar, original)
        for compression in ["stored", "mszip"]:
            package = args.directory / f"{profile}-{compression}.accdc"
            getters[profile][compression] = inspect(package, original.stat().st_size, digest(original))

    records = []
    for path in inputs:
        for label, sql, expected, streaming, batch, overrides, error in cases:
            command = [
                args.cli,
                "--format", "tsv",
                "query", "run", sql,
                "--database", path,
                "--native-bytes", "268435456",
                "--native-steps", "10000000",
                "--number-format", "invariant",
                "--collation", "ordinal",
            ]
            if streaming:
                command.append("--stream")
                command.extend(["--batch-rows", str(batch)])
            command.extend(overrides)
            process = invoke(command)
            actual = list(csv.reader(io.StringIO(process.stdout), delimiter="\t"))
            if error is None:
                matched = process.returncode == 0 and actual == expected
            else:
                matched = process.returncode != 0 and not process.stdout and error in process.stderr.lower()
            records.append({
                "case": label,
                "streaming": streaming,
                "command": [relative(value) if isinstance(value, Path) else value for value in command],
                "expected_rows": expected,
                "expected_error_fragment": error,
                "exit_code": process.returncode,
                "stdout": process.stdout,
                "stderr": process.stderr,
                "matched": matched,
            })
    after = {relative(path): digest(path) for path in pinned}
    report = {
        "scope": "Original native/CAB grouped engineering replay; no native Access SQL or timing claims",
        "compiled_revision": args.revision,
        "compiled_source_sha256": sources,
        "immutable_sha256_before": before,
        "immutable_sha256_after": after,
        "pins_unchanged": before == after,
        "getters": getters,
        "cli_commands": len(records),
        "success_commands": sum(record["expected_rows"] is not None for record in records),
        "refusal_commands": sum(record["expected_rows"] is None for record in records),
        "disagreements": sum(not record["matched"] for record in records),
        "commands": records,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as destination:
        json.dump(report, destination, indent=2)
        destination.write("\n")
    print(json.dumps({key: report[key] for key in [
        "cli_commands", "success_commands", "refusal_commands", "disagreements", "pins_unchanged"
    ]}, indent=2))
    if report["disagreements"] or not report["pins_unchanged"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
