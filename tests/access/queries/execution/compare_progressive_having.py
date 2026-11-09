"""Replay original native/CAB HAVING queries against independent group models.

Selected projection suppression and progressive prefixes have explicit controls.
No fixture is generated, native Access SQL executed, or production oracle used.
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
    "docs/testing/compare_progressive_having.py",
    "docs/testing/compare_progressive_derived.py",
    "docs/testing/compare_progressive_grouped.py",
    "docs/testing/compare_large_deployment.py",
    "docs/testing/build_large_deployment.py",
    "docs/testing/LargeDeploymentFixture.java",
]


def populations(limit=1100):
    groups = []
    for identity in range(1, limit + 1):
        key = identity % 17
        for previous, members in groups:
            if previous == key:
                members.append(identity)
                break
        else:
            groups.append((key, [identity]))
    return groups


def controls():
    groups = populations()
    sql = (
        "SELECT Id Mod 17 AS K,Count(*) AS N FROM LargeDeployment "
        "GROUP BY Id Mod 17 HAVING Count(*)>=65"
    )
    expected = [["K", "N"]]
    expected.extend([str(key), str(len(members))] for key, members in groups if len(members) >= 65)
    for streaming, batch in [(False, None), (True, 1), (True, 128)]:
        yield "common-group-having", sql, expected, streaming, batch, (), None

    sql = (
        "SELECT CLng(7) AS V FROM LargeDeployment HAVING Count(*)=1100 "
        "AND Sum(Id)=605550 AND Avg(Id)=550.5 AND Min(Id)=1 AND Max(Id)=1100"
    )
    yield "only-having-five-aggregates", sql, [["V"], ["7"]], True, 128, (), None
    sql = "SELECT Count(*) AS N FROM LargeDeployment WHERE False HAVING Count(*)=0"
    yield "empty-global-having", sql, [["N"], ["0"]], True, 128, (), None

    sql = (
        "SELECT Id Mod 17 AS K,Sum(Score) AS S,Avg(Id) AS A FROM LargeDeployment "
        "GROUP BY Id Mod 17 HAVING Avg(Score)>9400 AND Count(*)>=64"
    )
    expected = [["K", "S", "A"]]
    for key, members in groups:
        scores = [identity * 17 - 9 for identity in members]
        mean = Fraction(sum(scores), len(scores))
        assert mean == Fraction(scores[0] + scores[-1], 2)
        if mean > 9400:
            expected.append([str(key), str(sum(scores)), number(Fraction(sum(members), len(members)))])
    for streaming in [False, True]:
        yield "exact-mean-having", sql, expected, streaming, 128, (), None

    for predicate in ["False", "NULL"]:
        sql = f"SELECT Sum(1/0) AS Bad FROM LargeDeployment HAVING {predicate}"
        yield "suppressed-projection-" + predicate, sql, [["Bad"]], True, 128, (), None

    sql = (
        "SELECT TOP 3 Id Mod 17 AS K,Sum(Score) AS S FROM LargeDeployment "
        "GROUP BY Id Mod 17 HAVING Count(*)=64"
    )
    expected = [["K", "S"]]
    accepted = [(key, members) for key, members in groups if len(members) == 64]
    for key, members in accepted[:3]:
        expected.append([str(key), str(sum(identity * 17 - 9 for identity in members))])
    for streaming in [False, True]:
        yield "top-counts-accepted-groups", sql, expected, streaming, 128, (), None

    sql = (
        "SELECT K,Count(*) AS N FROM (SELECT CLng(Id Mod 17) AS K,Id,Score "
        "FROM LargeDeployment WHERE Id<=40) Q GROUP BY K HAVING Count(*)>2"
    )
    expected = [["K", "N"]]
    expected.extend([str(key), str(len(members))] for key, members in populations(40) if len(members) > 2)
    yield "derived-input-having", sql, expected, True, 128, ("--declared-arithmetic",), None

    sql = (
        "SELECT TOP 1 Id Mod 17 AS K,1/((Id Mod 17)-14) AS V FROM LargeDeployment "
        "GROUP BY Id Mod 17 HAVING (Id Mod 17)>=13"
    )
    # Group13 passes first. Group14 also passes and must still evaluate its
    # failing projection despite TOP having already published one accepted row.
    prefix = [["K", "V"], ["13", "-1"]]
    yield "later-accepted-top-error", sql, prefix, True, 1, (), "division by zero"
    sql = "SELECT Count(*) AS N FROM LargeDeployment HAVING False AND Count(1/0)>0"
    yield "eager-having-operand-error", sql, None, True, 128, (), "division by zero"
    sql = "SELECT Count(*) FROM LargeDeployment HAVING Last(Id)>0"
    yield "unsupported-having-aggregate", sql, None, True, 128, (), "query feature is not implemented"
    sql = "SELECT Count(*) AS N FROM LargeDeployment HAVING Count(*)>0"
    yield "low-query-work", sql, None, True, 128, ("--execution-steps", "1"), "resource"


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
        raise ValueError("Registered provenance differs")
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
            path = args.directory / f"{profile}-{mode}.accdc"
            getters[profile][mode] = inspect(path, original.stat().st_size, digest(original))

    records = []
    for path in inputs:
        for label, sql, expected, streaming, batch, extra, error in cases:
            command = [
                args.cli,
                "--format", "tsv",
                "query", "run", sql,
                "--database", path,
                "--native-bytes", "268435456",
                "--native-steps", "10000000",
                "--number-format", "invariant",
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
                "case": label,
                "streaming": streaming,
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
        "scope": "Original native/CAB HAVING engineering replay; no native Access SQL or timing claims",
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
