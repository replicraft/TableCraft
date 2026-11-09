"""Compare original native/CAB Text grouping with bounded independent models.

General expectations cover ASCII case and trailing-space equality only, following
the existing explicit engineering contract. No native Access SQL or timing claim.
"""

import argparse
import csv
from fractions import Fraction
import io
import json
from pathlib import Path

from build_large_deployment import digest, inspect
from compare_large_deployment import JAR_SHA256, getter_model
from compare_progressive_derived import compiled_sources
from compare_progressive_grouped import PROVENANCE_HASH, invoke, number, relative

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 100
TOOLS = [
    "docs/testing/compare_progressive_text_groups.py",
    "docs/testing/compare_progressive_derived.py",
    "docs/testing/compare_progressive_grouped.py",
    "docs/testing/compare_large_deployment.py",
    "docs/testing/build_large_deployment.py",
    "docs/testing/LargeDeploymentFixture.java",
]


def label(identity):
    return f"row-{identity:06d}"


def groups(key, equivalence=lambda value: value):
    result = []
    for identity in range(1, 1101):
        current = key(identity)
        for original, members in result:
            if equivalence(current) == equivalence(original):
                members.append(identity)
                break
        else:
            result.append((current, [identity]))
    return result


def text(value):
    return "NULL" if value is None else json.dumps(value)


def decorated(identity):
    prefix = "ROW-" if identity % 2 == 0 else "row-"
    suffix = " " if identity % 3 == 0 else ""
    return prefix + label(identity)[4:8] + suffix


def controls():
    population = groups(lambda identity: label(identity)[:8])
    assert len(population) == 12
    sql = (
        "SELECT K,Count(*) AS N,Sum(Id) AS S,Avg(Id) AS A,Min(Id) AS Lo,Max(Id) AS Hi "
        "FROM (SELECT Id,Left(Label,8) AS K FROM LargeDeployment) Q GROUP BY K"
    )
    expected = [["K", "N", "S", "A", "Lo", "Hi"]]
    for key, members in population:
        mean = Fraction(sum(members), len(members))
        assert mean == Fraction(members[0] + members[-1], 2)
        expected.append([text(key), *[number(value) for value in [
            len(members), sum(members), mean, min(members), max(members)
        ]]])
    for streaming, batch in [(False, None), (True, 1), (True, 128)]:
        yield "prefix-numeric", sql, expected, streaming, batch, "ordinal", (), None

    key_expression = "(IIf(Id Mod 2=0,'ROW-','row-')&Mid(Label,5,4))&IIf(Id Mod 3=0,' ','')"
    sql = (
        f"SELECT K,Count(*) AS N FROM (SELECT {key_expression} AS K "
        "FROM LargeDeployment) Q GROUP BY K"
    )
    for collation, streaming in [
        ("ordinal", True),
        ("general-legacy", False),
        ("general-legacy", True),
        ("general-2010", True),
    ]:
        equivalence = (lambda value: value) if collation == "ordinal" else (lambda value: value.lower().rstrip(" "))
        expected = [["K", "N"]]
        expected.extend([text(key), str(len(members))] for key, members in groups(decorated, equivalence))
        work = ("--execution-steps", "10000000") if collation == "ordinal" else ()
        yield "ascii-case-pad", sql, expected, streaming, 128, collation, work, None

    def tuple_key(identity):
        return (None if identity % 13 == 0 else label(identity)[:8], identity % 3)

    expected = [["K", "J", "N", "S"]]
    for (key, fixed), members in groups(tuple_key):
        expected.append([text(key), str(fixed), str(len(members)), str(sum(members))])
    sql = (
        "SELECT K,J,Count(*) AS N,Sum(Id) AS S FROM "
        "(SELECT Id,IIf(Id Mod 13=0,NULL,Left(Label,8)) AS K,CDec(Id Mod 3) AS J "
        "FROM LargeDeployment) Q GROUP BY K,J"
    )
    for streaming in [False, True]:
        yield "nullable-text-fixed-tuples", sql, expected, streaming, 128, "ordinal", (), None

    expected = [["K", "N"]]
    expected.extend([text(key), str(len(members))] for key, members in population
                    if key[:7] == "row-000" and len(members) >= 100)
    sql = (
        "SELECT K,Count(*) AS N FROM (SELECT Left(Label,8) AS K FROM LargeDeployment) Q "
        "GROUP BY K HAVING Left(K,7)='row-000' AND Count(*)>=100"
    )
    yield "text-having", sql, expected, True, 128, "ordinal", (), None

    for predicate in ["False", "NULL"]:
        sql = (
            "SELECT K,Sum(1/0) AS Bad FROM (SELECT Left(Label,8) AS K FROM LargeDeployment) Q "
            f"GROUP BY K HAVING {predicate}"
        )
        yield "rejected-projection-" + predicate, sql, [["K", "Bad"]], True, 128, "ordinal", (), None

    sql = (
        "SELECT LenB(K) AS Bytes,Count(*) AS N FROM "
        "(SELECT LeftB(Label,16) AS K FROM LargeDeployment) Q GROUP BY K"
    )
    expected = [["Bytes", "N"]]
    expected.extend(["16", str(len(members))] for _, members in population)
    yield "valid-raw-key-observer", sql, expected, True, 128, "ordinal", (), None

    sql = (
        "SELECT TOP 1 K,10\\(Max(Id)-1100) AS V FROM "
        "(SELECT Id,Left(Label,8) AS K FROM LargeDeployment) Q GROUP BY K"
    )
    prefix = [["K", "V"], [text(population[0][0]), "0"]]
    yield "late-final-error-prefix", sql, prefix, True, 1, "ordinal", (), "division by zero"
    sql = (
        "SELECT K,Sum(10\\(Id-1100)) AS V FROM "
        "(SELECT Id,Left(Label,8) AS K FROM LargeDeployment) Q GROUP BY K"
    )
    yield "operand-replay-error-before-output", sql, None, True, 1, "ordinal", (), "division by zero"
    sql = "SELECT Label,Count(*) FROM LargeDeployment GROUP BY Label"
    yield "low-query-work", sql, None, True, 128, "ordinal", ("--execution-steps", "1"), "resource"


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
        raise ValueError("Frozen executable or reviewed reader differs")
    if digest(args.provenance) != PROVENANCE_HASH:
        raise ValueError("Registered original provenance differs")
    provenance = json.loads(args.provenance.read_text())
    inputs = [args.directory / name for name in provenance["files"]]
    for path in inputs:
        if digest(path) != provenance["files"][path.name]["sha256"]:
            raise ValueError("Registered original input differs")
    cases = list(controls())
    if len(cases) * len(inputs) > MAX_COMMANDS:
        raise ValueError("Command ceiling exceeded before replay")
    pinned = [args.cli, args.jar, args.provenance, *inputs]
    pinned.extend(ROOT / name for name in TOOLS)
    before = {relative(path): digest(path) for path in pinned}
    sources = compiled_sources(args.revision)
    getters = {}
    for profile in ["V2016", "V2019"]:
        original = args.directory / f"{profile}.accdb"
        getters[profile] = getter_model(args.jar, original)
        for mode in ["stored", "mszip"]:
            package = args.directory / f"{profile}-{mode}.accdc"
            getters[profile][mode] = inspect(package, original.stat().st_size, digest(original))
    records = []
    for path in inputs:
        for case, sql, expected, streaming, batch, collation, extra, error in cases:
            command = [
                args.cli,
                "--format", "tsv",
                "query", "run", sql,
                "--database", path,
                "--native-bytes", "268435456",
                "--native-steps", "10000000",
                "--number-format", "invariant",
                "--collation", collation,
            ]
            if streaming:
                command.extend(["--stream", "--batch-rows", str(batch)])
            command.extend(extra)
            result = invoke(command)
            actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
            if error is None:
                matched = result.returncode == 0 and actual == expected
            else:
                output_matches = actual == expected if expected is not None else result.stdout == ""
                matched = result.returncode != 0 and output_matches and error in result.stderr.lower()
            records.append({
                "case": case,
                "streaming": streaming,
                "comparison_scope": collation,
                "command": [relative(value) if isinstance(value, Path) else value for value in command],
                "expected_rows_or_prefix": expected,
                "expected_error_fragment": error,
                "exit_code": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
                "matched": matched,
            })
    after = {relative(path): digest(path) for path in pinned}
    report = {
        "scope": "Original native/CAB Text GROUP engineering replay; ASCII-only explicit General equality model; no native Access SQL claim",
        "compiled_revision": args.revision,
        "compiled_source_sha256": sources,
        "immutable_sha256_before": before,
        "immutable_sha256_after": after,
        "pins_unchanged": before == after,
        "getters": getters,
        "cli_commands": len(records),
        "success_commands": sum(record["expected_error_fragment"] is None for record in records),
        "failure_controls": sum(record["expected_error_fragment"] is not None for record in records),
        "disagreements": sum(not record["matched"] for record in records),
        "commands": records,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as destination:
        json.dump(report, destination, indent=2)
        destination.write("\n")
    print(json.dumps({key: report[key] for key in [
        "cli_commands", "success_commands", "failure_controls", "disagreements", "pins_unchanged"
    ]}, indent=2))
    if report["disagreements"] or not report["pins_unchanged"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
