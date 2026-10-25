"""Replay static native derived/named-view pipelines against independent formulas.

Existing original ACE files and CAB packages remain unchanged. A named Stage is
an explicitly selected ordinary-column host view, not a native saved definition.
Only TableCraft executes SQL; this is engineering, not native Access evidence.
"""

import argparse
import csv
from fractions import Fraction
import hashlib
import io
import json
import subprocess
from pathlib import Path

from build_large_deployment import digest, inspect
from compare_large_deployment import JAR_SHA256, getter_model
from compare_progressive_grouped import (
    PROVENANCE_HASH,
    invoke,
    number,
    relative,
    source_pins,
)

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 100
TOOLS = [
    "docs/testing/compare_progressive_derived.py",
    "docs/testing/compare_progressive_grouped.py",
    "docs/testing/compare_large_deployment.py",
    "docs/testing/build_large_deployment.py",
    "docs/testing/LargeDeploymentFixture.java",
]
EXTRA_SOURCES = [
    "crates/backends/tablecraft-access/src/source/recomputed/forwarding.rs",
    "crates/backends/tablecraft-access/src/source/external/forwarding.rs",
    "apps/tablecraft-cli/src/commands/query/stream.rs",
]


def linear_groups(members):
    result = []
    for identity in members:
        key = identity % 17
        for retained, population in result:
            if key == retained:
                population.append(identity)
                break
        else:
            result.append((key, [identity]))
    return result


def controls():
    pipeline = (
        "(SELECT TOP 20 Id,CLng(Score+Id) AS V,Label FROM "
        "(SELECT TOP 80 Id,Score,Label FROM LargeDeployment WHERE Id>=5) I "
        "WHERE Id Mod 2=0) Q"
    )
    sql = f"SELECT TOP 5 Id,V,Label FROM {pipeline} WHERE Id>10"
    expected = [["Id", "V", "Label"]]
    expected.extend(
        [str(identity), str(identity * 18 - 9), json.dumps(f"row-{identity:06d}")]
        for identity in [12, 14, 16, 18, 20]
    )
    for streaming, batch in [(False, None), (True, 1), (True, 128)]:
        yield "inline-stage-top", sql, expected, streaming, batch, False, (), None

    sql = (
        "SELECT Id,V,Label FROM (SELECT Id,CLng(Score+Id) AS V,Label "
        "FROM (SELECT TOP 3 Id,Score,Label FROM Stage) I) Q"
    )
    expected = [["Id", "V", "Label"]]
    expected.extend(
        [str(identity), str(identity * 18 - 9), json.dumps(f"row-{identity:06d}")]
        for identity in [1, 2, 3]
    )
    for streaming in [False, True]:
        yield "ordinary-host-named-view", sql, expected, streaming, 128, True, (), None

    members = [identity for identity in range(1, 1101) if identity % 3 == 0]
    scores = [identity * 17 - 9 for identity in members]
    sql = (
        "SELECT Count(*) AS N,Sum(Id) AS S,Avg(V) AS A,Min(V) AS Lo,Max(V) AS Hi "
        "FROM (SELECT Id,CLng(Score) AS V FROM "
        "(SELECT Id,Score FROM LargeDeployment WHERE Id Mod 3=0) I) Q"
    )
    mean = Fraction(sum(scores), len(scores))
    assert mean == Fraction(scores[0] + scores[-1], 2)
    expected = [["N", "S", "A", "Lo", "Hi"]]
    expected.append([number(value) for value in [
        len(members), sum(members), mean, min(scores), max(scores)
    ]])
    for streaming in [False, True]:
        yield "outer-global-aggregate", sql, expected, streaming, 128, False, (), None

    sql = (
        "SELECT K,Count(*) AS N,Sum(Id) AS S,Avg(V) AS A FROM "
        "(SELECT Id,CLng(Id Mod 17) AS K,CLng(Score) AS V FROM "
        "(SELECT Id,Score FROM LargeDeployment WHERE Id Mod 3=0) I) Q GROUP BY K"
    )
    expected = [["K", "N", "S", "A"]]
    for key, population in linear_groups(members):
        values = [identity * 17 - 9 for identity in population]
        mean = Fraction(sum(values), len(values))
        assert mean == Fraction(values[0] + values[-1], 2)
        expected.append([number(value) for value in [key, len(population), sum(population), mean]])
    for streaming in [False, True]:
        yield "outer-grouped-aggregate", sql, expected, streaming, 128, False, (), None

    sql = (
        "SELECT TOP 3 V,VarType(V) AS T FROM "
        "(SELECT CDbl(Id) AS V FROM LargeDeployment WHERE Id<=3) Q "
        "UNION ALL SELECT TOP 2 Id AS V,VarType(Id) AS T FROM Stage WHERE Id>=1099"
    )
    expected = [
        ["V", "T"],
        ["1", "5"],
        ["2", "5"],
        ["3", "5"],
        ["1099", "3"],
        ["1100", "3"],
    ]
    for streaming in [False, True]:
        yield "flat-union-host-view", sql, expected, streaming, 128, True, (), None

    sql = (
        "PARAMETERS P LONG; SELECT TOP 3 Id,V FROM "
        "(SELECT Id,CLng(Score+P) AS V FROM LargeDeployment WHERE Id>=P) Q"
    )
    expected = [["Id", "V"]]
    expected.extend([str(identity), str(identity * 17 - 9 + 1098)] for identity in [1098, 1099, 1100])
    yield (
        "typed-parameter-declared", sql, expected, True, 128, False,
        ("--param", "P=long:1098", "--declared-arithmetic"), None,
    )

    sql = (
        "SELECT Count(*) FROM (SELECT TOP 1 Id,1/(Id-1100) AS Unused "
        "FROM LargeDeployment) Q WHERE False"
    )
    yield "eager-unused-inner-late-error", sql, None, True, 128, False, (), "division by zero"
    sql = "SELECT Count(*) FROM (SELECT Sum(Id) AS N FROM LargeDeployment) Q"
    yield "inner-aggregate-refusal", sql, None, True, 128, False, (), "query feature is not implemented"
    sql = "SELECT Id FROM (SELECT Id FROM LargeDeployment ORDER BY Id) Q"
    yield "inner-order-refusal", sql, None, True, 128, False, (), "query feature is not implemented"
    sql = "SELECT Count(*) FROM (SELECT Id FROM LargeDeployment) Q"
    yield "query-work-refusal", sql, None, True, 128, False, ("--execution-steps", "1"), "resource"


def compiled_sources(revision):
    sources = source_pins(revision)
    for name in EXTRA_SOURCES:
        content = subprocess.run(
            ["git", "show", f"{revision}:{name}"],
            cwd=ROOT,
            capture_output=True,
            check=True,
            timeout=120,
        ).stdout
        sources[name] = hashlib.sha256(content).hexdigest()
    return sources


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
        raise FileExistsError("Refusing to overwrite an existing report")
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
    if len(inputs) * len(cases) > MAX_COMMANDS:
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
        for label, sql, expected, streaming, batch, named, extra, error in cases:
            command = [
                args.cli,
                "--format", "tsv",
                "query", "run", sql,
                "--database", path,
                "--native-bytes", "268435456",
                "--native-steps", "10000000",
                "--number-format", "invariant",
            ]
            if named:
                for column in ["Id", "Score", "Label"]:
                    command.extend(["--recomputed-column", "Stage", "LargeDeployment", column])
            if streaming:
                command.extend(["--stream", "--batch-rows", str(batch)])
            command.extend(extra)
            result = invoke(command)
            actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
            if error is None:
                matched = result.returncode == 0 and actual == expected
            else:
                matched = result.returncode != 0 and not result.stdout and error in result.stderr.lower()
            records.append({
                "case": label,
                "streaming": streaming,
                "ordinary_host_named_view": named,
                "command": [relative(value) if isinstance(value, Path) else value for value in command],
                "expected_rows": expected,
                "expected_error_fragment": error,
                "exit_code": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
                "matched": matched,
            })
    after = {relative(path): digest(path) for path in pinned}
    report = {
        "scope": "Original static derived and ordinary-column named host view engineering replay; no native Access SQL claims",
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
