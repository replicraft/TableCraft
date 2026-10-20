"""Compare CLI table browsing against independent Jackcess scalar snapshots.

Uses the reviewed Jackcess artifact and original ScalarFixture recipe. Generated
files remain local. This establishes independent reading, not native Access SQL
or shared-file qualification. Scalar float formatting covers this recipe's exact
binary fractions only; it is not a general cross-language float formatter.
"""

from cli_tsv import command_output
from float_model import expected_bits, display_bits
import argparse
import calendar
import datetime
import hashlib
from fractions import Fraction
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
        for interval in ["yyyy", "q", "m", "y", "d", "w", "ww", "h", "n", "s"]:
            values = []
            for row in tagged_rows:
                tag = row[columns.index("Stamp")]
                if tag == "N": values.append("NULL\n"); continue
                current = datetime.datetime.fromisoformat(tag[1:])
                january = datetime.date(current.year, 1, 1)
                yearday = (current.date() - january).days + 1
                offset = (january.weekday() + 1) % 7
                parts = {"yyyy": current.year, "q": (current.month-1)//3+1, "m": current.month, "y": yearday, "d": current.day, "w": (current.weekday()+1)%7+1, "ww": (yearday-1+offset)//7+1, "h": current.hour, "n": current.minute, "s": current.second}
                values.append(str(parts[interval]) + "\n")
            sql = "SELECT DatePart('" + interval + "',Stamp) AS PartValue FROM ScalarSamples ORDER BY ID"
            assert run(args.cli, "query", "run", sql, "--database", path) == "PartValue\n" + "".join(values), (profile, "DatePart", interval)
            controls += 1
        for first, rule in [(2,2),(1,3)]:
            values=[]
            for row in tagged_rows:
                tag=row[columns.index("Stamp")]
                if tag == "N": values.append("NULL\n"); continue
                current=datetime.datetime.fromisoformat(tag[1:]).date()
                # Independent weekday iteration locates the first qualifying
                # week start; these original dates do not hit the Monday anomaly.
                anchor=datetime.date(current.year,1,4 if rule == 2 else 1)
                target=(first+5)%7
                if rule == 2:
                    while anchor.weekday() != target: anchor-=datetime.timedelta(days=1)
                else:
                    while anchor.weekday() != target: anchor+=datetime.timedelta(days=1)
                if current < anchor:
                    anchor=datetime.date(current.year-1,1,1)
                    while anchor.weekday() != target: anchor+=datetime.timedelta(days=1)
                values.append(str((current-anchor).days//7+1)+"\n")
            sql="SELECT DatePart('ww',Stamp,"+str(first)+","+str(rule)+") AS PartValue FROM ScalarSamples ORDER BY ID"
            assert run(args.cli,"query","run",sql,"--database",path) == "PartValue\n"+"".join(values),(profile,"DatePart",first,rule)
            controls+=1
        # Exact independent rational conversion model; no reference expressions.
        numeric_columns=["ID","Flag","Octet","Small","Money","Real","WideReal","Amount"]
        targets=[("CByte",0,255),("CInt",-(2**15),2**15-1),("CLng",-(2**31),2**31-1),("CLngLng",-(2**63),2**63-1),("CBool",None,None)]
        def rational(tag):
            if tag.startswith("B"): return Fraction(-int(tag[1:]))
            if tag.startswith("I"): return Fraction(int(tag.split(":")[1]))
            if tag.startswith("U"): return Fraction(int(tag[1:]))
            if tag.startswith("C"): return Fraction(int(tag[1:]),10000)
            if tag.startswith("Q"):
                coefficient,scale=tag[1:].split(":")
                return Fraction(int(coefficient),10**int(scale))
            return Fraction(struct.unpack(">f" if tag[0]=="F" else ">d",bytes.fromhex(tag[1:]))[0])
        for column in numeric_columns:
            position=columns.index(column)
            tags=[row[position] for row in tagged_rows if row[position] != "N"]
            for function,minimum,maximum in targets:
                expected=[]; overflow=False
                for tag in tags:
                    number=rational(tag)
                    if function=="CBool": expected.append("TRUE" if number else "FALSE"); continue
                    integer=255 if function=="CByte" and tag=="B1" else round(number)
                    overflow |= not minimum <= integer <= maximum
                    expected.append(str(integer))
                sql="SELECT "+function+"("+quote(column)+") AS ConvertedValue FROM ScalarSamples WHERE "+quote(column)+" IS NOT NULL ORDER BY ID"
                result=subprocess.run([str(args.cli),"query","run",sql,"--database",str(path)],capture_output=True,text=True)
                if overflow:
                    assert result.returncode==2 and not result.stdout,(profile,column,function,"overflow")
                else:
                    assert result.returncode==0,result.stderr
                    assert command_output([str(args.cli),"query","run"],result.stdout)=="ConvertedValue\n"+"".join(value+"\n" for value in expected),(profile,column,function)
                controls+=1
                if any(row[position]=="N" for row in tagged_rows):
                    sql="SELECT "+function+"("+quote(column)+") FROM ScalarSamples WHERE ID=3"
                    result=subprocess.run([str(args.cli),"query","run",sql,"--database",str(path)],capture_output=True,text=True)
                    assert result.returncode==2 and not result.stdout and "NULL" in result.stderr,(profile,column,function,"NULL")
                    controls+=1
        negative=round(rational(tagged_rows[1][columns.index("Amount")]))
        sql="SELECT CLngLng(Amount) AS ConvertedValue FROM ScalarSamples WHERE ID=2"
        assert run(args.cli,"query","run",sql,"--database",path)=="ConvertedValue\n"+str(negative)+"\n",(profile,"exact negative Decimal")
        controls+=1
        # Exact getter/carrier model for CDec; normalized floating inputs have
        # separate goldens and are not inferred from a native SQL oracle here.
        for column in ["ID","Flag","Octet","Small","Money","Amount"]:
            position=columns.index(column)
            expected=[]
            for row in tagged_rows:
                tag=row[position]
                if tag=="N": continue
                if tag.startswith("Q"):
                    coefficient,scale=map(int,tag[1:].split(":"))
                elif tag.startswith("C"):
                    coefficient,scale=int(tag[1:]),4
                else:
                    coefficient,scale=int(rational(tag)),0
                digits=str(abs(coefficient)).zfill(scale+1)
                expected.append(("-" if coefficient<0 else "")+(digits[:-scale]+"."+digits[-scale:] if scale else digits))
            sql="SELECT CDec("+quote(column)+") AS ConvertedValue FROM ScalarSamples WHERE "+quote(column)+" IS NOT NULL ORDER BY ID"
            assert run(args.cli,"query","run",sql,"--database",path)=="ConvertedValue\n"+"".join(value+"\n" for value in expected),(profile,column,"CDec")
            controls+=1
            if any(row[position]=="N" for row in tagged_rows):
                result=subprocess.run([str(args.cli),"query","run","SELECT CDec("+quote(column)+") FROM ScalarSamples WHERE ID=3","--database",str(path)],capture_output=True,text=True)
                assert result.returncode==2 and not result.stdout and "NULL" in result.stderr,(profile,column,"CDec NULL")
                controls+=1
        assert run(args.cli,"query","run","SELECT CDec(2^95) AS ConvertedValue FROM ScalarSamples WHERE ID=1","--database",path)=="ConvertedValue\n39614081257132168796771975168\n",(profile,"wide expression Decimal")
        controls+=1
        for column in numeric_columns:
            position=columns.index(column)
            tags=[row[position] for row in tagged_rows if row[position] != "N"]
            for function,single in [("CSng",True),("CDbl",False)]:
                sql="SELECT "+function+"("+quote(column)+") AS ConvertedValue FROM ScalarSamples WHERE "+quote(column)+" IS NOT NULL ORDER BY ID"
                actual=run(args.cli,"query","run",sql,"--database",path).splitlines()
                assert actual[0]=="ConvertedValue" and len(actual)==len(tags)+1,(profile,column,function)
                for cell,tag in zip(actual[1:],tags):
                    negative_zero=tag.startswith(("F","D")) and struct.unpack(">f" if tag[0]=="F" else ">d",bytes.fromhex(tag[1:]))[0]==0 and bool(int(tag[1],16)&8)
                    assert display_bits(cell,single)==expected_bits(rational(tag),single,negative_zero),(profile,column,function,tag)
                controls+=1
                if any(row[position]=="N" for row in tagged_rows):
                    sql="SELECT "+function+"("+quote(column)+") FROM ScalarSamples WHERE ID=3"
                    result=subprocess.run([str(args.cli),"query","run",sql,"--database",str(path)],capture_output=True,text=True)
                    assert result.returncode==2 and not result.stdout and "NULL" in result.stderr,(profile,column,function,"NULL")
                    controls+=1
        for column in numeric_columns:
            position=columns.index(column)
            scaled=[round(rational(row[position])*10000) for row in tagged_rows if row[position]!="N"]
            sql="SELECT CCur("+quote(column)+") AS ConvertedValue FROM ScalarSamples WHERE "+quote(column)+" IS NOT NULL ORDER BY ID"
            result=subprocess.run([str(args.cli),"query","run",sql,"--database",str(path)],capture_output=True,text=True)
            if any(not -(2**63)<=value<2**63 for value in scaled):
                assert result.returncode==2 and not result.stdout,(profile,column,"Currency overflow")
            else:
                assert result.returncode==0,result.stderr
                assert command_output([str(args.cli),"query","run"],result.stdout)=="ConvertedValue\n"+"".join(fixed(str(value),4)+"\n" for value in scaled),(profile,column,"Currency")
            controls+=1
            if any(row[position]=="N" for row in tagged_rows):
                result=subprocess.run([str(args.cli),"query","run","SELECT CCur("+quote(column)+") FROM ScalarSamples WHERE ID=3","--database",str(path)],capture_output=True,text=True)
                assert result.returncode==2 and not result.stdout and "NULL" in result.stderr,(profile,column,"Currency NULL")
                controls+=1
        # Independent exact rational checks for newly supported Decimal arithmetic.
        amount_position=columns.index("Amount")
        for expression,operation in [("Amount+1",lambda value:value+1),("Amount-1",lambda value:value-1),("Amount*2",lambda value:value*2),("Amount/2",lambda value:value/2)]:
            actual=run(args.cli,"query","run","SELECT "+expression+" AS DecimalValue FROM ScalarSamples ORDER BY ID","--database",path).splitlines()
            assert actual[0]=="DecimalValue" and len(actual)==len(tagged_rows)+1
            for cell,row in zip(actual[1:],tagged_rows):
                tag=row[amount_position]
                if tag=="N": assert cell=="NULL",(profile,expression,"NULL")
                else: assert Fraction(cell)==operation(rational(tag)),(profile,expression,"exact result")
            controls+=1
        actual=run(args.cli,"query","run","SELECT Sum(Amount) AS Total,Avg(Amount) AS Mean FROM ScalarSamples","--database",path).splitlines()
        values=[rational(row[amount_position]) for row in tagged_rows if row[amount_position]!="N"]
        assert actual[0]=="Total\tMean" and len(actual)==2
        total,mean=map(Fraction,actual[1].split("\t"))
        assert total==sum(values) and mean==sum(values)/len(values),(profile,"Decimal aggregates")
        controls+=1
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
