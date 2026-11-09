"""Replay exact mixed fixed/binary query keys over unchanged native scalar inputs.

Public getters supply stored values. Python Fraction/IEEE models supply expected
partitions and ordering; only TableCraft executes SQL. Earlier results and the
historical numeric-key runner remain unchanged.
"""

import argparse
import csv
from dataclasses import dataclass
from fractions import Fraction
import hashlib
import io
import itertools
import json
from pathlib import Path
import struct
import subprocess

import compare_numeric_keys as model
from compare_declared_numeric_text import INPUTS, JAR_SHA256

ROOT = Path(__file__).resolve().parents[2]
MAX_COMMANDS = 5000
SOURCE_ROOTS = [
    "crates/query/tablecraft-expressions/src/comparison.rs",
    "crates/query/tablecraft-query/src/execution/keys.rs",
    "crates/query/tablecraft-query/src/execution/keys",
    "crates/query/tablecraft-query/src/execution/values.rs",
    "crates/query/tablecraft-query/src/execution/aggregate/extrema.rs",
    "crates/query/tablecraft-query/src/execution/aggregate/extrema",
    "crates/query/tablecraft-query/src/execution/exact_keys",
]


@dataclass(frozen=True)
class Case:
    family: model.Family
    native_row: int | None = None


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def family(name, expressions, cells):
    # Direct output preserves exact fixed coefficients and original carriers.
    # Single transport spelling is decoded at binary32 precision by the model.
    return model.Family(name, tuple(expressions), tuple(cells))


def fixed_cases():
    f = model.fixed
    b = model.floating
    return [
        Case(family("positive-tenth", ["CDec('0.1')", "CCur('0.1')", "CDbl(0.1)"],
                    [f(14, Fraction(1, 10)), f(6, Fraction(1, 10)), b(5, "0.1")])),
        Case(family("negative-tenth", ["CDec('-0.1')", "CCur('-0.1')", "CDbl(-0.1)"],
                    [f(14, Fraction(-1, 10)), f(6, Fraction(-1, 10)), b(5, "-0.1")])),
        Case(family("dyadic-half", ["CCur('0.5')", "CDec('0.50')", "CSng(0.5)"],
                    [f(6, Fraction(1, 2)), f(14, Fraction(1, 2)), b(4, "0.5")])),
        Case(family("large-integer", ["CLngLng('9007199254740993')", "CLngLng('9007199254740992')", "CDbl(9007199254740992)"],
                    [f(20, 9007199254740993), f(20, 9007199254740992), b(5, "9007199254740992")])),
        Case(family("decimal-limit", ["CDec('79228162514264337593543950335')", "CDbl(79228162514264337593543950336)", "CDec('79228162514264337593543950334')"],
                    [f(14, 79228162514264337593543950335), b(5, bits=0x45F0000000000000), f(14, 79228162514264337593543950334)])),
        Case(family("smallest-binary", ["CDec('0.0000000000000000000000000001')", "CDbl(4.9406564584124654e-324)", "CDbl(0)"],
                    [f(14, Fraction(1, 10**28)), b(5, bits=1), b(5, "0")])),
        Case(family("signed-zeros", ["CCur(0)", "CDec('0.0000')", "(-CSng(0))"],
                    [f(6, 0), f(14, 0), b(4, bits=0x80000000)])),
        Case(family("boolean-minus-one", ["True", "CCur(-1)", "CSng(-1)"],
                    [f(11, -1), f(6, -1), b(4, "-1")])),
        Case(family("longlong-upper", ["CLngLng('9223372036854775807')", "CDbl(9223372036854775808)", "CDec('9223372036854775807')"],
                    [f(20, 9223372036854775807), b(5, bits=0x43E0000000000000), f(14, 9223372036854775807)])),
        Case(family("longlong-lower", ["CLngLng('-9223372036854775808')", "CDbl(-9223372036854775808)", "CDec('-9223372036854775808')"],
                    [f(20, -9223372036854775808), b(5, bits=0xC3E0000000000000), f(14, -9223372036854775808)])),
        Case(family("three-tenths", ["CCur('0.1')", "CSng(0.1)", "CDbl(0.1)"],
                    [f(6, Fraction(1, 10)), b(4, "0.1"), b(5, "0.1")])),
        Case(family("decimal-between-doubles", ["CDec('1.0000000000000000000000000001')", "CDbl(1)", "CDbl(1.0000000000000002)"],
                    [f(14, Fraction('1.0000000000000000000000000001')), b(5, "1"), b(5, "1.0000000000000002")])),
        Case(family("decimal-near-double", ["CDec('1.0000000000000002220446049250')", "CDbl(1.0000000000000002)", "CDec('1.0000000000000002220446049251')"],
                    [f(14, Fraction('1.0000000000000002220446049250')), b(5, "1.0000000000000002"), f(14, Fraction('1.0000000000000002220446049251'))])),
        Case(family("decimal-around-tenth", ["CDec('0.1000000000000000055511151231')", "CDbl(0.1)", "CDec('0.1000000000000000055511151232')"],
                    [f(14, Fraction('0.1000000000000000055511151231')), b(5, "0.1"), f(14, Fraction('0.1000000000000000055511151232'))])),
        Case(family("currency-small-floats", ["CCur('0.0001')", "CSng(0.0001)", "CDbl(0.0001)"],
                    [f(6, Fraction(1, 10000)), b(4, "0.0001"), b(5, "0.0001")])),
    ]


def native_cell(text):
    if text == "N":
        return None
    if text.startswith("I16:"):
        return model.fixed(2, int(text[4:]))
    if text.startswith("I32:"):
        return model.fixed(3, int(text[4:]))
    prefix, value = text[0], text[1:]
    if prefix == "B":
        return model.fixed(11, -int(value))
    if prefix == "U":
        return model.fixed(17, int(value))
    if prefix == "C":
        return model.fixed(6, Fraction(int(value), 10000))
    if prefix == "Q":
        coefficient, scale = value.split(":")
        return model.fixed(14, Fraction(int(coefficient), 10**int(scale)))
    if prefix in ("F", "D"):
        return model.floating(4 if prefix == "F" else 5, bits=int(value, 16))
    raise ValueError("Unexpected getter carrier")


def native_rows(text):
    columns = [("ID", 0), ("Flag", 1), ("Octet", 2), ("Small", 3),
               ("Money", 4), ("Real", 5), ("WideReal", 6), ("Amount", 12)]
    result = {}
    for line in text.splitlines():
        table, *fields = line.split("\t")
        if table != "ScalarSamples" or len(fields) != 14:
            raise ValueError("Unexpected original getter shape")
        row = {name: native_cell(fields[index]) for name, index in columns}
        identity = int(row["ID"].key)
        if identity in result:
            raise ValueError("Duplicate original identity")
        result[identity] = row
    if set(result) != {1, 2, 3}:
        raise ValueError("Original identities differ")
    return result


def native_cases(rows):
    def direct(name, identity, fields):
        expressions = [f"[{field}]" for field in fields]
        return Case(family(name, expressions, [rows[identity][field] for field in fields]), identity)

    def converted(name, identity, first, third):
        value = rows[identity][first]
        binary = model.floating(5, str(float(value.key)))
        return Case(family(name, [f"[{first}]", f"CDbl([{first}])", f"[{third}]"],
                           [value, binary, rows[identity][third]]), identity)

    return [
        direct("native-money-wide-decimal", 1, ["Money", "WideReal", "Amount"]),
        direct("native-money-single-decimal", 2, ["Money", "Real", "Amount"]),
        direct("native-boolean-single-byte", 1, ["Flag", "Real", "Octet"]),
        direct("native-zero-money-short", 2, ["WideReal", "Money", "Small"]),
        converted("native-large-decimal-double", 1, "Amount", "Money"),
        converted("native-negative-decimal-double", 2, "Amount", "Money"),
        converted("native-money-rounded-double", 1, "Money", "WideReal"),
        direct("native-false-zero-money", 2, ["Flag", "WideReal", "Money"]),
    ]


def base(case, order, identity=True):
    if case.native_row is None:
        inputs = family(case.family.name, ["P1", "P2", "P3"], case.family.cells)
        return model.base_query(inputs, order, include_identity=identity)
    arms = []
    expressions = case.family.expressions
    for index in order:
        expression = expressions[index - 1]
        other = next(value for value in expressions if value != expression)
        selected = f"IIf(CBool(True),{expression},{other}) AS K"
        identifier = f"CInt({index}) AS Id," if identity else ""
        arms.append(f"SELECT {identifier}{selected} FROM ScalarSamples S WHERE S.ID={case.native_row}")
    return " UNION ALL ".join(arms)


def statements(case, order):
    source = f"({base(case, order)}) AS Numbers"
    group = f"SELECT K,Count(*) AS N FROM {source} GROUP BY K"
    distinct = f"SELECT DISTINCT K FROM {source}"
    first, second, _ = case.family.expressions if case.native_row is not None else ["P1", "P2", "P3"]
    empty = f"SELECT IIf(CBool(True),{first},{second}) AS K"
    if case.native_row is not None:
        empty += " FROM ScalarSamples"
    union = f"{base(case, order, False)} UNION {empty} WHERE False"
    return {
        "group": f"SELECT VarType(K) AS T,K AS V,N FROM ({group}) G",
        "distinct": f"SELECT VarType(K) AS T,K AS V FROM ({distinct}) D",
        "union": f"SELECT VarType(K) AS T,K AS V FROM ({union}) U",
        "order": f"SELECT Id,VarType(K) AS T,K AS V FROM {source} ORDER BY K",
        "top": f"SELECT TOP 1 Id,VarType(K) AS T,K AS V FROM {source} ORDER BY K DESC",
        "extrema": f"SELECT VarType(L) AS LT,L AS L,VarType(H) AS HT,H AS H "
                   f"FROM (SELECT Min(K) AS L,Max(K) AS H FROM {source}) Ends",
        "pivot_rows": f"TRANSFORM Count(*) SELECT K,VarType(K) AS T,K AS V "
                      f"FROM {source} GROUP BY K PIVOT 7",
    }




def fixed_literal(value):
    negative = value < 0
    value = abs(value)
    whole, remainder = divmod(value.numerator, value.denominator)
    digits = []
    while remainder:
        remainder *= 10
        digit, remainder = divmod(remainder, value.denominator)
        digits.append(str(digit))
        if len(digits) > 28:
            raise ValueError("Fixed parameter exceeds the modeled decimal scale")
    result = str(whole)
    if digits:
        result += "." + "".join(digits)
    return "-" + result if negative else result


def parameter(index, cell):
    kinds = {17: "byte", 2: "short", 3: "long", 20: "bigint", 6: "currency", 14: "decimal",
             4: "single", 5: "double", 11: "boolean"}
    if cell.tag in (4, 5):
        text = repr(struct.unpack(">d", cell.bits.to_bytes(8, "big"))[0])
    elif cell.tag == 11:
        if cell.key not in (0, -1):
            raise ValueError("Invalid modeled Boolean")
        text = "true" if cell.key == -1 else "false"
    else:
        text = fixed_literal(cell.key)
    return f"P{index}={kinds[cell.tag]}:{text}"


def transport_table(table, consumer):
    # Rust's shortest Single spelling round-trips to binary32. Recover that
    # stored value before using the historical validator's exact binary64 view.
    # SQL observes the original key and original VarType without any extra cast.
    pairs = {
        "group": [(0, 1)], "distinct": [(0, 1)], "union": [(0, 1)],
        "order": [(1, 2)], "top": [(1, 2)],
        "extrema": [(0, 1), (2, 3)], "pivot_rows": [(1, 2)],
    }[consumer]
    normalized = [table[0]]
    for original in table[1:]:
        row = original.copy()
        for tag, value in pairs:
            if row[tag] == "4":
                binary = struct.unpack(">f", struct.pack(">f", float(row[value])))[0]
                row[value] = repr(binary)
        normalized.append(row)
    return normalized


def snapshots(revision):
    listing = subprocess.run(["git", "ls-tree", "-r", "--name-only", revision, "--", *SOURCE_ROOTS],
                             check=True, capture_output=True, text=True)
    result = {}
    for name in listing.stdout.splitlines():
        if name.endswith(".rs"):
            content = subprocess.run(["git", "show", f"{revision}:{name}"], check=True, capture_output=True).stdout
            result[name] = hashlib.sha256(content).hexdigest()
    if not result:
        raise ValueError("Missing selected source snapshots")
    return result


class Replay:
    def __init__(self, args):
        self.args = args
        self.observations = []
        self.rows = 0
        self.cells = 0

    def run(self, case, order, declared, consumer, sql, database=None, baseline=False,
            refusal=None, validator=None):
        if len(self.observations) >= MAX_COMMANDS:
            raise ValueError("Command ceiling exceeded")
        binary = self.args.baseline if baseline else self.args.cli
        parameter_values = []
        if database is None:
            sql = "PARAMETERS P1 VALUE,P2 VALUE,P3 VALUE; " + sql
            parameter_values = [parameter(index, cell) for index, cell in enumerate(case.family.cells, 1)]
        command = [binary, "--format", "tsv", "query", "run", sql,
                   "--number-format", "invariant", "--collation", "ordinal"]
        for value in parameter_values:
            command.extend(["--param", value])
        if declared:
            command.append("--declared-arithmetic")
        if database:
            command.extend(["--database", database])
        result = subprocess.run([str(value) for value in command], capture_output=True,
                                text=True, encoding="utf-8", timeout=120)
        observation = {
            "case": case.family.name, "consumer": consumer, "order": list(order),
            "declared": declared, "database": database.name if database else None,
            "baseline": baseline, "exit_code": result.returncode,
            "stdout_sha256": hashlib.sha256(result.stdout.encode()).hexdigest(),
            "stderr_sha256": hashlib.sha256(result.stderr.encode()).hexdigest(),
        }
        self.observations.append(observation)
        if refusal or (baseline and result.returncode):
            permitted = [refusal] if refusal else ["numeric combination", "precision"]
            if result.returncode != 2 or result.stdout or not any(word in result.stderr.lower() for word in permitted):
                raise AssertionError((observation, result.stderr))
            observation["refusal"] = refusal or "prior mixed-key refusal"
            observation["rows"] = 0
            observation["cells"] = 0
        else:
            if result.returncode or result.stderr:
                raise AssertionError((observation, result.stderr, sql))
            table = list(csv.reader(io.StringIO(result.stdout), delimiter="\t", strict=True))
            target = validator or consumer
            model.validate(case.family, target, transport_table(table, target))
            observation["rows"] = len(table) - 1
            observation["cells"] = sum(len(row) for row in table[1:])
            if not baseline:
                self.rows += observation["rows"]
                self.cells += observation["cells"]
        if len(self.observations) % 256 == 0:
            print(f"Checked {len(self.observations)} CLI commands", flush=True)

    def case(self, case, database=None):
        for order, declared in itertools.product(model.ORDERS, [False, True]):
            for consumer, sql in statements(case, order).items():
                self.run(case, order, declared, consumer, sql, database)
                if order == model.ORDERS[0] and not declared:
                    self.run(case, order, declared, consumer, sql, database, baseline=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "baseline", "jar", "directory", "report"]:
        parser.add_argument("--" + name, required=True, type=Path)
    for name in ["cli-sha256", "baseline-sha256", "revision"]:
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise ValueError("Report must be fresh")
    if digest(args.jar) != JAR_SHA256 or digest(args.cli) != args.cli_sha256 or digest(args.baseline) != args.baseline_sha256:
        raise ValueError("Reference/executable fingerprint differs")
    for name, expected in INPUTS.items():
        if digest(args.directory / name) != expected:
            raise ValueError("Original native fingerprint differs")
    tools = [args.cli, args.baseline, args.jar, Path(__file__),
             ROOT / "docs/testing/compare_numeric_keys.py", ROOT / "docs/testing/compare_declared_numeric_text.py",
             ROOT / "docs/testing/TableDataOracle.java", ROOT / "docs/testing/ScalarFixture.java",
             *(args.directory / name for name in INPUTS)]
    pins = {str(path.resolve().relative_to(ROOT)): digest(path) for path in tools}
    revision = subprocess.run(["git", "rev-parse", "--verify", f"{args.revision}^{{commit}}"],
                              check=True, capture_output=True, text=True).stdout.strip()
    source_pins = snapshots(revision)
    report = {
        "date": "2026-10-25", "revision": revision, "scope": __doc__,
        "immutable_before": pins, "selected_source_sha256": source_pins,
        "model_goldens": model.model_controls(), "getter_records": {},
        "observations": [], "complete": False,
    }
    replay = Replay(args)
    report["observations"] = replay.observations
    try:
        for case in fixed_cases():
            replay.case(case)
        print("All source-free mixed fixed/binary families pass", flush=True)
        for name in INPUTS:
            database = args.directory / name
            getter = subprocess.run(["java", "-Xmx256m", "-cp", args.jar,
                                     ROOT / "docs/testing/TableDataOracle.java", database, "ScalarSamples"],
                                    capture_output=True, text=True, encoding="utf-8", check=True, timeout=120)
            if getter.stderr:
                raise ValueError("Unexpected getter diagnostics")
            rows = native_rows(getter.stdout)
            report["getter_records"][name] = {
                "stdout_sha256": hashlib.sha256(getter.stdout.encode()).hexdigest(),
                "rows": len(rows),
                "numeric_values": {str(identity): {column: None if value is None else {
                    "tag": value.tag, "numerator": str(value.key.numerator),
                    "denominator": str(value.key.denominator), "binary64_bits": value.bits,
                } for column, value in row.items()} for identity, row in rows.items()},
            }
            for case in native_cases(rows):
                replay.case(case, database)
            expression = "IIf([ID]=1,[Money],[WideReal])"
            low = f"DMin('{expression}','ScalarSamples','ID<3')"
            high = f"DMax('{expression}','ScalarSamples','ID<3')"
            case = Case(family("native-domain-extrema", [], [rows[1]["Money"], rows[2]["WideReal"]]))
            sql = f"SELECT VarType({low}) AS LT,{low} AS L,VarType({high}) AS HT,{high} AS H"
            for declared in [False, True]:
                replay.run(case, (), declared, "domain_extrema", sql, database, validator="extrema")
                if not declared:
                    replay.run(case, (), declared, "domain_extrema", sql, database, baseline=True, validator="extrema")
            print(f"{name}: all native getter-driven families and domains pass", flush=True)
        report["immutable_after"] = {path: digest(ROOT / path) for path in pins}
        if report["immutable_after"] != pins or snapshots(revision) != source_pins:
            raise ValueError("Pinned artifact or versioned source changed")
        report["complete"] = True
    finally:
        report["cli_commands"] = len(replay.observations)
        report["candidate_commands"] = sum(not item["baseline"] for item in replay.observations)
        report["baseline_commands"] = sum(item["baseline"] for item in replay.observations)
        report["baseline_refusals"] = sum(item["baseline"] and item["exit_code"] != 0 for item in replay.observations)
        report["compared_rows"] = replay.rows
        report["compared_cells"] = replay.cells
        args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Passed {report['cli_commands']} CLI commands, {replay.rows} rows and {replay.cells} cells", flush=True)


if __name__ == "__main__":
    main()
