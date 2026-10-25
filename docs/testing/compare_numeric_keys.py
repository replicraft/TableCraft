"""Independent exact numeric relational-key model and unchanged native getters.

This is an explicit engineering relation, not native ACE grouping qualification.
Scalar predicates and prepared static UNION have separate retained contracts.
"""

import argparse
from collections import Counter
import csv
from dataclasses import dataclass
from fractions import Fraction
import hashlib
from io import StringIO
import itertools
import json
from pathlib import Path
import struct
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
BASELINE_SHA256 = "2fea30893b358d283b895049abc47d484d946e8232e8359dc6eed175fdcf1a83"
PROFILES = ("V2000", "V2007", "V2016", "V2019")
ORDERS = tuple(itertools.permutations((1, 2, 3)))
CONSUMERS = ("group", "distinct", "union", "order", "top", "extrema", "pivot_rows")


@dataclass(frozen=True)
class Cell:
    tag: int
    key: object
    bits: int | None = None


@dataclass(frozen=True)
class Family:
    name: str
    expressions: tuple
    cells: tuple
    kind: str = "fixed"
    refusal: str | None = None
    partial: bool = False


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def ieee(bits, width):
    fraction_bits, exponent_bits, bias = (23, 8, 127) if width == 32 else (52, 11, 1023)
    exponent = (bits >> fraction_bits) & ((1 << exponent_bits) - 1)
    assert exponent != (1 << exponent_bits) - 1, "Only finite inputs belong to this key corpus"
    significand = bits & ((1 << fraction_bits) - 1)
    power = 1 - bias - fraction_bits
    if exponent:
        significand += 1 << fraction_bits
        power = exponent - bias - fraction_bits
    result = Fraction(significand)
    result = result * (1 << power) if power >= 0 else result / (1 << -power)
    return -result if bits >> (width - 1) else result


def floating(tag, text=None, bits=None):
    width = 32 if tag == 4 else 64
    code = ">f" if tag == 4 else ">d"
    if bits is None:
        bits = int.from_bytes(struct.pack(code, float(text)), "big")
    raw = struct.unpack(code, bits.to_bytes(width // 8, "big"))[0]
    widened = int.from_bytes(struct.pack(">d", raw), "big")
    return Cell(tag, ieee(bits, width), widened)


def fixed(tag, number):
    return Cell(tag, Fraction(number))


def decimal_significant(value, digits):
    if value == 0:
        return Fraction(0)
    sign = -1 if value < 0 else 1
    numerator, denominator = abs(value.numerator), value.denominator
    exponent = len(str(numerator)) - len(str(denominator))
    power = Fraction(10**exponent) if exponent >= 0 else Fraction(1, 10**-exponent)
    if abs(value) < power:
        exponent -= 1
    shift = digits - 1 - exponent
    if shift >= 0:
        numerator *= 10**shift
    else:
        denominator *= 10**-shift
    whole, remainder = divmod(numerator, denominator)
    whole += remainder * 2 > denominator or (remainder * 2 == denominator and whole % 2 == 1)
    result = Fraction(whole, 10**shift) if shift >= 0 else Fraction(whole * 10**-shift)
    return sign * result


def heading_labels(family, key):
    return {decimal_significant(cell.key, 7 if cell.tag == 4 else 15)
            for cell in family.cells if cell.key == key}


def heading_collision(family):
    single_labels = []
    for key in set(cell.key for cell in family.cells):
        labels = heading_labels(family, key)
        if len(labels) == 1:
            single_labels.append(next(iter(labels)))
    return len(single_labels) != len(set(single_labels))


def model_controls():
    checks = [
        (ieee(0x3F800000, 32), Fraction(1)),
        (ieee(1, 32), Fraction(1, 2**149)),
        (ieee(0x7F7FFFFF, 32), Fraction(((1 << 24) - 1) << 104)),
        (ieee(0x3FB999999999999A, 64), Fraction(3602879701896397, 2**55)),
        (ieee(1, 64), Fraction(1, 2**1074)),
        (decimal_significant(Fraction(125, 100), 2), Fraction(12, 10)),
        (decimal_significant(Fraction(135, 100), 2), Fraction(14, 10)),
        (decimal_significant(-Fraction(125, 100), 2), -Fraction(12, 10)),
    ]
    for actual, expected in checks:
        assert actual == expected
    assert floating(4, bits=0x80000000) == Cell(4, Fraction(0), 1 << 63)
    return len(checks) + 1


def families():
    return [
        Family("mixed-float", ("CDbl(1)", "CSng(1)", "CDbl(1.00000001)"),
               (floating(5, "1"), floating(4, "1"), floating(5, "1.00000001")), "floating"),
        Family("subnormal-float", ("CDbl(1.401298464324817e-45)", "CSng(1.401298464324817e-45)", "CDbl(2e-45)"),
               (floating(5, "1.401298464324817e-45"), floating(4, "1.401298464324817e-45"),
                floating(5, "2e-45")), "floating"),
        Family("large-float", ("CSng(3.4028234663852886e38)", "CDbl(3.4028234663852886e38)", "CDbl(1e39)"),
               (floating(4, "3.4028234663852886e38"), floating(5, "3.4028234663852886e38"),
                floating(5, "1e39")), "floating"),
        Family("signed-zero", ("(-CSng(0))", "CDbl(0)", "CInt(0)"),
               (floating(4, "-0"), floating(5, "0"), fixed(2, 0)), "floating"),
        Family("fixed-equal", ("CByte(1)", "CCur(1)", "CDec('1.00')"),
               (fixed(17, 1), fixed(6, 1), fixed(14, 1))),
        Family("currency-adjacent", ("CCur('922337203685477.5807')", "CCur('922337203685477.5806')",
                                    "CDec('922337203685477.5807')"),
               (fixed(6, Fraction(9223372036854775807, 10000)), fixed(6, Fraction(9223372036854775806, 10000)),
                fixed(14, Fraction(9223372036854775807, 10000)))),
        Family("large-fixed", ("CLngLng('9223372036854775807')", "CDec('9223372036854775807')",
                               "CDec('9223372036854775806')"),
               (fixed(20, 9223372036854775807), fixed(14, 9223372036854775807), fixed(14, 9223372036854775806))),
        Family("boolean-byte", ("True", "CInt(-1)", "CByte(255)"),
               (fixed(11, -1), fixed(2, -1), fixed(17, 255))),
        Family("integral-float", ("CLng(1)", "CSng(1)", "CDbl(2)"),
               (fixed(3, 1), floating(4, "1"), floating(5, "2")), "floating"),
        Family("date-aliases", ("CDate(-0.25)", "CDate(0.25)", "CDate(1)"),
               (Cell(7, Fraction(1, 4)), Cell(7, Fraction(1, 4)), Cell(7, Fraction(1))), "date"),
        Family("date-numeric-refusal", ("CDate(1)", "CInt(1)", "CDbl(1)"), (), refusal="type mismatch"),
        Family("decimal-float-refusal", ("CDec('0.1')", "CCur('0.1')", "CDbl(0.1)"), (),
               refusal="numeric combination"),
        Family("large-integer-float-refusal", ("CLngLng('9007199254740993')", "CLngLng('9007199254740992')",
                                               "CDbl(1)"),
               (fixed(20, 9007199254740993), fixed(20, 9007199254740992), floating(5, "1")),
               refusal="precision", partial=True),
        Family("text-numeric-refusal", ("'1'", "CInt(1)", "LeftB('1',2)"), (), refusal="type mismatch"),
        Family("raw-text-ordinal", ("'1'", "LeftB('1',2)", "'2'"),
               (Cell(8, "1"), Cell(8, "1"), Cell(8, "2")), "text"),
    ]


def base_query(family, order, native_number=None, include_identity=True):
    if native_number is not None:
        ranking = f"IIf(Ordinal={order[0]},1,IIf(Ordinal={order[1]},2,3))"
        return "SELECT Ordinal AS Id,IIf(Kind,SingleValue,DoubleValue) AS K FROM NumericKeys " \
               f"WHERE Family={native_number} ORDER BY {ranking}"
    arms = []
    for number in order:
        expression = family.expressions[number - 1]
        alternatives = [candidate for candidate in family.expressions if candidate != expression]
        # An unevaluated cast condition retains runtime selector metadata. The
        # unused branch is original valid input, not an error-hiding expression.
        other = alternatives[0]
        identity = f"CInt({number}) AS Id," if include_identity else ""
        arms.append(f"SELECT {identity}IIf(CBool(True),{expression},{other}) AS K")
    return " UNION ALL ".join(arms)


def view(family, name="K"):
    return f"CDbl({name})" if family.kind in ("floating", "date") else name


def statements(family, base, flat_union=None, native_number=None, order=None):
    source = f"({base}) AS Numbers"
    key = view(family)
    grouped = f"SELECT K,Count(*) AS N FROM {source} GROUP BY K"
    unique = f"SELECT DISTINCT K FROM {source}"
    union = f"SELECT K FROM {source} UNION SELECT K FROM ({base}) AS EmptyArm WHERE False"
    if flat_union is not None:
        a, b, _ = family.expressions
        union = f"{flat_union} UNION SELECT IIf(CBool(True),{a},{b}) AS K WHERE False"
    if native_number is not None:
        arms = ["SELECT IIf(Kind,SingleValue,DoubleValue) AS K FROM NumericKeys "
                f"WHERE Family={native_number} AND Ordinal={ordinal}" for ordinal in order]
        arms.append("SELECT IIf(Kind,SingleValue,DoubleValue) AS K FROM NumericKeys WHERE False")
        union = " UNION ALL ".join(arms[:-1]) + " UNION " + arms[-1]
    return {
        "group": f"SELECT VarType(K) AS T,{key} AS V,N FROM ({grouped}) AS Groups",
        "distinct": f"SELECT VarType(K) AS T,{key} AS V FROM ({unique}) AS UniqueRows",
        "union": f"SELECT VarType(K) AS T,{key} AS V FROM ({union}) AS UnionRows",
        "order": f"SELECT Id,VarType(K) AS T,{key} AS V FROM {source} ORDER BY K",
        "top": f"SELECT TOP 1 Id,VarType(K) AS T,{key} AS V FROM {source} ORDER BY K DESC",
        "extrema": "SELECT VarType(L) AS LT," + view(family, "L") + " AS L,VarType(H) AS HT,"
                   + view(family, "H") + f" AS H FROM (SELECT Min(K) AS L,Max(K) AS H FROM {source}) AS Ends",
        "pivot_rows": f"TRANSFORM Count(*) SELECT K,VarType(K) AS T,{key} AS V "
                      f"FROM {source} GROUP BY K PIVOT 7",
        "pivot_columns": f"TRANSFORM Count(*) SELECT 7 AS G FROM {source} GROUP BY 7 PIVOT K",
    }


def decode(family, tag, text):
    tag = int(tag)
    if family.kind in ("floating", "date") or tag in (4, 5):
        bits = int.from_bytes(struct.pack(">d", float(text)), "big")
        number = ieee(bits, 64)
        if family.kind == "date":
            whole = int(number)
            return Cell(tag, whole + abs(number - whole))
        return Cell(tag, number, bits if tag in (4, 5) else None)
    if tag == 8:
        return Cell(tag, json.loads(text))
    if tag == 11:
        assert text in ("TRUE", "FALSE")
        return fixed(tag, -1 if text == "TRUE" else 0)
    return fixed(tag, Fraction(text))


def valid_representative(family, cell):
    assert cell in family.cells, (family.name, cell, family.cells)


def validate(family, consumer, table):
    header, *rows = table
    headers = {
        "group": ["T", "V", "N"], "distinct": ["T", "V"], "union": ["T", "V"],
        "order": ["Id", "T", "V"], "top": ["Id", "T", "V"],
        "extrema": ["LT", "L", "HT", "H"], "pivot_rows": ["K", "T", "V", "7"],
    }
    if consumer in headers:
        assert header == headers[consumer], (family.name, consumer, header)
    expected = Counter(cell.key for cell in family.cells)
    if consumer in ("group", "pivot_rows"):
        actual = {}
        for row in rows:
            tag, text, count = row if consumer == "group" else row[1:]
            cell = decode(family, tag, text)
            valid_representative(family, cell)
            assert cell.key not in actual, (family.name, consumer, rows)
            actual[cell.key] = int(count)
        assert actual == expected, (family.name, consumer, actual, expected)
    elif consumer in ("distinct", "union"):
        actual = []
        for tag, text in rows:
            cell = decode(family, tag, text)
            valid_representative(family, cell)
            actual.append(cell.key)
        assert Counter(actual) == Counter({key: 1 for key in expected}), (family.name, consumer, rows)
    elif consumer in ("order", "top"):
        actual = []
        identities = []
        for identity, tag, text in rows:
            identity = int(identity)
            cell = decode(family, tag, text)
            assert cell == family.cells[identity - 1], (family.name, consumer, identity, cell)
            actual.append(cell.key)
            identities.append(identity)
        keys = [cell.key for cell in family.cells]
        if consumer == "order":
            assert actual == sorted(keys) and sorted(identities) == [1, 2, 3], (family.name, rows)
        else:
            maximum = max(keys)
            wanted = [index + 1 for index, cell in enumerate(family.cells) if cell.key == maximum]
            assert actual == [maximum] * len(wanted) and sorted(identities) == wanted, (family.name, rows)
    elif consumer == "extrema":
        assert len(rows) == 1
        low = decode(family, *rows[0][:2])
        high = decode(family, *rows[0][2:])
        valid_representative(family, low)
        valid_representative(family, high)
        assert (low.key, high.key) == (min(expected), max(expected)), (family.name, rows)
    else:
        assert consumer == "pivot_columns" and len(rows) == 1
        assert header[0] == "G" and int(rows[0][0]) == 7
        keys = sorted(expected)
        assert len(header) == len(keys) + 1, (family.name, header, keys)
        for name, count, key in zip(header[1:], rows[0][1:], keys, strict=True):
            assert Fraction(name) in heading_labels(family, key), (family.name, header, rows)
            assert int(count) == expected[key], (family.name, header, rows)


def native_records(text):
    schema = []
    rows = {}
    for line in text.splitlines():
        fields = line.split("\t")
        if fields[0] == "C":
            schema.append(fields[1:])
        elif fields[0] == "R":
            identity, family, ordinal = map(int, fields[1:4])
            assert identity == family * 10 + ordinal
            kind, single, real = fields[4:]
            assert kind in ("true", "false")
            selected = single if kind == "true" else real
            assert (real if kind == "true" else single) == "N"
            tag = 4 if kind == "true" else 5
            assert selected.startswith("S" if tag == 4 else "D")
            assert ordinal not in rows.setdefault(family, {}), "Duplicate original getter identity"
            rows.setdefault(family, {})[ordinal] = floating(tag, bits=int(selected[1:], 16))
        else:
            assert fields == ["O", "0"]
    assert schema == [["ID", "LONG"], ["Family", "LONG"], ["Ordinal", "LONG"], ["Kind", "BOOLEAN"],
                      ["SingleValue", "FLOAT"], ["DoubleValue", "DOUBLE"]]
    assert set(rows) == {1, 2, 3, 4}
    return [Family(f"native-{number}", (), tuple(rows[number][index] for index in (1, 2, 3)), "floating")
            for number in (1, 2, 3, 4)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "baseline_cli", "jar", "directory", "audit", "report"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    parser.add_argument("--source", type=Path, action="append", default=[])
    args = parser.parse_args()
    model_count = model_controls()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.jar) == JAR_SHA256
    assert digest(args.baseline_cli) == BASELINE_SHA256
    paths = {args.cli, args.baseline_cli, args.jar, args.audit, Path(__file__), *args.source,
             ROOT / "docs/testing/NumericKeyFixture.java", ROOT / "docs/testing/NumericKeyOracle.java"}
    hashes = {path: digest(path) for path in paths}
    prior = json.loads(args.audit.read_text())
    prior_binary = ROOT / prior["binary"]
    assert digest(prior_binary) == prior["sha256_before"] == prior["sha256_after"]
    hashes[prior_binary] = digest(prior_binary)
    binary_names = ("mixed-float", "subnormal-float", "large-float")
    assert sum(prior["summary"][name]["disagreements"] for name in binary_names) == 70
    assert prior["summary"]["decimal-float-refusal"]["disagreements"] == 48
    assert prior["summary"]["large-integer-float-refusal"]["disagreements"] == 4
    wording = [row for row in prior["observations"] if row["family"] == "decimal-float-refusal"]
    assert len(wording) == 48
    assert all(row["exit"] != 0 and not row["stdout"] and "numeric combination" in row["stderr"] for row in wording)
    avoided = [row for row in prior["observations"]
               if row["family"] == "large-integer-float-refusal" and not row["agrees"]]
    assert len(avoided) == 4
    for row in avoided:
        assert row["consumer"] == "top" and row["exit"] == 0 and not row["stderr"]
        assert row["stdout"].splitlines() == ["K", "9007199254740993"]
    observations = []
    native_getters = {}
    totals = Counter()

    def run(family, order, declared, consumer, sql, database=None, baseline=False, expected_error=None,
            validator=None):
        binary = args.baseline_cli if baseline else args.cli
        command = [str(binary), "--format", "tsv", "query", "run", sql,
                   "--number-format", "invariant", "--collation", "ordinal"]
        if declared:
            command.append("--declared-arithmetic")
        if database is not None:
            command += ["--database", str(database)]
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", timeout=120)
        agreed = False
        classification = "successful exact key model"
        refusal = expected_error or family.refusal
        if result.returncode:
            assert not result.stdout, (command, result)
            if refusal and refusal in result.stderr.lower():
                agreed = True
                classification = (
                    "retained heading-name refusal" if expected_error else "retained per-comparison refusal"
                )
        elif refusal and not family.partial:
            agreed = False
        else:
            table = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            try:
                validate(family, validator or consumer, table)
                agreed = not result.stderr
            except (AssertionError, ValueError, OverflowError, IndexError):
                if not baseline:
                    raise
            if family.partial:
                classification = "correct result without global comparison-domain preflight"
        observations.append({
            "family": family.name, "order": order, "declared": declared, "consumer": consumer,
            "baseline": baseline, "database": str(database) if database else None,
            "command": command,
            "classification": classification if agreed else "differs from independent exact key model",
            "agrees": agreed,
            "returncode": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
        })
        totals["baseline" if baseline else "candidate"] += 1
        if not baseline:
            assert agreed, observations[-1]
        return result

    for family in families():
        for order, declared in itertools.product(ORDERS, (False, True)):
            sql = statements(family, base_query(family, order), base_query(family, order, include_identity=False))
            consumers = CONSUMERS
            if family.kind == "floating":
                consumers += ("pivot_columns",)
            for consumer in consumers:
                run(family, order, declared, consumer, sql[consumer])
                if family.name in binary_names and consumer in ("group", "distinct", "top", "union"):
                    run(family, order, declared, consumer, sql[consumer], baseline=True)

    boolean = Family("retained-scalar-predicate", (), (fixed(11, -1),))
    static = Family("retained-static-union", (), (floating(5, "1"), floating(5, "1.00000001")), "floating")
    overflow = Family("retained-scalar-overflow", (), (), refusal="overflow")
    for declared in (False, True):
        sql = "SELECT VarType(E) AS T,E AS V FROM (SELECT CSng(1)=CDbl(1.00000001) AS E) AS Predicate"
        run(boolean, (), declared, "scalar_predicate", sql, validator="distinct")
        sql = "SELECT VarType(K) AS T,CDbl(K) AS V FROM "
        sql += "(SELECT CSng(1) AS K UNION SELECT CDbl(1.00000001) AS K) AS StaticUnion"
        run(static, (), declared, "static_union", sql, validator="distinct")
        run(overflow, (), declared, "scalar_overflow", "SELECT CSng(3e38)=CDbl(1e39) AS E")

    for profile in PROFILES:
        database = args.directory / (profile + (".mdb" if profile == "V2000" else ".accdb"))
        getter = database.with_suffix(".tsv")
        for path in (database, getter):
            hashes[path] = digest(path)
        command = ["java", "-cp", str(args.jar), str(ROOT / "docs/testing/NumericKeyOracle.java"), str(database)]
        result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", check=True)
        assert result.stdout == getter.read_text(), "Original getter records changed"
        native_getters[profile] = result.stdout
        for number, family in enumerate(native_records(result.stdout), 1):
            for order, declared in itertools.product(ORDERS, (False, True)):
                sql = statements(family, base_query(family, order, number), native_number=number, order=order)
                for consumer in CONSUMERS:
                    run(family, order, declared, consumer, sql[consumer], database)
                run(family, order, declared, "pivot_columns", sql["pivot_columns"], database,
                    expected_error="ambiguous" if heading_collision(family) else None)
            low = f"DMin('IIf(Kind,SingleValue,DoubleValue)','NumericKeys','Family={number}')"
            high = f"DMax('IIf(Kind,SingleValue,DoubleValue)','NumericKeys','Family={number}')"
            for declared in (False, True):
                sql = f"SELECT VarType({low}) AS LT,CDbl({low}) AS L,VarType({high}) AS HT,CDbl({high}) AS H"
                run(family, (), declared, "domain_extrema", sql, database, validator="extrema")
        for declared in (False, True):
            sql = "SELECT VarType(E) AS T,E AS V FROM (SELECT s.SingleValue=d.DoubleValue AS E "
            sql += "FROM NumericKeys s INNER JOIN NumericKeys d ON s.Family=d.Family "
            sql += "WHERE s.Family=1 AND s.Ordinal=2 AND d.Ordinal=3) AS Predicate"
            run(boolean, (), declared, "native_scalar_predicate", sql, database, validator="distinct")
            sql = "SELECT VarType(K) AS T,CDbl(K) AS V FROM "
            sql += "(SELECT SingleValue AS K FROM NumericKeys WHERE Family=1 AND Ordinal=2 "
            sql += "UNION SELECT DoubleValue AS K FROM NumericKeys WHERE Family=1 AND Ordinal=3) AS StaticUnion"
            run(static, (), declared, "native_static_union", sql, database, validator="distinct")

    for path, expected in hashes.items():
        assert digest(path) == expected, path
    report = {
        "scope": "Independent exact engineering keys; native Access grouping unqualified",
        "model_controls": model_count,
        "runner_arguments": sys.argv[1:], "controls": dict(totals),
        "hashes": {str(path.resolve().relative_to(ROOT)): expected for path, expected in sorted(hashes.items())},
        "prior_audit": prior, "native_getters": native_getters, "observations": observations,
        "prior_audit_classification": {
            "binary_key_disagreements": 70,
            "fixed_float_diagnostic_wording_only": 48,
            "correct_top_results_without_global_domain_preflight": 4,
            "native_access_qualification": False,
        },
    }
    args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"controls": dict(totals), "hashes": len(hashes)}))


if __name__ == "__main__":
    main()
