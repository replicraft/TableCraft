"""Independent source-free Long declaration and retained array-bound replay.

No reference runtime or fixture is executed. Public Long signatures and exact
signed32 integer arithmetic distinguish scalar overflow from Variant widening.
"""

import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]
BASELINE_SHA256 = "747425183d2e0869646fa359f3f557a540bff6fb835116a8912e6346a38c4cf3"
LONG_MAX = (1 << 31) - 1
LONG_MIN = -(1 << 31)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def signature_matrix():
    expressions = []
    expected = []
    for length in range(4):
        elements = ",".join(str(index) for index in range(length))
        for function in ("LBound", "UBound"):
            value = 0 if function == "LBound" else length - 1
            for dimension in ("", ",", ",1"):
                expression = function + "(Array(" + elements + ")" + dimension + ")"
                expressions.extend((expression, "VarType(" + expression + ")"))
                expected.extend((str(value), "3"))
    columns = ["C" + str(index) for index in range(len(expressions))]
    sql = "SELECT " + ",".join(expression + " AS " + column for expression, column in zip(expressions, columns))
    return sql, [columns, expected]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "baseline", "source_manifest", "report"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.baseline) == BASELINE_SHA256
    assert digest(args.source_manifest) == args.manifest_sha256
    paths = {args.cli, args.baseline, args.source_manifest, Path(__file__), ROOT / "Cargo.lock"}
    for line in args.source_manifest.read_text().splitlines():
        name, expected = line.split("\0")
        path = ROOT / name
        assert digest(path) == expected, name
        paths.add(path)
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path) for path in sorted(paths)}
    observations = []
    model_cells = {"candidate": 0, "baseline": 0}

    def execute(role, label, sql, expected=None, refusal=None, declared=False, extra=()):
        binary = args.cli if role == "candidate" else args.baseline
        command = [str(binary), "--format", "tsv", "query", "run", "--number-format", "invariant"]
        if declared:
            command.append("--declared-arithmetic")
        command.extend(extra)
        command.append(sql)
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        if refusal:
            assert result.returncode != 0 and not result.stdout, (label, result.stdout, result.stderr)
            assert refusal in result.stderr, (label, refusal, result.stderr)
        else:
            assert result.returncode == 0, (label, result.stderr)
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            assert actual == expected, (label, actual, expected)
            model_cells[role] += sum(len(row) for row in expected[1:])
        observations.append({"role": role, "label": label, "command": command, "exit": result.returncode,
                             "stdout": result.stdout, "stderr": result.stderr, "expected_refusal": refusal})

    matrix, answers = signature_matrix()
    for role in ("candidate", "baseline"):
        for declared in (False, True):
            execute(role, "signature-matrix:" + str(declared), matrix, answers, declared=declared)
        for function in ("LBound", "UBound"):
            value = 0 if function == "LBound" else 1
            execute(role, "rounded-dimension:" + function,
                    "SELECT " + function + "(VBA.Array(1,2),1.4) AS V", [["V"], [str(value)]], declared=True)

    contexts = (
        ("upper-direct", "UBound(Array(1,2))", "+", LONG_MAX, 1, ""),
        ("lower-direct", "LBound(Array(1,2))", "-", LONG_MIN, 0, ""),
        ("empty-upper", "UBound(Array())", "+", LONG_MIN, -1, ""),
        ("derived", "N", "+", LONG_MAX, 1, " FROM (SELECT UBound(Array(1,2)) AS N) Q"),
        ("scalar-subquery", "(SELECT UBound(Array(1,2)))", "+", LONG_MAX, 1, ""),
        ("correlated", "(SELECT UBound(Array(Q.X,NULL)))", "+", LONG_MAX, 1, " FROM (SELECT 1 AS X) Q"),
    )
    for name, bound, operator, peer, value, source in contexts:
        integer = value + peer if operator == "+" else value - peer
        assert integer < LONG_MIN or integer > LONG_MAX
        assert int(float(integer)) == integer, "All selected widened results are exact binary64 integers"
        for mode in ("default", "scalar", "variant"):
            base = "CVar(" + bound + ")" if mode == "variant" else bound
            expression = base + operator + "CLng(" + str(peer) + ")"
            sql = "SELECT " + expression + " AS V,VarType(" + expression + ") AS T" + source
            for role in ("candidate", "baseline"):
                if mode == "scalar":
                    error = "numeric overflow" if role == "candidate" else "declaration is unknown"
                    execute(role, name + ":" + mode, sql, refusal=error, declared=True)
                else:
                    execute(role, name + ":" + mode, sql, [["V", "T"], [str(integer), "5"]],
                            declared=mode == "variant")

    guards = (
        "SELECT N+CLng(2147483647) FROM (SELECT UBound(Array(1,2)) AS N "
        "UNION ALL SELECT UBound(Array(1,2)) WHERE False) Q",
        "SELECT First(N)+CLng(2147483647) FROM (SELECT UBound(Array(1,2)) AS N) Q",
        "SELECT Min(N)+CLng(2147483647) FROM (SELECT UBound(Array(1,2)) AS N) Q",
    )
    errors = (
        ("dimension-zero", "LBound(Array(1),0)", "invalid array bounds"),
        ("dimension-two", "UBound(Array(1),2)", "invalid array bounds"),
        ("dimension-true", "LBound(Array(1),True)", "invalid array bounds"),
        ("rounded-dimension-two", "UBound(Array(1),1.5)", "invalid array bounds"),
        ("null-dimension", "LBound(Array(1),NULL)", "invalid use of NULL"),
        ("bad-dimension", "UBound(Array(1),'bad')", "type mismatch"),
        ("null-input", "UBound(NULL)", "invalid use of NULL"),
        ("scalar-input", "LBound(1)", "type mismatch"),
        ("eager-array-element", "UBound(Array(1/0))", "division by zero"),
        ("dimension-before-input", "UBound(NULL,1/0)", "division by zero"),
        ("dimension-overflow", "LBound(Array(1),1e100)", "numeric overflow"),
    )
    for role in ("candidate", "baseline"):
        for position, sql in enumerate(guards):
            execute(role, "unknown-category:" + str(position), sql, refusal="declaration is unknown", declared=True)
        execute(role, "empty-scalar-subquery",
                "SELECT (SELECT UBound(Array(1,2)) WHERE False)+CLng(2147483647) AS V",
                [["V"], ["NULL"]], declared=True)
        for label, expression, error in errors:
            execute(role, label, "SELECT " + expression + " AS V", refusal=error, declared=True)
    for option in ("--execution-bytes", "--execution-steps", "--output-bytes"):
        message = "output limit" if option == "--output-bytes" else "resource limit"
        execute("candidate", "positive-limit:" + option, "SELECT UBound(Array(1,2)) AS V",
                refusal=message, declared=True, extra=(option, "1"))

    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    pairs = {}
    for observation in observations:
        pairs.setdefault(observation["label"], {})[observation["role"]] = observation
    paired = [pair for pair in pairs.values() if "candidate" in pair and "baseline" in pair]
    differences = sum(
        (pair["candidate"]["exit"], pair["candidate"]["stdout"], pair["candidate"]["stderr"])
        != (pair["baseline"]["exit"], pair["baseline"]["stdout"], pair["baseline"]["stderr"])
        for pair in paired
    )
    report = {"scope": "Source-free Long declarations and retained engineering array profile; "
                       "no native Access/VBA runtime qualification or query-availability expansion.",
              "controls": len(observations), "modeled_cells": model_cells,
              "baseline_pairs": len(paired), "changed_pairs": differences,
              "hashes": hashes, "observations": observations}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "hashes": len(hashes), "report": str(args.report)}))


if __name__ == "__main__":
    main()
