"""Compare original modern ACE scalar fixtures; native Access remains unrun."""

from cli_tsv import command_output
import argparse
import datetime
import hashlib
from pathlib import Path
import subprocess


def run(*args, success=True):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True, check=False)
    if success and result.returncode:
        raise RuntimeError(result.stderr)
    if not success and (result.returncode != 2 or result.stdout):
        raise RuntimeError("Unsupported query published output or did not fail")
    return command_output(args, result.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "rows", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path, code in [(args.ace2016, 5), (args.ace2019, 6)]:
        original = path.read_bytes()
        assert original[4:20] == b"Standard ACE DB\0"
        assert original[20:24] == bytes([code, 0, 0, 0])
        def query(sql, *extra, success=True):
            return run(args.cli, "query", "run", sql, "--database", path, *extra, success=success)
        tables = ["LargeNumbers"] + (["ExtendedDates"] if code == 6 else [])
        tagged = ""
        for table in tables:
            actual = run(args.rows, path, table)
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/TableDataOracle.java", path, table)
            assert actual == expected, (code, table, "carrier mismatch")
            tagged += actual
            if table == "LargeNumbers":
                numbers = [None, -(2**63), -9007199254740993, -1, 0, 9007199254740993, 2**63-1]
                assert query("SELECT [Value] FROM LargeNumbers ORDER BY [Value]") == "Value\n" + "".join("NULL\n" if n is None else f"{n}\n" for n in numbers)
                assert query("PARAMETERS Floor BIGINT; SELECT [Value] FROM LargeNumbers WHERE [Value] > Floor ORDER BY [Value]", "--param", "Floor=bigint:9007199254740992") == "Value\n9007199254740993\n9223372036854775807\n"
            else:
                values = [line.split("\t")[2] for line in expected.splitlines()]
                dates = sorted(value[1:] for value in values if value.startswith("Z"))
                assert query("SELECT Stamp FROM ExtendedDates ORDER BY Stamp") == "Stamp\nNULL\n" + "".join(f"#{date}#\n" for date in dates)
                assert query("SELECT Count(*) AS N FROM ExtendedDates WHERE Stamp=Stamp") == "N\n5\n"
                assert query("SELECT DateAdd('s',0,Stamp) AS Stamp FROM ExtendedDates ORDER BY Stamp") == "Stamp\nNULL\n" + "".join(f"#{date}#\n" for date in dates)
                leap = next(date for date in dates if date.startswith("2000-02-29 "))
                assert query("SELECT DateAdd('yyyy',1,Stamp) AS Stamp FROM ExtendedDates WHERE Year(Stamp)=2000") == "Stamp\n#2001-02-28" + leap[10:] + "#\n"
                for function, attribute in [("Year", "year"), ("Month", "month"), ("Day", "day"), ("Hour", "hour"), ("Minute", "minute"), ("Second", "second")]:
                    wanted = "Part\nNULL\n" + "".join(str(getattr(datetime.datetime.fromisoformat(date), attribute)) + "\n" for date in dates)
                    assert query("SELECT " + function + "(Stamp) AS Part FROM ExtendedDates ORDER BY Stamp") == wanted, (code, function)
        query("SELECT * FROM Calculated", success=False)
        indexed = query("SELECT [Value] FROM LargeNumbers", "--index", "LargeNumbers", "BigKey")
        assert indexed == "Value\n" + "".join("NULL\n" if n is None else f"{n}\n" for n in numbers)
        coverage = run(args.cli, "inspect", path, "--table", "LargeNumbers", "--check")
        assert "unsupported" not in coverage.lower() and "checked" in coverage.lower(), coverage
        assert path.read_bytes() == original
        print(code, "file", hashlib.sha256(original).hexdigest(), "rows", len(tagged.splitlines()), hashlib.sha256(tagged.encode()).hexdigest())
    print("Exact carriers, query ordering/filtering, rejected unsupported features and unchanged files verified; native Access unrun.")


if __name__ == "__main__":
    main()
