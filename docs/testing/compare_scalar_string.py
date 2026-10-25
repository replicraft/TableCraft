"""Independent declared Scalar String/Variant formatting comparison replay.

The model uses explicit short rendered goldens and ordinal String comparisons,
not numeric comparison of the text input. No reference SQL is executed.
"""

import argparse
import base64
import csv
from decimal import Decimal
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019", "V2000", "V2007")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
OPERATORS = ("=", "<>", "<", ">", "<=", ">=")
SPELLINGS = ("02", "10", "bad", "", "é", "-2", "2", "True", "1.25", "2000-01-02", "2 ", "0")
PEERS = (
    ("integer", "CVar(2)", "CVar([Number])", "2", "invariant", None, 0),
    ("boolean", "CVar(True)", "CVar([Flag])", "True", None, None, 1),
    ("currency", "CVar(CCur(1.25))", "CVar([Money])", "1.25", "invariant", None, 2),
    ("currency-regional", "CVar(CCur('1,25'))", "CVar([Money])", "1,25", "de-de", None, 2),
    ("date-iso", "CVDate(#2000-01-02#)", "CVar([D])", "2000-01-02", None, "iso", 3),
    ("date-us", "CVDate(#2000-01-02#)", "CVar([D])", "1/2/2000", None, "en-us", 3),
    ("date-de", "CVDate(#2000-01-02#)", "CVar([D])", "02.01.2000", None, "de-de", 3),
)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def literal(value):
    return "NULL" if value is None else "'" + value.replace("'", "''") + "'"


def display(value):
    if value is None:
        return "NULL"
    if isinstance(value, bool):
        return "TRUE" if value else "FALSE"
    return str(value)


def compare(left, right, operator):
    if left is None or right is None:
        return None
    # Explicit ordinal compares complete Strings, including trailing spaces.
    order = (left > right) - (left < right)
    return {
        "=": order == 0, "<>": order != 0, "<": order < 0,
        ">": order > 0, "<=": order <= 0, ">=": order >= 0,
    }[operator]


def expressions(left, right):
    return [f"{a}{operator}{b} AS C{position}" for position, (a, operator, b) in enumerate(
        (a, operator, b) for a, b in ((left, right), (right, left)) for operator in OPERATORS
    )]


def answers(left, right):
    return [display(compare(a, b, operator))
            for a, b in ((left, right), (right, left)) for operator in OPERATORS]


def getter(path):
    rows = []
    columns = []
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "C":
            columns.append(tuple(fields[1:]))
            continue
        assert fields[0] == "R", fields
        cells = [None if cell == "N" else base64.b64decode(cell[1:]).decode("utf-8") for cell in fields[2:]]
        rows.append((int(fields[1]), *cells))
    assert columns == [
        ("ID", "LONG"), ("Spelling", "TEXT"), ("Number", "LONG"),
        ("Flag", "BOOLEAN"), ("Money", "MONEY"), ("D", "SHORT_DATE_TIME"),
    ]
    for _, _, integer, boolean, money, day in rows:
        assert integer in (None, "2")
        assert boolean in ("true", "false")
        assert money is None or Decimal(money) == Decimal("1.25")
        assert day in (None, "2000-01-02T00:00")
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.jar) == JAR_SHA256
    paths = {
        args.cli, args.jar, Path(__file__), ROOT / "Cargo.lock",
        ROOT / ".cache/declared-string-rust-source-20261024.manifest",
    }
    paths.update(args.directory.iterdir())
    paths.update(ROOT / "docs/testing" / name for name in ("ScalarStringFixture.java", "ScalarStringOracle.java"))
    for directory in ("crates/query/tablecraft-expressions/src", "crates/query/tablecraft-query/src"):
        paths.update((ROOT / directory).rglob("*.rs"))
    manifest = ROOT / ".cache/declared-string-rust-source-20261024.manifest"
    assert digest(manifest) == "ec3f7808c617daf80477f51af569cdcde4d6e5c7b1cf9826b8de223b720a7024"
    for line in manifest.read_text().splitlines():
        name, expected = line.split("\0")
        path = ROOT / name
        assert digest(path) == expected, name
        paths.add(path)
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path)
              for path in sorted(paths) if path.is_file()}
    observations = []
    model_counts = {"boolean_values": 0, "null_values": 0, "direct_comparison_values": 0}
    headers = [f"C{position}" for position in range(12)]

    def execute(label, sql, expected=None, database=None, number="invariant", date=None,
                collation="ordinal", declared=True, refusal=None, extra=()):
        command = [str(args.cli), "--format", "tsv", "query", "run"]
        if declared:
            command.append("--declared-arithmetic")
        if collation:
            command.extend(["--collation", collation])
        if number:
            command.extend(["--number-format", number])
        if date:
            command.extend(["--date-format", date])
        if database:
            command.extend(["--database", str(database)])
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
            model_counts["boolean_values"] += sum(
                cell in ("TRUE", "FALSE") for row in expected[1:] for cell in row
            )
            model_counts["null_values"] += sum(cell == "NULL" for row in expected[1:] for cell in row)
            if label.startswith("source-free:"):
                model_counts["direct_comparison_values"] += sum(len(row) for row in expected[1:])
            elif ":all-operators:" in label:
                model_counts["direct_comparison_values"] += sum(len(row) - 1 for row in expected[1:])
        observations.append({"label": label, "command": command, "exit": result.returncode,
                             "stdout": result.stdout, "stderr": result.stderr, "expected_refusal": refusal})

    # Independent discriminators: text ordering must differ from numeric coercion.
    assert answers("02", "2") == ["FALSE", "TRUE", "TRUE", "FALSE", "TRUE", "FALSE",
                                      "FALSE", "TRUE", "FALSE", "TRUE", "FALSE", "TRUE"]
    assert compare("10", "2", "<") is True
    for name, source, _, rendered, number, date, _ in PEERS:
        for spelling in (*SPELLINGS, None):
            sql = "SELECT " + ",".join(expressions(literal(spelling), source))
            execute(f"source-free:{name}:{spelling!r}", sql,
                    [headers, answers(spelling, rendered)], number=number, date=date)
        execute("cstr-golden:" + name, "SELECT CStr(" + source + ") AS Rendered",
                [["Rendered"], ['"' + rendered + '"']], number=number, date=date)

    for spelling in SPELLINGS:
        # Retained both-Variant rank: numeric2 precedes any String regardless of spelling.
        expected = ["FALSE", "TRUE", "FALSE", "TRUE", "FALSE", "TRUE",
                    "FALSE", "TRUE", "TRUE", "FALSE", "TRUE", "FALSE"]
        execute("both-variant-rank:" + spelling,
                "SELECT " + ",".join(expressions("CVar(" + literal(spelling) + ")", "CVar(2)")),
                [headers, expected], number=None, collation=None)
    for spelling in ("02", "10", "-2", "2", "0"):
        value = int(spelling)
        numeric = [display({"=": value == 2, "<>": value != 2, "<": value < 2,
                            ">": value > 2, "<=": value <= 2, ">=": value >= 2}[op]) for op in OPERATORS]
        for declared in (False, True):
            execute(f"numeric-scalar-variant-string:{declared}:{spelling}",
                    "SELECT " + ",".join(f"CVar({literal(spelling)}){op}2 AS C{i}"
                                         for i, op in enumerate(OPERATORS)),
                    [headers[:6], numeric], declared=declared, collation=None)
            execute(f"default-mixed:{declared}:{spelling}",
                    "SELECT " + ",".join(f"{literal(spelling)}{op}CVar(2) AS C{i}"
                                         for i, op in enumerate(OPERATORS)),
                    [headers[:6], numeric] if not declared else [headers[:6], answers(spelling, "2")[:6]],
                    declared=declared)

    execute("default-both-variant", "SELECT CVar('02')=CVar(2) AS V", [["V"], ["TRUE"]], declared=False)
    execute("default-both-variant-malformed", "SELECT CVar('bad')=CVar(2)",
            declared=False, refusal="type mismatch")
    for spelling in ("bad", "", "é"):
        execute("default-malformed:" + repr(spelling),
                "SELECT " + literal(spelling) + "=CVar(2)", declared=False, refusal="type mismatch")

    for profile in PROFILES:
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        path = args.directory / (profile + suffix)
        rows = getter(path.with_suffix(suffix + ".getters.tsv"))
        assert len(rows) == 17
        execute(profile + ":default-native", "SELECT Spelling=CVar([Number]) AS V "
                "FROM TextOperands WHERE ID=1", [["V"], ["TRUE"]], path, declared=False)
        for name, _, native, rendered, number, date, cell in PEERS:
            expected = [["ID", *headers]]
            for identifier, spelling, integer, boolean, money, day in rows:
                values = (integer, boolean, money, day)
                peer = None if values[cell] is None else rendered
                if cell == 1 and boolean == "false":
                    peer = "False"
                expected.append([str(identifier), *answers(spelling, peer)])
            execute(f"{profile}:all-operators:{name}",
                    "SELECT ID," + ",".join(expressions("Spelling", native))
                    + " FROM TextOperands ORDER BY ID", expected, path, number=number, date=date)

        for label, predicate, accepted in (
            ("membership", "Spelling IN(CVar(2),CVar(10))",
             lambda spelling: any(compare(spelling, peer, "=") is True for peer in ("2", "10"))),
            ("range", "Spelling BETWEEN CVar(1) AND CVar(3)",
             lambda spelling: compare(spelling, "1", ">=") is True and compare(spelling, "3", "<=") is True),
        ):
            expected = [["ID"], *[[str(row[0])] for row in rows if accepted(row[1])]]
            execute(profile + ":" + label, "SELECT ID FROM TextOperands WHERE " + predicate + " ORDER BY ID",
                    expected, path)
        equal_ids = [[str(row[0])] for row in rows if compare(row[1], "2", "=") is True]
        execute(profile + ":join",
                "SELECT T.ID FROM TextOperands T INNER JOIN (SELECT CVar(2) AS N) V "
                "ON T.Spelling=V.N ORDER BY T.ID", [["ID"], *equal_ids], path)
        execute(profile + ":quantified",
                "SELECT ID FROM TextOperands WHERE Spelling=ANY"
                "(SELECT CVar([Number]) FROM TextOperands WHERE ID=1) ORDER BY ID",
                [["ID"], *equal_ids], path)
        execute(profile + ":domain", "SELECT DCount('*','TextOperands','Spelling=CVar(2)') AS N",
                [["N"], [str(len(equal_ids))]], path)
        expected = [["ID", "V"], *[[str(row[0]), display(compare(row[1], None if row[2] is None else "2", "="))]
                                  for row in rows]]
        execute(profile + ":derived", "SELECT ID,V FROM "
                "(SELECT ID,Spelling=CVar([Number]) AS V FROM TextOperands) ORDER BY ID", expected, path)

        for label, expression in (
            ("unknown-number", "'2'=Max([Number])"),
            ("unknown-string", "Max(Spelling)=CVar(2)"),
            ("both-unknown", "Max(Spelling)=Max([Number])"),
            ("variant-string-unknown-number", "CVar('2')=Max([Number])"),
        ):
            execute(profile + ":" + label,
                    "SELECT " + expression + " AS V FROM TextOperands WHERE ID=1", database=path,
                    refusal="declaration is unknown")
        execute(profile + ":explicit-aggregate-variant",
                "SELECT '02'=CVar(Max([Number])) AS V FROM TextOperands WHERE ID=1",
                [["V"], ["FALSE"]], path)
        execute(profile + ":numeric-scalar-unknown-string",
                "SELECT 2=Max(Spelling) AS V FROM TextOperands WHERE ID=1", [["V"], ["TRUE"]], path)

    # Separate error/provider contracts must not be hidden by formatting or comparisons.
    for label, sql, options in (
        ("missing-number", "SELECT '2'=CVar(2)", {"number": None, "refusal": "numeric text conversion"}),
        ("missing-date", "SELECT '2000-01-02'=CVDate(#2000-01-02#)",
         {"number": None, "refusal": "date text conversion"}),
        ("missing-collation", "SELECT '2'=CVar(2)", {"collation": None, "refusal": "comparison"}),
        ("eager-zero", "SELECT 'bad'=CVar(1/0)", {"refusal": "division by zero"}),
        ("numeric-parse", "SELECT CVar('bad')=2", {"refusal": "type mismatch"}),
    ):
        execute(label, sql, **options)
    execute("null-no-policy", "SELECT NULL=CVar(2) AS A,'2'=CVar(NULL) AS B",
            [["A", "B"], ["NULL", "NULL"]], number=None, collation=None)
    for option in ("--execution-bytes", "--execution-steps", "--output-bytes"):
        message = "output limit" if option == "--output-bytes" else "resource limit"
        execute("positive-limit:" + option, "SELECT '02'=CVar(2) AS A",
                refusal=message, extra=(option, "1"))

    for path, before in hashes.items():
        assert digest(ROOT / path) == before, path
    report = {"scope": "Independent declared String-effective-type engineering model; not native Access qualification.",
              "controls": len(observations), "modeled_values": model_counts,
              "hashes": hashes, "observations": observations}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"controls": len(observations), "hashes": len(hashes), "report": str(args.report)}))


if __name__ == "__main__":
    main()
