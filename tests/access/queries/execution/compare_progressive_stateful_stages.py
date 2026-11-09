"""Replay native/CAB aggregate-stage composition against independent formulas.

Only TableCraft executes SQL. Existing original inputs, inert getters and CAB
inspection establish data provenance; this is not native Access qualification.
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
    "docs/testing/compare_progressive_stateful_stages.py",
    "docs/testing/compare_progressive_derived.py",
    "docs/testing/compare_progressive_grouped.py",
    "docs/testing/compare_large_deployment.py",
    "docs/testing/build_large_deployment.py",
    "docs/testing/LargeDeploymentFixture.java",
]


def groups(key):
    result = []
    for identity in range(1, 1101):
        current = key(identity)
        for retained, members in result:
            if current == retained:
                members.append(identity)
                break
        else:
            result.append((current, [identity]))
    return result


def score(identity):
    return identity * 17 - 9


def controls():
    identities = list(range(1, 1101))
    total = sum(identities)
    total_score = sum(score(identity) for identity in identities)
    mean = Fraction(total, len(identities))
    assert mean == Fraction(identities[0] + identities[-1], 2)
    sql = (
        "SELECT N,S,A,Lo,Hi,VarType(N) AS [NT],VarType(S) AS [ST],VarType(A) AS [AT] "
        "FROM (SELECT Count(*) AS [N],Sum(Score) AS [S],Avg(Id) AS [A],"
        "Min(Id) AS [Lo],Max(Id) AS [Hi] FROM LargeDeployment) Q"
    )
    expected = [["N", "S", "A", "Lo", "Hi", "NT", "ST", "AT"]]
    expected.append([number(value) for value in [1100, total_score, mean, 1, 1100, 20, 20, 5]])
    for streaming, batch in [(False, None), (True, 1), (True, 128)]:
        yield "inner-global-all-kernels", sql, expected, streaming, batch, None

    exact = groups(lambda identity: identity % 17)
    sum_means = sum(Fraction(sum(members), len(members)) for _, members in exact)
    sql = (
        "SELECT Count(*) AS [Groups],Sum(N) AS [Population],Sum(S) AS [Total],"
        "Sum(A) AS [Means],Min(Lo) AS [LowId],Max(Hi) AS [HighId] FROM "
        "(SELECT Id Mod 17 AS [K],Count(*) AS [N],Sum(Score) AS [S],Avg(Id) AS [A],"
        "Min(Id) AS [Lo],Max(Id) AS [Hi] FROM LargeDeployment GROUP BY Id Mod 17) Q"
    )
    expected = [["Groups", "Population", "Total", "Means", "LowId", "HighId"]]
    expected.append([number(value) for value in [17, 1100, total_score, sum_means, 1, 1100]])
    for streaming in [False, True]:
        yield "exact-inner-outer-global", sql, expected, streaming, 128, None

    text_groups = groups(lambda identity: f"row-{identity:06d}"[:8])
    outer = []
    for _, members in text_groups:
        count = len(members)
        entry = (count, Fraction(sum(members), count), min(members), max(members))
        for retained, items in outer:
            if count % 2 == retained:
                items.append(entry)
                break
        else:
            outer.append((count % 2, [entry]))
    sql = (
        "SELECT CLng(N Mod 2) AS [Parity],Count(*) AS [Groups],Sum(N) AS [Population],"
        "Avg(A) AS [MeanOfMeans],Min(Lo) AS [LowId],Max(Hi) AS [HighId] FROM "
        "(SELECT Left(Label,8) AS [K],Count(*) AS [N],Sum(Score) AS [S],Avg(Id) AS [A],"
        "Min(Id) AS [Lo],Max(Id) AS [Hi] FROM LargeDeployment GROUP BY Left(Label,8)) Q "
        "GROUP BY CLng(N Mod 2)"
    )
    expected = [["Parity", "Groups", "Population", "MeanOfMeans", "LowId", "HighId"]]
    for parity, entries in outer:
        mean_of_means = sum(item[1] for item in entries) / len(entries)
        expected.append([number(value) for value in [
            parity, len(entries), sum(item[0] for item in entries), mean_of_means,
            min(item[2] for item in entries), max(item[3] for item in entries)
        ]])
    for streaming in [False, True]:
        yield "text-inner-outer-group", sql, expected, streaming, 128, None

    accepted = [(key, members) for key, members in exact if len(members) == 64]
    sql = (
        "SELECT Count(*) AS [Groups],Sum(N) AS [Population],Sum(S) AS [Total] FROM "
        "(SELECT Id Mod 17 AS [K],Count(*) AS [N],Sum(Score) AS [S] FROM LargeDeployment "
        "GROUP BY Id Mod 17 HAVING Count(*)=64) Q"
    )
    expected = [["Groups", "Population", "Total"], [
        str(len(accepted)), str(sum(len(members) for _, members in accepted)),
        str(sum(score(identity) for _, members in accepted for identity in members)),
    ]]
    yield "inner-having-before-outer-global", sql, expected, True, 128, None

    sql = (
        "SELECT Count(*) AS [Groups],Sum(N) AS [Population],Sum(J) AS [Keys] FROM "
        "(SELECT TOP 4 K,N,CLng(K+1) AS [J] FROM "
        "(SELECT Id Mod 17 AS [K],Count(*) AS [N] FROM LargeDeployment GROUP BY Id Mod 17) I "
        "WHERE N=64) Q"
    )
    first = accepted[:4]
    expected = [["Groups", "Population", "Keys"], [
        str(len(first)), str(sum(len(members) for _, members in first)),
        str(sum(key + 1 for key, _ in first)),
    ]]
    yield "middle-filter-top-before-outer-global", sql, expected, True, 128, None

    sql = (
        "SELECT Count(*) AS [Rows],Sum(G) AS [Groups],Sum(P) AS [Population] FROM "
        "(SELECT N,Count(*) AS [G],Sum(N) AS [P] FROM "
        "(SELECT Id Mod 17 AS [K],Count(*) AS [N] FROM LargeDeployment GROUP BY Id Mod 17) I "
        "GROUP BY N) Q"
    )
    expected = [["Rows", "Groups", "Population"], ["2", "17", "1100"]]
    yield "three-stateful-barriers", sql, expected, True, 128, None

    sql = (
        "SELECT Count(*) AS [N] FROM (SELECT Count(*) AS [N] FROM LargeDeployment WHERE False) Q "
        "UNION ALL SELECT Count(*) FROM "
        "(SELECT Id,Count(*) AS [N] FROM LargeDeployment WHERE False GROUP BY Id) Q"
    )
    yield "empty-global-one-versus-keyed-zero", sql, [["N"], ["1"], ["0"]], True, 128, None

    sql = (
        "SELECT LenB(R) AS [Bytes],AscW(R) AS [Unit],CLng(E) AS [Code],VarType(N) AS [NT],"
        "VarType(C) AS [CT],VarType(D) AS [DT],CLng(C) AS [Money],CLng(D) AS [Mean] FROM "
        "(SELECT Min(ChrW(55296)) AS [R],Min(CVErr(Id)) AS [E],Count(*) AS [N],"
        "Sum(CCur(Score)) AS [C],Avg(CDec(Id)) AS [D] FROM LargeDeployment WHERE Id=1) Q"
    )
    expected = [["Bytes", "Unit", "Code", "NT", "CT", "DT", "Money", "Mean"],
                ["2", "-10240", "1", "20", "6", "14", "8", "1"]]
    yield "retained-raw-error-and-natural-carriers", sql, expected, True, 128, None

    sql = "SELECT Count(*) AS [N] FROM (SELECT Sum(1/0) AS [Bad] FROM LargeDeployment HAVING False) Q"
    yield "inner-having-selected-suppression", sql, [["N"], ["0"]], True, 128, None

    inner = (
        "(SELECT Left(Label,8) AS [K],10\\(Max(Id)-1100) AS [Bad] FROM LargeDeployment "
        "GROUP BY Left(Label,8)) Q"
    )
    sql = f"SELECT TOP 1 K FROM {inner}"
    prefix = [["K"], [json.dumps(text_groups[0][0])]]
    yield "inner-late-error-outer-row-prefix", sql, prefix, True, 1, "division by zero"
    sql = f"SELECT Count(*) AS [N] FROM {inner}"
    yield "inner-late-error-no-outer-global-eof", sql, None, True, 1, "division by zero"
    sql = "SELECT Count(*) FROM (SELECT Count(*) AS [N] FROM LargeDeployment) Q"
    yield "low-query-work", sql, None, True, 128, "resource"


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
        raise ValueError("Original provenance differs")
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
            path = args.directory / f"{profile}-{mode}.accdc"
            getters[profile][mode] = inspect(path, original.stat().st_size, digest(original))
    records = []
    for path in inputs:
        for label, sql, expected, streaming, batch, error in cases:
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
                command.extend(["--stream", "--batch-rows", str(batch)])
            if label == "low-query-work":
                command.extend(["--execution-steps", "1"])
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
        "scope": "Original native/CAB static stateful stage engineering replay; no native Access SQL or timing claims",
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
