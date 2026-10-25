"""Getter-backed actual-String policy contribution replay with a fixed baseline.

No reference collation weights or SQL execution are used. Exact ASCII output
identities supply equality goldens; policy-conflict refusals are engineering rules.
"""

import argparse
import base64
import csv
from dataclasses import dataclass
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019", "V2000", "V2007")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
BASELINE_SHA256 = "8b9c8e89f6804f57e45a6bba74cf7b6ebbdb8b7a20917cd8b1170c6e31710d23"
FIXTURE_SHA256 = {
    "V2000": "9aa2b386d739e99228ac6ed82e772dc073f3094d038d32a08cb0785718d5c4a2",
    "V2007": "431835e835186515df4ab11673aef860741c935ad0867dc73166f577e4688c28",
    "V2016": "86076051e896e0e79c5e2e649f4dfd5e0386e6a87ef145314730901f8454ee6e",
    "V2019": "3ae803876edee6a5510169ebbaa380a8a1dbe1a522edf4b080d7047ea5ce3347",
}


@dataclass(frozen=True)
class Case:
    name: str
    expression: str
    text: str | None
    declared: bool = True
    refusal: str | None = None
    previous_conflict: bool = False


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def literal(text):
    return "'" + text.replace("'", "''") + "'"


def cases(left):
    result = []
    for declared in (False, True):
        suffix = ":declared" if declared else ":default"
        for name, expression, text in (
            ("number", "LegacyText & IIf(True,2,ModernText)", left + "2"),
            ("reverse", "IIf(True,2,ModernText) & LegacyText", "2" + left),
            ("currency", "LegacyText & IIf(True,CCur(1.25),ModernText)", left + "1.25"),
            ("boolean", "LegacyText & IIf(True,True,ModernText)", left + "True"),
            ("date", "LegacyText & IIf(True,#2000-01-02#,ModernText)", left + "2000-01-02"),
            ("both-numeric", "IIf(True,1,LegacyText) & IIf(True,2,ModernText)", "12"),
        ):
            result.append(Case(name + suffix, expression, text, declared, previous_conflict=True))
        result.append(Case("conditional-null" + suffix,
                           "LegacyText & IIf(True,NULL,ModernText)", left, declared,
                           refusal="conflicting text comparison"))
        result.append(Case("literal-null" + suffix, "LegacyText & NULL", left, declared))
        result.append(Case("cstr-control" + suffix,
                           "LegacyText & CStr(IIf(True,2,ModernText))", left + "2", declared))
        for name, expression in (
            ("actual-strings", "LegacyText & ModernText"),
            ("selected-string", "LegacyText & IIf(False,2,ModernText)"),
        ):
            result.append(Case(name + suffix, expression, left + "A", declared,
                               refusal="conflicting text comparison"))
        result.append(Case("cstr-actual-string" + suffix,
                           "LegacyText & CStr(IIf(False,2,ModernText))", left + "A", declared))
    for name, expression, text in (
        ("plus-number", "LegacyText + IIf(True,2,ModernText)", left + "2"),
        ("plus-reverse", "IIf(True,2,ModernText) + LegacyText", "2" + left),
    ):
        result.append(Case(name, expression, text, previous_conflict=True))
    result.append(Case("plus-null", "LegacyText + IIf(True,NULL,ModernText)", None))
    result.append(Case("plus-default", "LegacyText + IIf(True,2,ModernText)", None,
                       declared=False, refusal="type mismatch"))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "baseline", "jar", "directory", "getters", "report", "source_manifest"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.baseline) == BASELINE_SHA256
    assert digest(args.jar) == JAR_SHA256
    assert digest(args.source_manifest) == args.manifest_sha256
    paths = {args.cli, args.baseline, args.jar, args.source_manifest, Path(__file__), ROOT / "Cargo.lock"}
    paths.update(args.directory.iterdir())
    paths.update(args.getters.iterdir())
    paths.update(ROOT / "docs/testing" / name for name in (
        "NativePolicyFixture.java", "ConcatenationPolicyOracle.java",
    ))
    for line in args.source_manifest.read_text().splitlines():
        name, expected = line.split("\0")
        path = ROOT / name
        assert digest(path) == expected, name
        paths.add(path)
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path)
              for path in sorted(paths) if path.is_file()}
    observations = []
    modeled_cells = {"candidate": 0, "baseline": 0}

    def execute(label, database, sql, expected=None, refusal=None, declared=True,
                binary=None, comparison="database", number="invariant", extra=()):
        binary = binary or args.cli
        role = "baseline" if binary == args.baseline else "candidate"
        command = [str(binary), "--format", "tsv", "query", "run", "--database", str(database),
                   "--collation", comparison, "--date-format", "iso"]
        if number:
            command.extend(["--number-format", number])
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
            modeled_cells[role] += sum(len(row) for row in expected[1:])
        observations.append({"label": label, "role": role, "command": command, "exit": result.returncode,
                             "stdout": result.stdout, "stderr": result.stderr, "expected_refusal": refusal})

    for profile in PROFILES:
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        database = args.directory / ("Policies-" + profile + suffix)
        assert digest(database) == FIXTURE_SHA256[profile]
        records = [line.split("\t") for line in (args.getters / (profile + ".getters.tsv")).read_text().splitlines()]
        default = next(row[1:] for row in records if row[0] == "DATABASE")
        assert default[0] == "1033" and default[1] in ("0", "1") and default[2] == "0"
        columns = {row[1]: row[2:] for row in records if row[0] == "COLUMN"}
        assert columns["LegacyText"] == ["TEXT", "1033", "0", "0"]
        assert columns["ModernText"] == ["TEXT", "1033", "1", "0"]
        raw = next(row for row in records if row[:2] == ["ROW", "3"])
        left = base64.b64decode(raw[2][1:]).decode("utf-8")
        right = base64.b64decode(raw[3][1:]).decode("utf-8")
        assert left == "a" and right == "A"
        for case in cases(left):
            projection = case.expression if case.text is None else "(" + case.expression + ")=" + literal(case.text)
            sql = "SELECT " + projection + " AS V FROM PolicyProbe WHERE ID=3"
            expected = [["V"], ["NULL" if case.text is None else "TRUE"]]
            execute(profile + ":candidate:" + case.name, database, sql, expected, case.refusal, case.declared)
            refusal = "conflicting text comparison" if case.previous_conflict else case.refusal
            execute(profile + ":baseline:" + case.name, database, sql, expected, refusal,
                    case.declared, binary=args.baseline)

        expression = "LegacyText & IIf(True,2,ModernText)"
        for label, sql, expected in (
            ("derived", "SELECT V='a2' AS V FROM (SELECT " + expression
             + " AS V FROM PolicyProbe WHERE ID=3)", [["V"], ["TRUE"]]),
            ("union", "SELECT V='a2' AS V FROM (SELECT " + expression
             + " AS V FROM PolicyProbe WHERE ID=3 UNION ALL SELECT 'a2' AS V)",
             [["V"], ["TRUE"], ["TRUE"]]),
            ("aggregate", "SELECT Min(" + expression + ")='a2' AS V FROM PolicyProbe WHERE ID=3",
             [["V"], ["TRUE"]]),
            ("domain-lookup", "SELECT DLookup(" + literal(expression) + ",'PolicyProbe','ID=3')='a2' AS V",
             [["V"], ["TRUE"]]),
            ("domain-count", "SELECT DCount('*','PolicyProbe',"
             + literal("ID=3 AND (" + expression + ")='a2'") + ") AS N", [["N"], ["1"]]),
        ):
            execute(profile + ":candidate:" + label, database, sql, expected)
            execute(profile + ":baseline:" + label, database, sql, refusal="conflicting text comparison",
                    binary=args.baseline)
        empty_arm = (
            "SELECT V='a2' AS V FROM (SELECT " + expression + " AS V FROM PolicyProbe WHERE ID=3 "
            "UNION ALL SELECT ModernText AS V FROM PolicyProbe WHERE ID<0)"
        )
        for label, binary in (("candidate", args.cli), ("baseline", args.baseline)):
            execute(profile + ":" + label + ":empty-actual-string-arm", database, empty_arm,
                    refusal="conflicting text comparison", binary=binary)
        sql = "SELECT (" + expression + ")='a2' AS V FROM PolicyProbe WHERE ID=3"
        execute(profile + ":selected-index", database, sql, [["V"], ["TRUE"]],
                extra=("--index", "PolicyProbe", "PrimaryKey"))
        execute(profile + ":global-override", database,
                "SELECT (LegacyText & ModernText)='aA' AS V FROM PolicyProbe WHERE ID=3",
                [["V"], ["TRUE"]], comparison="ordinal")
        execute(profile + ":missing-format", database, sql, refusal="numeric text conversion", number=None)
        execute(profile + ":eager-error", database,
                "SELECT (LegacyText & IIf(True,1/0,ModernText))='a2' AS V FROM PolicyProbe WHERE ID=3",
                refusal="division by zero")
        for option in ("--execution-bytes", "--execution-steps", "--output-bytes"):
            message = "output limit" if option == "--output-bytes" else "resource limit"
            execute(profile + ":positive-limit:" + option, database, sql,
                    refusal=message, extra=(option, "1"))

    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    pairs = {}
    for observation in observations:
        label = observation["label"].replace(":candidate:", ":").replace(":baseline:", ":")
        pairs.setdefault(label, {})[observation["role"]] = observation
    matched = [pair for pair in pairs.values() if "candidate" in pair and "baseline" in pair]
    changed = sum(
        (pair["candidate"]["exit"], pair["candidate"]["stdout"], pair["candidate"]["stderr"])
        != (pair["baseline"]["exit"], pair["baseline"]["stdout"], pair["baseline"]["stderr"])
        for pair in matched
    )
    report = {"scope": "Getter-backed actual-String policy routing; ASCII identities, "
                       "not native collation qualification.",
              "controls": len(observations), "modeled_cells": modeled_cells,
              "paired_baseline_controls": len(matched), "changed_pairs": changed,
              "hashes": hashes, "observations": observations}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"controls": len(observations), "hashes": len(hashes), "report": str(args.report)}))


if __name__ == "__main__":
    main()
