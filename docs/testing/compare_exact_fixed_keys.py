"""Replay native fixed relational keys with independent scaled/Fraction models.

Original native/CAB inputs stay unchanged. TableCraft alone executes SQL; this is
engineering key equality/resource evidence, not native Access SQL qualification.
"""

import argparse
import csv
from decimal import Decimal as PythonDecimal
from fractions import Fraction
import hashlib
import io
import json
from pathlib import Path
import subprocess

from build_large_deployment import digest, inspect
from compare_large_deployment import JAR_SHA256, getter_model
from compare_progressive_derived import compiled_sources
from compare_progressive_grouped import PROVENANCE_HASH, invoke, relative

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 100
BASELINE_SHA = "f6a86b005512d52520c9398853843be1d3e47d19b34f1758e09b0f6f40f1a89c"
BASELINE_REVISION = "e28fd42cf33a64489106d9514fbc227b8ae1f9e4"
TOOLS = [
    "docs/testing/compare_exact_fixed_keys.py",
    "docs/testing/compare_progressive_derived.py",
    "docs/testing/compare_progressive_grouped.py",
    "docs/testing/compare_large_deployment.py",
    "docs/testing/build_large_deployment.py",
    "docs/testing/LargeDeploymentFixture.java",
]


def groups(values):
    result = []
    for key, identity in values:
        for previous, members in result:
            if key == previous:
                members.append(identity)
                break
        else:
            result.append((key, [identity]))
    return result


def spelling(key, currency):
    if key is None:
        return "NULL"
    if currency:
        scaled = key * 10000
        assert scaled.denominator == 1
        return f"{scaled.numerator // 10000}.{scaled.numerator % 10000:04d}"
    return str(PythonDecimal(key.numerator) / PythonDecimal(key.denominator))


def values():
    return [(None if identity % 13 == 0 else Fraction(identity % 17, 10), identity)
            for identity in range(1, 1101)]


def unique_sql(currency):
    conversion = "CCur" if currency else "CDec"
    return (
        f"SELECT K,Count(*) AS N,VarType(K) AS VT FROM "
        f"(SELECT {conversion}(Id) AS K FROM LargeDeployment) Q GROUP BY K"
    )


def unique_rows(currency):
    kind = "6" if currency else "14"
    return [["K", "N", "VT"], *[
        [spelling(Fraction(identity), currency), "1", kind]
        for identity in range(1, 1101)
    ]]


def controls():
    population = groups(values())
    for currency in [True, False]:
        name = "currency" if currency else "decimal"
        kind = "6" if currency else "14"
        expression = "CCur((Id Mod 17)/10)" if currency else "CDec(Id Mod 17)/CDec(10)"
        input_sql = (
            f"(SELECT Id,IIf(Id Mod 13=0,NULL,{expression}) AS K "
            "FROM LargeDeployment) Q"
        )
        sql = f"SELECT K,Count(*) AS N,Sum(Id) AS S,VarType(K) AS VT FROM {input_sql} GROUP BY K"
        expected = [["K", "N", "S", "VT"]]
        for key, members in population:
            expected.append([spelling(key, currency), str(len(members)), str(sum(members)),
                             "1" if key is None else kind])
        for streaming in [False, True]:
            yield name + "-group", sql, expected, streaming

        sql = f"SELECT DISTINCT K FROM {input_sql}"
        expected = [["K"], *[[spelling(key, currency)] for key, _ in population]]
        yield name + "-distinct", sql, expected, False

        sql = f"SELECT K FROM {input_sql} WHERE Id<=17 UNION SELECT K FROM {input_sql} WHERE Id>=1080"
        selected = [(key, identity) for key, identity in values() if identity <= 17]
        selected.extend((key, identity) for key, identity in values() if identity >= 1080)
        expected = [["K"], *[[spelling(key, currency)] for key, _ in groups(selected)]]
        yield name + "-union-prefix", sql, expected, False

        sql = f"SELECT K,Count(*) AS N FROM {input_sql} GROUP BY K HAVING Count(*)>=65"
        expected = [["K", "N"]]
        expected.extend([spelling(key, currency), str(len(members))]
                        for key, members in population if len(members) >= 65)
        yield name + "-group-having", sql, expected, True
        yield name + "-unique-1100", unique_sql(currency), unique_rows(currency), False

    input_sql = (
        "(SELECT IIf(Id Mod 2=0,CDec('1.00'),CDec('1.0')) AS K "
        "FROM LargeDeployment) Q"
    )
    yield "decimal-scale-distinct-first", f"SELECT DISTINCT K FROM {input_sql}", [["K"], ["1.0"]], False
    yield (
        "decimal-scale-group-first",
        f"SELECT K,Count(*) AS N FROM {input_sql} GROUP BY K",
        [["K", "N"], ["1.0", "1100"]],
        True,
    )


def key_sources(revision):
    sources = compiled_sources(revision)
    listing = invoke([
        "git", "ls-tree", "-r", "--name-only", revision, "--",
        "crates/query/tablecraft-query/src/execution/exact_keys",
    ])
    listing.check_returncode()
    for name in listing.stdout.splitlines():
        if name.endswith(".rs"):
            content = subprocess.run(
                ["git", "show", f"{revision}:{name}"],
                cwd=ROOT,
                capture_output=True,
                check=True,
                timeout=120,
            ).stdout
            sources[name] = hashlib.sha256(content).hexdigest()
    return sources


def rows_match(label, actual, expected):
    quotient_keys = {"decimal-group", "decimal-distinct", "decimal-union-prefix"}
    if label not in quotient_keys:
        return actual == expected
    if not actual or actual[0] != expected[0] or len(actual) != len(expected):
        return False
    for observed, wanted in zip(actual[1:], expected[1:]):
        if len(observed) != len(wanted) or observed[1:] != wanted[1:]:
            return False
        if observed[0] == "NULL" or wanted[0] == "NULL":
            if observed[0] != wanted[0]:
                return False
        elif Fraction(PythonDecimal(observed[0])) != Fraction(PythonDecimal(wanted[0])):
            return False
    return True


def representation_probe(cli, label):
    sql = (
        "SELECT CDec(1)/CDec(10) AS A,CDec(2)/CDec(10) AS B,"
        "CDec(10)/CDec(10) AS C,CDec(13)/CDec(10) AS D"
    )
    command = [cli, "--format", "tsv", "query", "run", sql]
    result = invoke(command)
    actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
    expected = [["A", "B", "C", "D"], ["0.10", "0.20", "1", "1.30"]]
    return {
        "case": label,
        "command": [relative(value) if isinstance(value, Path) else value for value in command],
        "expected_rows": expected,
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "matched": result.returncode == 0 and actual == expected,
    }


def run_case(cli, path, label, sql, expected, streaming, extra=(), error=None):
    command = [
        cli,
        "--format", "tsv",
        "query", "run", sql,
        "--database", path,
        "--native-bytes", "268435456",
        "--native-steps", "10000000",
        "--number-format", "invariant",
    ]
    if streaming:
        command.extend(["--stream", "--batch-rows", "128"])
    command.extend(extra)
    result = invoke(command)
    actual = list(csv.reader(io.StringIO(result.stdout), delimiter="\t"))
    if error is None:
        matched = result.returncode == 0 and rows_match(label, actual, expected)
    else:
        matched = result.returncode != 0 and not result.stdout and error in result.stderr.lower()
    return {
        "case": label,
        "command": [relative(value) if isinstance(value, Path) else value for value in command],
        "expected_rows": expected,
        "expected_error_fragment": error,
        "comparison": (
            "Decimal quotient key column0: exact Fraction; header and other cells: exact wire"
            if label in {"decimal-group", "decimal-distinct", "decimal-union-prefix"}
            else "Exact wire cells"
        ),
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
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--baseline-cli", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    relative(args.report)
    if args.report.exists():
        raise FileExistsError("Refusing to overwrite report")
    if digest(args.cli) != args.cli_sha256 or digest(args.jar) != JAR_SHA256:
        raise ValueError("Frozen executable or reviewed reader differs")
    if digest(args.provenance) != PROVENANCE_HASH:
        raise ValueError("Original provenance differs")
    if args.baseline_cli and digest(args.baseline_cli) != BASELINE_SHA:
        raise ValueError("Frozen old HAVING executable differs")
    provenance = json.loads(args.provenance.read_text())
    inputs = [args.directory / name for name in provenance["files"]]
    for path in inputs:
        if digest(path) != provenance["files"][path.name]["sha256"]:
            raise ValueError("Registered original input differs")
    cases = list(controls())
    count = len(inputs) * len(cases) + (6 if args.baseline_cli else 1)
    if count > MAX_COMMANDS:
        raise ValueError("Replay exceeds explicit command ceiling")
    pinned = [args.cli, args.jar, args.provenance, *inputs]
    pinned.extend(ROOT / name for name in TOOLS)
    if args.baseline_cli:
        pinned.append(args.baseline_cli)
    before = {relative(path): digest(path) for path in pinned}
    sources = key_sources(args.revision)
    getters = {}
    for profile in ["V2016", "V2019"]:
        original = args.directory / f"{profile}.accdb"
        getters[profile] = getter_model(args.jar, original)
        for mode in ["stored", "mszip"]:
            path = args.directory / f"{profile}-{mode}.accdc"
            getters[profile][mode] = inspect(path, original.stat().st_size, digest(original))
    records = []
    for path in inputs:
        for label, sql, expected, streaming in cases:
            records.append(run_case(args.cli, path, label, sql, expected, streaming))
    baseline = []
    if args.baseline_cli:
        for profile in ["V2016", "V2019"]:
            path = args.directory / f"{profile}.accdb"
            sql = unique_sql(False)
            baseline.append(run_case(
                args.baseline_cli, path, "old-default-work", sql, None, False, error="steps resource",
            ))
            baseline.append(run_case(
                args.baseline_cli, path, "old-explicit-work", sql, unique_rows(False), False,
                extra=("--execution-steps", "10000000"),
            ))
    representation = [representation_probe(args.cli, "new-existing-division-representation")]
    if args.baseline_cli:
        representation.append(representation_probe(args.baseline_cli, "old-existing-division-representation"))
    after = {relative(path): digest(path) for path in pinned}
    report = {
        "scope": "Original exact fixed key engineering replay; no native Access SQL or timing claims",
        "compiled_revision": args.revision,
        "compiled_source_sha256": sources,
        "baseline_revision": BASELINE_REVISION if args.baseline_cli else None,
        "baseline_source_sha256": key_sources(BASELINE_REVISION) if args.baseline_cli else {},
        "immutable_sha256_before": before,
        "immutable_sha256_after": after,
        "pins_unchanged": before == after,
        "getters": getters,
        "cli_commands": len(records),
        "baseline_commands": len(baseline),
        "representation_commands": len(representation),
        "comparison_contract": (
            "Only Decimal quotient key column0 in decimal-group/distinct/union-prefix "
            "uses exact Fraction(PythonDecimal). All other cells/controls remain exact wire; "
            "explicit first-scale tests remain strict. Original numerical models/SQL unchanged."
        ),
        "disagreements": sum(not record["matched"] for record in [*records, *baseline, *representation]),
        "commands": records,
        "baseline": baseline,
        "representation_probes": representation,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    with args.report.open("x") as destination:
        json.dump(report, destination, indent=2)
        destination.write("\n")
    print(json.dumps({key: report[key] for key in [
        "cli_commands", "baseline_commands", "representation_commands", "disagreements", "pins_unchanged"
    ]}, indent=2))
    if report["disagreements"] or not report["pins_unchanged"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
