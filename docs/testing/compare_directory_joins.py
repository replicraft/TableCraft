"""Check native exact-key directory joins against independent integer models.

Original Jackcess getters validate immutable inputs; only TableCraft executes SQL.
No fixture generation, native Access SQL oracle, or timing claims are involved.
"""

import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

import compare_native_equijoins as borrowed
from compare_large_deployment import getter_model

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 100
LARGE_HASHES = {
    "V2016.accdb": "51b4a6bf4477b2c1dca061e95179b5ccd70de158623edeafc96c5e38186bf630",
    "V2019.accdb": "cbb12a433c622b6d2a91e72d627b1cfab0eb6192953eaca7510c8d97668cc4f7",
}
TOOLS = [
    "docs/testing/compare_directory_joins.py",
    "docs/testing/compare_native_equijoins.py",
    "docs/testing/compare_large_deployment.py",
    "docs/testing/build_large_deployment.py",
    "docs/testing/TableDataOracle.java",
    "docs/testing/DecimalIndexFixture.java",
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


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def compiled_sources(revision):
    paths = [*borrowed.SOURCE_ROOTS, "crates/query/tablecraft-query/src/execution/exact_keys"]
    listing = invoke(["git", "ls-tree", "-r", "--name-only", revision, "--", *paths])
    listing.check_returncode()
    pins = {}
    for name in listing.stdout.splitlines():
        if name.endswith(".rs"):
            content = subprocess.run(
                ["git", "show", f"{revision}:{name}"],
                cwd=ROOT,
                capture_output=True,
                check=True,
                timeout=120,
            ).stdout
            pins[name] = hashlib.sha256(content).hexdigest()
    if not pins:
        raise ValueError("Missing compiled source pins")
    return pins


def joined(left, right, kind):
    """Preserve existing left-major matches, then unmatched right encounter order."""
    result = []
    matched_right = set()
    for a in left:
        matches = []
        for index, b in enumerate(right):
            if a == b:
                matches.append((a, b))
                matched_right.add(index)
        if matches:
            result.extend(matches)
        elif kind == "LEFT":
            result.append((a, -1))
    if kind == "RIGHT":
        result.extend((-1, b) for index, b in enumerate(right) if index not in matched_right)
    return result


def pair_cases(table, values):
    for kind in ["INNER", "LEFT", "RIGHT"]:
        for reversed_operands in [False, True]:
            equality = "B.ID=A.ID" if reversed_operands else "A.ID=B.ID"
            sql = (
                f"SELECT Nz(A.ID,-1) AS L,Nz(B.ID,-1) AS R FROM {table} A "
                f"{kind} JOIN {table} B ON {equality}"
            )
            yield f"{kind.lower()}-reversed-{reversed_operands}", sql, joined(values, values, kind)

    right = [value for value in values if value < 3]
    sql = (
        f"SELECT A.ID AS L,Nz(B.ID,-1) AS R FROM {table} A LEFT JOIN "
        f"(SELECT ID FROM {table} WHERE ID<3) B ON A.ID=B.ID"
    )
    yield "filtered-right-unmatched", sql, joined(values, right, "LEFT")
    sql = (
        f"SELECT Nz(A.ID,-1) AS L,B.ID AS R FROM "
        f"(SELECT ID FROM {table} WHERE ID<3) A RIGHT JOIN {table} B ON B.ID=A.ID"
    )
    yield "filtered-left-unmatched", sql, joined(right, values, "RIGHT")

    subset = [value for value in values if value < 3]
    left = f"(SELECT ID FROM {table} WHERE ID<3 UNION ALL SELECT ID FROM {table} WHERE ID<3)"
    sql = (
        f"SELECT A.ID AS L,B.ID AS R FROM {left} A INNER JOIN "
        f"(SELECT ID FROM {table} WHERE ID<3) B ON A.ID=B.ID"
    )
    yield "small-derived-duplicates", sql, joined(subset + subset, subset, "INNER")


def run_case(cli, database, label, sql, expected, extra=(), error=None):
    command = [
        cli, "--format", "tsv", "query", "run", sql,
        "--database", database,
        "--native-bytes", "268435456",
        "--native-steps", "10000000",
        "--native-rows", "5000",
        "--execution-rows", "10000",
        "--execution-bytes", "268435456",
        *extra,
    ]
    result = invoke(command)
    actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
    if error is None:
        matched = result.returncode == 0 and actual == expected
    else:
        matched = result.returncode != 0 and not result.stdout and error in result.stderr.lower()
    return {
        "case": label,
        "command": [relative(item) if isinstance(item, Path) else item for item in command],
        "expected_rows": expected,
        "expected_error_fragment": error,
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "matched": matched,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--decimal-inputs", type=Path, nargs=4, required=True)
    parser.add_argument("--large-inputs", type=Path, nargs=2, required=True)
    parser.add_argument("--baseline-cli", type=Path)
    parser.add_argument("--baseline-cli-sha256")
    parser.add_argument("--baseline-revision")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    relative(args.report)
    if args.report.exists():
        raise FileExistsError("Refusing to overwrite report")
    if digest(args.cli) != args.cli_sha256 or digest(args.jar) != borrowed.JAR_SHA256:
        raise ValueError("Frozen executable or reviewed reader differs")
    if args.baseline_cli and not args.baseline_revision:
        raise ValueError("Baseline executable requires its compiled revision")
    if args.baseline_cli and digest(args.baseline_cli) != args.baseline_cli_sha256:
        raise ValueError("Baseline executable hash mismatch")
    if {path.name for path in args.decimal_inputs} != set(borrowed.INPUT_SHA256):
        raise ValueError("Supply exactly the four registered Decimal originals")
    if {path.name for path in args.large_inputs} != set(LARGE_HASHES):
        raise ValueError("Supply exactly both registered large originals")
    for path in args.decimal_inputs:
        if digest(path) != borrowed.INPUT_SHA256[path.name]:
            raise ValueError("Original Decimal bytes changed")
    for path in args.large_inputs:
        if digest(path) != LARGE_HASHES[path.name]:
            raise ValueError("Original large database bytes changed")

    pinned = [args.cli, args.jar, *args.decimal_inputs, *args.large_inputs]
    pinned.extend(ROOT / name for name in TOOLS)
    if args.baseline_cli:
        pinned.append(args.baseline_cli)
    before = {relative(path): digest(path) for path in pinned}
    sources = compiled_sources(args.revision)
    getters = {}
    records = []
    planned_commands = 6 * 12 + 2 + (2 if args.baseline_cli else 0)
    if planned_commands > MAX_COMMANDS:
        raise ValueError("Command ceiling exceeded before replay")
    for database in [*args.decimal_inputs, *args.large_inputs]:
        large = database in args.large_inputs
        if large:
            getters[relative(database)] = getter_model(args.jar, database)
            table = "LargeDeployment"
            values = list(range(1, 1101))
        else:
            identities, observation = borrowed.getter_ids(args.jar, database)
            getters[relative(database)] = observation
            table = "Decimal4"
            values = identities[table]
        for label, sql, pairs in pair_cases(table, values):
            expected = [["L", "R"], *[[str(a), str(b)] for a, b in pairs]]
            records.append(run_case(args.cli, database, label, sql, expected))

        fallback = f"SELECT A.ID AS L,B.ID AS R FROM {table} A INNER JOIN {table} B ON A.ID+0=B.ID"
        records.append(run_case(
            args.cli, database, "expression-fallback-work-refusal", fallback, None,
            extra=("--execution-steps", "10000"), error="resource",
        ))
        subset = [value for value in values if value < 3]
        mixed = (
            f"SELECT A.ID AS L,B.ID AS R FROM "
            f"(SELECT ID FROM {table} WHERE ID<3) A INNER JOIN "
            f"(SELECT ID FROM {table} WHERE ID<3) B ON CDbl(A.ID)=B.ID"
        )
        expected = [["L", "R"]]
        expected.extend([str(a), str(b)] for a, b in joined(subset, subset, "INNER"))
        records.append(run_case(args.cli, database, "mixed-carrier-small-fallback", mixed, expected))
        eager = f"SELECT A.ID AS L FROM {table} A INNER JOIN {table} B ON A.ID=B.ID AND 1/0=0"
        records.append(run_case(args.cli, database, "eager-on-failure", eager, None, error="division by zero"))
        if large:
            sql = (
                "SELECT A.ID AS L,B.Score AS S,B.Label AS T FROM LargeDeployment A "
                "INNER JOIN LargeDeployment B ON A.ID=B.ID"
            )
            expected = [["L", "S", "T"]]
            expected.extend(
                [str(value), str(value * 17 - 9), json.dumps(f"row-{value:06d}")]
                for value in values
            )
            records.append(run_case(args.cli, database, "native-1100-label-score", sql, expected))
            if args.baseline_cli:
                records.append(run_case(
                    args.baseline_cli, database, "baseline-1100-work-refusal", sql, None,
                    error="resource",
                ))
    if len(records) > MAX_COMMANDS:
        raise ValueError("Command ceiling exceeded")
    after = {relative(path): digest(path) for path in pinned}
    report = {
        "scope": "Original native getter and independent integer-directory join evidence; no timing/native Access SQL claim",
        "compiled_revision": args.revision,
        "compiled_revision_source_sha256": sources,
        "baseline_revision": args.baseline_revision,
        "baseline_revision_source_sha256": compiled_sources(args.baseline_revision) if args.baseline_cli else {},
        "immutable_sha256_before": before,
        "immutable_sha256_after": after,
        "pins_unchanged": before == after,
        "getters": getters,
        "cli_commands": len(records),
        "success_commands": sum(record["expected_error_fragment"] is None for record in records),
        "refusal_commands": sum(record["expected_error_fragment"] is not None for record in records),
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
