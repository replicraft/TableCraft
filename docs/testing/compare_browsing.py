"""Compare CLI table browsing against independent Jackcess scalar snapshots.

Uses the reviewed Jackcess artifact and original ScalarFixture recipe. Generated
files remain local. This establishes independent reading, not native Access SQL
or shared-file qualification. Scalar float formatting covers this recipe's exact
binary fractions only; it is not a general cross-language float formatter.
"""

from cli_tsv import command_output
import argparse
import calendar
import datetime
import hashlib
from pathlib import Path
import struct
import subprocess
import uuid

PROFILES = [("V2000", "mdb"), ("V2007", "accdb"), ("V2016", "accdb"), ("V2019", "accdb")]
RECIPES = Path(__file__).resolve().parent


def run(*args):
    return command_output(args, subprocess.check_output(list(map(str, args)), encoding="utf-8"))


def escaped(text):
    simple = {"\\": "\\\\", '"': '\\"', "\t": "\\t", "\n": "\\n", "\r": "\\r"}
    return "".join(simple.get(ch, f"\\u{{{ord(ch):04x}}}" if ord(ch) < 32 or 127 <= ord(ch) <= 159 else ch) for ch in text)


def fixed(coefficient, scale):
    negative = coefficient.startswith("-")
    digits = coefficient.lstrip("-").zfill(scale + 1)
    result = digits if scale == 0 else digits[:-scale] + "." + digits[-scale:]
    return ("-" if negative else "") + result


def display(tag):
    if tag == "N":
        return "NULL"
    if tag.startswith("B"):
        return {"B0": "FALSE", "B1": "TRUE"}[tag]
    if tag.startswith("I"):
        return tag.split(":", 1)[1]
    if tag.startswith("U"):
        return tag[1:]
    if tag.startswith("C"):
        return fixed(tag[1:], 4)
    if tag.startswith("Q"):
        coefficient, scale = tag[1:].split(":")
        return fixed(coefficient, int(scale))
    if tag.startswith(("F", "D")):
        single = tag[0] == "F"
        value = struct.unpack(">f" if single else ">d", bytes.fromhex(tag[1:]))[0]
        return format(value, ".9g" if single else ".17g")
    if tag.startswith("T"):
        return '"' + escaped(bytes.fromhex(tag[1:]).decode("utf-8")) + '"'
    if tag.startswith("X"):
        return "0x" + tag[1:]
    if tag.startswith("G"):
        return "{" + str(uuid.UUID(tag[1:])) + "}"
    if tag.startswith(("Y", "Z")):
        return "#" + tag[1:] + "#"
    raise ValueError("Unsupported oracle tag")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--profile", choices=["V2000", "V2007", "V2016", "V2019"])
    args = parser.parse_args()
    profiles = [(args.profile, "mdb" if args.profile == "V2000" else "accdb")] if args.profile else PROFILES
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in profiles:
            run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "ScalarFixture.java", args.directory / f"Scalar-{profile}.{suffix}", profile)
    for profile, suffix in profiles:
        path = args.directory / f"Scalar-{profile}.{suffix}"
        before = path.read_bytes()
        family = {"V2000": 1, "V2007": 2, "V2016": 5, "V2019": 6}[profile]
        assert before[20:24] == bytes([family, 0, 0, 0]), "Wrong original family"
        def oracle(recipe, *extra):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / f"{recipe}.java", path, *extra)
        catalog = [line.split("\t") for line in oracle("CatalogOracle").splitlines()]
        names = sorted(escaped(row[4]) + "\t" + ("System" if int(row[3]) & 0x80000002 else "User") for row in catalog if int(row[2]) in (1, 4, 6))
        actual = run(args.cli, "tables", path, "--show-system").splitlines()
        assert actual[0] == "Name\tKind" and sorted(actual[1:]) == names, "Table inventory mismatch"
        columns = [escaped(line.split("\t")[1]) for line in oracle("TableSchemaOracle", "ScalarSamples").splitlines()]
        header = "\t".join(columns) + "\n"
        tagged_rows = [line.split("\t")[1:] for line in oracle("TableDataOracle", "ScalarSamples").splitlines()]
        rows = ["\t".join(display(tag) for tag in row) + "\n" for row in tagged_rows]
        cases = [([], rows), (["--limit", "1"], rows[:1]), (["--offset", "1", "--limit", "1"], rows[1:2]), (["--offset", str(len(rows))], []), (["--offset", "4294967295"], [])]
        for options, selected in cases:
            assert run(args.cli, "rows", path, "ScalarSamples", *options) == header + "".join(selected), "Scalar page mismatch"
        # Reuse the native query path as an additional integration regression;
        # expected bytes still come from the independent Java row oracle above.
        assert run(args.cli, "query", "run", "SELECT * FROM ScalarSamples ORDER BY ID", "--database", path) == header + "".join(rows), "SELECT mismatch"
        # Each independent native column is also requested alone. This exercises
        # exact carriers after pruning every earlier/later physical scalar field.
        cells = [row.rstrip("\n").split("\t") for row in rows]
        quote = lambda name: "[" + name + "]"
        controls = 7
        for position, column in enumerate(columns):
            expected = column + "\n" + "".join(row[position] + "\n" for row in cells)
            sql = "SELECT " + quote(column) + " FROM ScalarSamples ORDER BY ID"
            assert run(args.cli, "query", "run", sql, "--database", path) == expected, (profile, column)
            controls += 1
            nulls = column + "\n" + "".join(("TRUE" if row[position] == "NULL" else "FALSE") + "\n" for row in cells)
            assert run(args.cli, "query", "run", "SELECT IsNull(" + quote(column) + ") AS " + quote(column) + " FROM ScalarSamples ORDER BY ID", "--database", path) == nulls, (profile, column, "IsNull")
            wrapped = "SELECT IIf(IsNull(" + quote(column) + "),Null," + quote(column) + ") AS " + quote(column) + " FROM ScalarSamples ORDER BY ID"
            assert run(args.cli, "query", "run", wrapped, "--database", path) == expected, (profile, column, "IIf")
            controls += 2
            for fallback, wanted in [(",Null", expected), ("", column + "\n" + "".join(('""' if row[position] == "NULL" else row[position]) + "\n" for row in cells))]:
                sql = "SELECT Nz(" + quote(column) + fallback + ") AS " + quote(column) + " FROM ScalarSamples ORDER BY ID"
                assert run(args.cli, "query", "run", sql, "--database", path) == wanted, (profile, column, "Nz", fallback)
                controls += 1
        # Unicode expectations are independent Python UTF-16 buffers from inert
        # tagged getters, without evaluating expressions in the reference engine.
        for position, column in enumerate(columns):
            if not any(row[position].startswith("T") for row in tagged_rows):
                continue
            for function, arguments in [("Len", ""), ("Trim", ""), ("LTrim", ""), ("RTrim", ""), ("Left", ",1"), ("Right", ",2"), ("Mid", ",2"), ("Mid", ",2,1")]:
                expected_cells = []
                for row in tagged_rows:
                    tag = row[position]
                    if tag == "N":
                        expected_cells.append("NULL")
                        continue
                    text = bytes.fromhex(tag[1:]).decode("utf-8")
                    units = text.encode("utf-16-le")
                    if function == "Len":
                        expected_cells.append(str(len(units) // 2))
                        continue
                    if function == "Trim": result = text.strip(" ")
                    elif function == "LTrim": result = text.lstrip(" ")
                    elif function == "RTrim": result = text.rstrip(" ")
                    elif function == "Left": result = units[:2].decode("utf-16-le")
                    elif function == "Right": result = units[-4:].decode("utf-16-le")
                    elif arguments == ",2": result = units[2:].decode("utf-16-le")
                    else: result = units[2:4].decode("utf-16-le")
                    expected_cells.append('"' + escaped(result) + '"')
                sql = "SELECT " + function + "(" + quote(column) + arguments + ") AS " + quote(column) + " FROM ScalarSamples ORDER BY ID"
                wanted = column + "\n" + "".join(cell + "\n" for cell in expected_cells)
                assert run(args.cli, "query", "run", sql, "--database", path) == wanted, (profile, column, function, arguments)
                controls += 1
        for function, attribute in [("Year", "year"), ("Month", "month"), ("Day", "day"), ("Hour", "hour"), ("Minute", "minute"), ("Second", "second")]:
            position = columns.index("Stamp")
            values = []
            for row in tagged_rows:
                tag = row[position]
                values.append("NULL" if tag == "N" else str(getattr(datetime.datetime.fromisoformat(tag[1:]), attribute)))
            wanted = "Stamp\n" + "".join(value + "\n" for value in values)
            sql = "SELECT " + function + "(Stamp) AS Stamp FROM ScalarSamples ORDER BY ID"
            assert run(args.cli, "query", "run", sql, "--database", path) == wanted, (profile, function)
            controls += 1
        for interval, count in [("yyyy", 1), ("q", -1), ("m", 1), ("y", 2.5), ("d", -1.5), ("w", 1), ("ww", -1), ("h", 25), ("n", 61), ("s", 61)]:
            parts = [("Year", "year"), ("Month", "month"), ("Day", "day"), ("Hour", "hour"), ("Minute", "minute"), ("Second", "second")]
            values = []
            for row in tagged_rows:
                tag = row[columns.index("Stamp")]
                if tag == "N":
                    values.append("\t".join(["NULL"] * len(parts)) + "\n")
                    continue
                date = datetime.datetime.fromisoformat(tag[1:])
                number = round(count)
                if interval in ("yyyy", "q", "m"):
                    months = number * {"yyyy": 12, "q": 3, "m": 1}[interval]
                    year, month = divmod(date.year * 12 + date.month - 1 + months, 12)
                    month += 1
                    date = date.replace(year=year, month=month, day=min(date.day, calendar.monthrange(year, month)[1]))
                else:
                    unit = {"y": "days", "d": "days", "w": "days", "ww": "weeks", "h": "hours", "n": "minutes", "s": "seconds"}[interval]
                    date += datetime.timedelta(**{unit: number})
                values.append("\t".join(str(getattr(date, attribute)) for _, attribute in parts) + "\n")
            projection = ",".join(function + "(DateAdd('" + interval + "'," + str(count) + ",Stamp)) AS Part" + str(i) for i, (function, _) in enumerate(parts))
            wanted = "\t".join("Part" + str(i) for i in range(len(parts))) + "\n" + "".join(values)
            assert run(args.cli, "query", "run", "SELECT " + projection + " FROM ScalarSamples ORDER BY ID", "--database", path) == wanted, (profile, "DateAdd", interval, count)
            controls += 1
        for interval, count in [("yyyy", 1), ("q", -1), ("m", 1), ("y", 2.5), ("d", -1.5), ("w", 8), ("ww", -1), ("h", 25), ("n", 61), ("s", 61)]:
            expected_count = round(count)
            if interval == "w": expected_count = int(expected_count / 7)
            wanted = "Difference\n" + "".join(("NULL" if row[columns.index("Stamp")] == "N" else str(expected_count)) + "\n" for row in tagged_rows)
            sql = "SELECT DateDiff('" + interval + "',Stamp,DateAdd('" + interval + "'," + str(count) + ",Stamp)) AS Difference FROM ScalarSamples ORDER BY ID"
            assert run(args.cli, "query", "run", sql, "--database", path) == wanted, (profile, "DateDiff", interval)
            controls += 1
        for first_day, first_week in [(1, 2), (2, 3)]:
            values = []
            for row in tagged_rows:
                tag = row[columns.index("Stamp")]
                if tag == "N": values.append("NULL\n"); continue
                start = datetime.datetime.fromisoformat(tag[1:]).date()
                target = (first_day + 5) % 7  # Python Monday=0; Access Sunday=1.
                count = sum((start + datetime.timedelta(days=step)).weekday() == target for step in range(1, 4))
                values.append(str(count) + "\n")
            options = ",," + str(first_week) if first_day == 1 else "," + str(first_day) + "," + str(first_week)
            sql = "SELECT DateDiff('ww',Stamp,DateAdd('d',3,Stamp)" + options + ") AS Difference FROM ScalarSamples ORDER BY ID"
            assert run(args.cli, "query", "run", sql, "--database", path) == "Difference\n" + "".join(values), (profile, "DateDiff", options)
            controls += 1
        def query(sql):
            return run(args.cli, "query", "run", sql, "--database", path)
        def expected(header, values):
            return header + "\n" + "".join("\t".join(value) + "\n" for value in values)
        cases = [
            ("SELECT Count(*) AS N FROM ScalarSamples", "N\n3\n"),
            ("SELECT 1 AS N FROM ScalarSamples", "N\n1\n1\n1\n"),
            ("SELECT Label, ID FROM ScalarSamples WHERE Small IS NOT NULL ORDER BY ID DESC", expected("Label\tID", [[row[7], row[0]] for row in reversed(cells[:2])])),
            ("SELECT A.ID, B.Label FROM ScalarSamples AS A LEFT JOIN ScalarSamples AS B ON A.ID=B.ID+1 ORDER BY A.ID", expected("ID\tLabel", [[row[0], "NULL" if i == 0 else cells[i-1][7]] for i, row in enumerate(cells)])),
            ("SELECT A.ID, (SELECT Count(*) FROM ScalarSamples AS B WHERE B.ID<A.ID) AS N FROM ScalarSamples AS A ORDER BY A.ID", expected("ID\tN", [[row[0], str(i)] for i, row in enumerate(cells)])),
            ("SELECT ID FROM ScalarSamples WHERE EXISTS (SELECT B.Label FROM ScalarSamples AS B WHERE B.ID=ScalarSamples.ID AND B.Label IS NOT NULL) ORDER BY ID", expected("ID", [[row[0]] for row in cells[:2]])),
            ("SELECT D.Label FROM (SELECT ID, Label FROM ScalarSamples) AS D WHERE D.ID=2", "Label\n" + cells[1][7] + "\n"),
            ("SELECT ID FROM ScalarSamples WHERE ID=1 UNION ALL SELECT ID FROM ScalarSamples WHERE ID=3 ORDER BY ID", expected("ID", [[cells[0][0]], [cells[2][0]]])),
            ("SELECT Flag, Count(*) AS N FROM ScalarSamples GROUP BY Flag ORDER BY Flag", "Flag\tN\nTRUE\t1\nFALSE\t2\n"),
            ("SELECT TOP 1 Label FROM ScalarSamples ORDER BY ID DESC", "Label\nNULL\n"),
        ]
        cases += [
            ("SELECT ID, IIf(Amount,'yes','no') AS Chosen FROM ScalarSamples ORDER BY ID", 'ID\tChosen\n1\t"yes"\n2\t"yes"\n3\t"no"\n'),
            ("SELECT ID, IIf([Money],1,0) AS Chosen FROM ScalarSamples ORDER BY ID", "ID\tChosen\n1\t1\n2\t0\n3\t0\n"),
            ("SELECT ID, IIf([Real],1,0) AS Chosen FROM ScalarSamples ORDER BY ID", "ID\tChosen\n1\t0\n2\t1\n3\t0\n"),
        ]
        for sql, wanted in cases:
            assert query(sql) == wanted, (profile, sql)
            controls += 1
        assert path.read_bytes() == before, "Read command changed original bytes"
        print(f"{profile}: {controls} inventory/page/projection/query controls agree; unchanged SHA-256 {hashlib.sha256(before).hexdigest()}")


if __name__ == "__main__":
    main()
