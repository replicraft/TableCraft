"""Independently model explicit predicate previews from inert ordinary getters.

Native caches remain stale data. No Microsoft Access formula execution,
freshness, cache replacement, write or broader formula profile is qualified.
"""

import argparse
import hashlib
import json
import operator
from pathlib import Path
import subprocess

from cli_tsv import display_tsv
from compare_calculated_decimals import digest, JACKCESS_SHA256

ROOT = Path(__file__).resolve().parents[2]
RECIPES = ROOT / "docs/testing"
FORMULAS = {
    "Equal": "[Input]=0",
    "NotEqual": "[Input]<>0",
    "Less": "[Input]<0",
    "LessEqual": "[Input]<=0",
    "Greater": "[Input]>0",
    "GreaterEqual": "[Input]>=0",
    "Negated": "Not ([Input]=0)",
    "Conjunction": "([Input]>-1) And ([Input]<2)",
    "Disjunction": "([Input]<0) Or ([Input]>1)",
    "Exclusive": "([Input]>=0) Xor ([Input]<=0)",
    "Equivalent": "([Input]>=0) Eqv ([Input]<=0)",
    "Implies": "([Input]<0) Imp ([Input]>0)",
    "Missing": "[Input] Is Null",
    "Present": "[Input] Is Not Null",
    "NegatedMissing": "Not ([Input] Is Null)",
    "FalseAnd": "False And ([Input]=0)",
    "TrueAnd": "True And ([Input]=0)",
    "FalseOr": "False Or ([Input]=0)",
    "TrueOr": "True Or ([Input]=0)",
    "TrueXor": "True Xor ([Input]=0)",
    "FalseEqv": "False Eqv ([Input]=0)",
    "TrueConsequent": "([Input]=0) Imp True",
    "FalseAntecedent": "False Imp ([Input]=0)",
    "TrueAntecedent": "True Imp ([Input]=0)",
    "FalseConsequent": "([Input]=0) Imp False",
    "NotUnknown": "Not Null",
    "BlockedCall": "Len('x')>0",
    "BlockedDependency": "[Equal]=True",
    "LateEagerError": "([Input]<>1) Or (1/([Input]-2)>0)",
}
BLOCKED = ("BlockedCall", "BlockedDependency")


def execute(*arguments):
    result = subprocess.run(list(map(str, arguments)), capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stderr
    return result.stdout


def negated(value):
    return None if value is None else not value


def conjunction(left, right):
    if left is False or right is False:
        return False
    return None if left is None or right is None else True


def disjunction(left, right):
    if left is True or right is True:
        return True
    return None if left is None or right is None else False


def exclusive(left, right):
    return None if left is None or right is None else left != right


def implication(left, right):
    return disjunction(negated(left), right)


def model(value):
    def relation(operation, constant):
        return None if value is None else operation(value, constant)

    equal = relation(operator.eq, 0)
    greater_equal = relation(operator.ge, 0)
    less_equal = relation(operator.le, 0)
    return {
        "Equal": equal,
        "NotEqual": relation(operator.ne, 0),
        "Less": relation(operator.lt, 0),
        "LessEqual": less_equal,
        "Greater": relation(operator.gt, 0),
        "GreaterEqual": greater_equal,
        "Negated": negated(equal),
        "Conjunction": conjunction(relation(operator.gt, -1), relation(operator.lt, 2)),
        "Disjunction": disjunction(relation(operator.lt, 0), relation(operator.gt, 1)),
        "Exclusive": exclusive(greater_equal, less_equal),
        "Equivalent": negated(exclusive(greater_equal, less_equal)),
        "Implies": implication(relation(operator.lt, 0), relation(operator.gt, 0)),
        "Missing": value is None,
        "Present": value is not None,
        "NegatedMissing": negated(value is None),
        "FalseAnd": conjunction(False, equal),
        "TrueAnd": conjunction(True, equal),
        "FalseOr": disjunction(False, equal),
        "TrueOr": disjunction(True, equal),
        "TrueXor": exclusive(True, equal),
        "FalseEqv": negated(exclusive(False, equal)),
        "TrueConsequent": implication(equal, True),
        "FalseAntecedent": implication(False, equal),
        "TrueAntecedent": implication(True, equal),
        "FalseConsequent": implication(equal, False),
        "NotUnknown": negated(None),
    }


def truth(value):
    return "NULL" if value is None else ("TRUE" if value else "FALSE")


def records(output):
    tables = {}
    for line in output.splitlines():
        tag, table, *cells = line.split("\t")
        entry = tables.setdefault(table, {"fields": [], "rows": []})
        if tag == "D":
            name, carrier, capacity, result, encoded = cells
            assert carrier == "BOOLEAN" and capacity == "39" and result == "1"
            assert bytes.fromhex(encoded).decode("utf-8") == FORMULAS[name]
            entry["fields"].append(name)
        elif tag == "R":
            identity, value, *cached = cells
            assert int(identity) == len(entry["rows"]) + 1
            assert len(cached) == len(FORMULAS)
            entry["rows"].append((None if value == "N" else int(value), cached))
        else:
            raise AssertionError("Unexpected original getter record")
    assert set(tables) == {"Predicates", "EmptyPredicates"}
    for table, entry in tables.items():
        assert entry["fields"] == list(FORMULAS)
        assert [value for value, _ in entry["rows"]] == ([] if table.startswith("Empty") else [-2, -1, 0, 1, 2, None])
    return tables


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    arguments = parser.parse_args()
    assert not arguments.report.exists(), "Refuse to overwrite accepted evidence"
    assert digest(arguments.jar) == JACKCESS_SHA256
    paths = {
        arguments.cli, arguments.jar, Path(__file__), RECIPES / "PredicatePreviewFixture.java",
        RECIPES / "PredicatePreviewOracle.java", RECIPES / "cli_tsv.py",
        RECIPES / "compare_calculated_decimals.py",
        ROOT / "crates/backends/tablecraft-access/src/source/calculation.rs",
        ROOT / "crates/backends/tablecraft-access/src/source/calculation/syntax.rs",
        ROOT / "apps/tablecraft-cli/src/commands/query/preview_calculation.rs",
        *[arguments.directory / f"{profile}.accdb" for profile in ("V2016", "V2019")],
    }
    hashes = {str(path.resolve().relative_to(ROOT)): digest(path) for path in sorted(paths)}
    controls = 0
    recomputed_cells = 0
    stale_cells = 0
    observations = []

    def preview(path, table, field, *options):
        output = execute(arguments.cli, "--format", "tsv", "query", "preview-calculation",
                         "--database", path, table, field, "--number-format", "invariant", *options)
        heading, *values = display_tsv(output).splitlines()
        assert heading == "Recomputed"
        return values

    def refused(path, table, field, *options, reason):
        result = subprocess.run(
            list(map(str, (arguments.cli, "query", "preview-calculation", "--database",
                           path, table, field, "--number-format", "invariant", *options))),
            capture_output=True, timeout=120
        )
        assert result.returncode == 2 and not result.stdout, result.stderr
        assert reason.encode() in result.stderr, result.stderr

    for profile in ("V2016", "V2019"):
        path = arguments.directory / f"{profile}.accdb"
        oracle = execute("java", "-cp", arguments.jar, RECIPES / "PredicatePreviewOracle.java", path)
        for table, entry in records(oracle).items():
            controls += len(FORMULAS)
            expected = [model(value) for value, _ in entry["rows"]]
            for index, field in enumerate(entry["fields"]):
                if field in BLOCKED:
                    refused(path, table, field, reason="outside the supported row-local profile")
                elif field == "LateEagerError":
                    if entry["rows"]:
                        refused(path, table, field, reason="division by zero")
                    else:
                        assert preview(path, table, field) == []
                else:
                    values = [truth(row[field]) for row in expected]
                    assert preview(path, table, field) == values, (profile, table, field, values)
                    recomputed_cells += len(values)
                    for actual, (_, cached) in zip(values, entry["rows"]):
                        stored = {"N": "NULL", "T": "TRUE", "F": "FALSE"}[cached[index]]
                        assert stored != actual, (profile, table, field, stored, actual)
                        stale_cells += 1
                controls += 1

            for flag, reason in (
                ("--native-bytes", "Bytes resource limit"),
                ("--execution-bytes", "Bytes resource limit"),
                ("--execution-steps", "Steps resource limit"),
                ("--output-bytes", "output limit"),
            ):
                refused(path, table, "Equal", flag, "1", reason=reason)
                controls += 1
            observations.append({
                "profile": profile, "table": table, "declarations": len(FORMULAS),
                "rows": len(entry["rows"]), "oracle_sha256": hashlib.sha256(oracle.encode()).hexdigest(),
            })

    assert all(digest(ROOT / path) == expected for path, expected in hashes.items())
    report = {
        "date": "2026-10-24", "scope": __doc__, "controls": controls,
        "recomputed_cells": recomputed_cells, "distinct_stale_cache_cells": stale_cells,
        "hashes_unchanged": True, "hashes": hashes, "observations": observations,
    }
    arguments.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {controls} controls and {recomputed_cells} independently modeled stale-cache distinctions")


if __name__ == "__main__":
    main()
