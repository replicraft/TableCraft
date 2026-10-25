"""Original ordinary-getter model for explicitly recomputed native IIf previews.

Stored formulas/caches are inspected inertly; no reference implementation executes
these expressions, and malformed declarations are not native-qualified formulas.
"""

import argparse
import base64
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess

from compare_progressive_queries import display as scalar_display


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019")
JAR_HASH = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
PRIMARY = "https://support.microsoft.com/en-us/office/add-functions-to-access-expressions-72a471a0-4f0e-4d49-923b-be97b0fa4bb0"
FORMULAS = {
    "NumericIIf": "IIf([Input]>0,[Input]*2,0-[Input])",
    "NullableIIf": "IIf([Input] IS NULL,NULL,[Input]+1)",
    "BooleanIIf": "IIf([Input] IS NULL,True,[Input]>0)",
    "TextIIf": "IIf([Input] IS NULL,'missing',[Label])",
    "NullCondition": "IIf(NULL,[Input],7)",
    "Selected": "iIf([BooleanIIf],[NumericIIf],[NullableIIf])",
    "Title": "IIf([BooleanIIf],[TextIIf]&'!',[TextIIf]&'?')",
    "HiddenError": "IIf(True,[Input],1/0)",
    "HiddenDate": "IIf(True,[Input],Date())",
    "HiddenUnknown": "IIf(True,[Input],Mystery([ID]))",
    "HiddenIsNullCall": "IIf(True,[Input],IsNull([Input]))",
    "HiddenBadDependency": "IIf(True,[Input],[HiddenUnknown])",
    "TooFew": "IIf(True,[Input])",
    "TooMany": "IIf(True,[Input],0,1)",
    "Omitted": "IIf(True,,[Input])",
    "SelfCycle": "IIf(True,[Input],[SelfCycle])",
    "CycleA": "IIf(True,[Input],[CycleB])",
    "CycleB": "[CycleA]",
}
SUCCESSFUL = (
    "NumericIIf",
    "NullableIIf",
    "BooleanIIf",
    "TextIIf",
    "NullCondition",
    "Selected",
    "Title",
)
UNSUPPORTED = tuple(name for name in FORMULAS
                    if name not in SUCCESSFUL and name != "HiddenError")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def decode(value):
    if value == "N":
        return None
    if value in ("B0", "B1"):
        return value == "B1"
    if value.startswith("I"):
        return int(value[1:])
    assert value.startswith("T")
    return base64.b64decode(value[1:], validate=True).decode("utf-8")


def display(value):
    if isinstance(value, bool):
        return "TRUE" if value else "FALSE"
    return scalar_display(value)


def model(column, row):
    number = row["Input"]
    positive = number is not None and number > 0
    numeric = None if number is None else number * 2 if positive else -number
    nullable = None if number is None else number + 1
    boolean = number is None or positive
    text = "missing" if number is None else row["Label"]
    return {
        "NumericIIf": numeric,
        "NullableIIf": nullable,
        "BooleanIIf": boolean,
        "TextIIf": text,
        "NullCondition": 7,
        "Selected": numeric if boolean else nullable,
        "Title": (text or "") + ("!" if boolean else "?"),
    }[column]


def getters(path):
    tables = {}
    for line in path.read_text().splitlines():
        parts = line.split("\t")
        if parts[0] == "TABLE":
            tables[parts[1]] = {"count": int(parts[2]), "columns": [], "rows": []}
        elif parts[0] == "COLUMN":
            name, kind, calculated, expression, result_type = parts[2:]
            if name in FORMULAS:
                assert calculated == "true" and decode(expression) == FORMULAS[name]
                expected = "BOOLEAN" if name == "BooleanIIf" else (
                    "TEXT" if name in ("TextIIf", "Title") else "LONG"
                )
                assert kind == expected
                assert result_type == {"BOOLEAN": "1", "TEXT": "10", "LONG": "4"}[kind]
            else:
                assert calculated == "false" and expression == "N" and result_type == "null"
            tables[parts[1]]["columns"].append(name)
        else:
            assert parts[0] == "ROW"
            table = tables[parts[1]]
            table["rows"].append(dict(zip(table["columns"], map(decode, parts[2:]), strict=True)))
    assert tables["Choices"]["count"] == len(tables["Choices"]["rows"]) == 5
    assert tables["EmptyChoices"]["count"] == len(tables["EmptyChoices"]["rows"]) == 0
    for row in tables["Choices"]["rows"]:
        for name in FORMULAS:
            expected = False if name == "BooleanIIf" else (
                "stale cache" if name in ("TextIIf", "Title") else 999
            )
            assert row[name] == expected
    return tables


def invoke(command, observations):
    command = [relative(value) if isinstance(value, Path) else str(value)
               for value in command]
    result = subprocess.run(command, capture_output=True, text=True, timeout=120, cwd=ROOT)
    observations.append({
        "command": command,
        "exit": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
    })
    return result


def generate(args):
    assert not args.directory.exists(), "Refuse to overwrite original corpus"
    assert digest(args.jar) == JAR_HASH
    args.directory.mkdir(parents=True)
    classes = args.directory / "classes"
    classes.mkdir()
    runner = args.directory / "generation-runner.py"
    runner.write_bytes(Path(__file__).read_bytes())
    recipe = ROOT / "docs/testing/CalculatedIIfFixture.java"
    observations = []
    result = invoke(["javac", "-cp", args.jar, "-d", classes, recipe], observations)
    assert result.returncode == 0, result.stderr
    originals = []
    for profile in PROFILES:
        path = args.directory / (profile + ".accdb")
        classpath = f"{args.jar}:{classes}"
        result = invoke(["java", "-cp", classpath, "CalculatedIIfFixture", "write", path, profile], observations)
        assert result.returncode == 0, result.stderr
        before = digest(path)
        result = invoke(["java", "-cp", classpath, "CalculatedIIfFixture", "read", path], observations)
        assert result.returncode == 0, result.stderr
        getter = path.with_suffix(".accdb.getters.tsv")
        getter.write_text(result.stdout)
        getters(getter)
        assert digest(path) == before
        originals.append({
            "profile": profile,
            "path": relative(path),
            "sha256": before,
            "getter": relative(getter),
            "getter_sha256": digest(getter),
        })
    compiled = classes / "CalculatedIIfFixture.class"
    report = {
        "scope": "Original inert IIf declarations/caches and ordinary getters; no native execution",
        "recipe": relative(recipe),
        "recipe_sha256": digest(recipe),
        "runner": relative(runner),
        "runner_sha256": digest(runner),
        "class": relative(compiled),
        "class_sha256": digest(compiled),
        "jar": relative(args.jar),
        "jar_sha256": digest(args.jar),
        "originals": originals,
        "commands": observations,
    }
    (args.directory / "generation.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Generated two unchanged originals; metadata and every stale cache cell verified")


def replay(args):
    assert args.cli and args.cli_sha256 and args.source_manifest and args.manifest_sha256
    assert args.report and not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.source_manifest) == args.manifest_sha256
    assert digest(args.jar) == JAR_HASH
    generation = args.directory / "generation.json"
    authored = json.loads(generation.read_text())
    paths = {args.cli, args.jar, args.source_manifest, generation, Path(__file__).resolve()}
    paths.update(path for path in args.directory.rglob("*") if path.is_file())
    paths.add(ROOT / "docs/testing/CalculatedIIfFixture.java")
    paths.add(ROOT / "docs/testing/compare_progressive_queries.py")
    paths.add(ROOT / "Cargo.lock")
    for key in ("recipe", "runner", "class"):
        assert digest(ROOT / authored[key]) == authored[key + "_sha256"]
    for item in authored["originals"]:
        assert digest(ROOT / item["path"]) == item["sha256"]
        assert digest(ROOT / item["getter"]) == item["getter_sha256"]
    for line in args.source_manifest.read_text().splitlines():
        name, expected = line.split("\0")
        path = ROOT / name
        assert digest(path) == expected, name
        paths.add(path)
    pins = {relative(path): digest(path) for path in sorted(paths)}
    observations = []
    failures = []
    matched_cells = 0
    stale_differences = 0

    def check(label, command, expected=None, refusal=False, diagnostic=None):
        nonlocal matched_cells
        result = invoke(command, observations)
        observation = observations[-1]
        observation["label"] = label
        observation["expected_rows"] = expected
        observation["expected_refusal"] = refusal
        observation["expected_diagnostic"] = diagnostic
        problem = None
        if refusal:
            if result.returncode == 0 or result.stdout:
                problem = "Expected buffered refusal without stdout"
            if diagnostic and diagnostic not in result.stderr:
                problem = "Missing expected diagnostic"
        elif result.returncode != 0:
            problem = "Unexpected command failure"
        elif expected is not None:
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            if actual != expected:
                observation["actual_rows"] = actual
                problem = "Independent ordinary-getter model disagreement"
            matched_cells += sum(len(row) for row in expected[1:])
        if problem:
            failures.append({"label": label, "problem": problem})

    for item in authored["originals"]:
        profile = item["profile"]
        path = ROOT / item["path"]
        tables = getters(ROOT / item["getter"])
        base = [args.cli, "--format", "tsv", "query", "preview-calculation", "--database", path,
                "--number-format", "invariant"]
        for table, content in tables.items():
            for name in SUCCESSFUL:
                values = [model(name, row) for row in content["rows"]]
                expected = [["Recomputed"], *[[display(value)] for value in values]]
                check(f"{profile}/{table}/{name}", [*base, table, name], expected)
                stale_differences += sum(value != row[name]
                                         for value, row in zip(values, content["rows"], strict=True))
            for name in UNSUPPORTED:
                check(f"{profile}/{table}/unsupported/{name}", [*base, table, name], refusal=True)
        check(f"{profile}/hidden-error-eager", [*base, "Choices", "HiddenError"],
              refusal=True, diagnostic="division by zero")
        check(f"{profile}/empty-arithmetic-eof", [*base, "EmptyChoices", "HiddenError"], [["Recomputed"]])
        names = ["ID", *SUCCESSFUL]
        cached = [names, *[[display(row[name]) for name in names] for row in tables["Choices"]["rows"]]]
        sql = "SELECT " + ",".join(names) + " FROM Choices"
        check(f"{profile}/stored-cache-control",
              [args.cli, "--format", "tsv", "query", "run", "--database", path,
               "--stored-calculations", sql], cached)
        check(f"{profile}/ordinary-query-still-refused",
              [args.cli, "query", "run", "--database", path, "SELECT NumericIIf FROM Choices"], refusal=True)
        check(f"{profile}/positive-quotas", [*base, "Choices", "Title", "--native-rows", "10000",
              "--execution-steps", "100000"],
              [["Recomputed"], *[[display(model("Title", row))] for row in tables["Choices"]["rows"]]])
        for option in ("--native-rows", "--native-steps", "--execution-steps", "--output-bytes"):
            check(f"{profile}/quota/{option}", [*base, "Choices", "Title", option, "1"], refusal=True)
    assert all(digest(ROOT / name) == expected for name, expected in pins.items())
    report = {
        "scope": "Independent ordinary-getter IIf preview model; no native Access execution, cache freshness or result coercion qualification",
        "primary_table_expression_contract": PRIMARY,
        "controls": len(observations),
        "matched_cells": matched_cells,
        "preview_values_differing_from_raw_caches": stale_differences,
        "pins": pins,
        "observations": observations,
        "failures": failures,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"{len(observations)} controls; {matched_cells} cells; {stale_differences} stale-cache differences; {len(failures)} disagreements; {len(pins)} unchanged pins")
    assert not failures, failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generate", action="store_true")
    for name in ("directory", "jar"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("cli", "source-manifest", "report"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--cli-sha256")
    parser.add_argument("--manifest-sha256")
    args = parser.parse_args()
    for name in ("directory", "jar", "cli", "source_manifest", "report"):
        value = getattr(args, name)
        if value is not None:
            setattr(args, name, Path(relative(value)))
    if args.generate:
        generate(args)
    else:
        replay(args)


if __name__ == "__main__":
    main()
