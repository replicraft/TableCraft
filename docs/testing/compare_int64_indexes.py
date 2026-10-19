"""Compare exact Int64 index order/coverage; native Microsoft Access remains unrun."""
import argparse
import hashlib
from pathlib import Path
import subprocess


def run(*args, success=True):
    result = subprocess.run(list(map(str, args)), capture_output=True, text=True, check=False)
    if success and result.returncode:
        raise RuntimeError(result.stderr)
    if not success and (result.returncode != 2 or result.stdout):
        raise RuntimeError("Failed query exposed partial output or unexpected status")
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["jar", "cli", "ranges", "ace2016", "ace2019"]:
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for path in [args.ace2016, args.ace2019]:
        original = path.read_bytes()
        normalized = ""
        for table, index, columns, count in [
            ("LargeIndexed", "BigAsc", ["ID", "Value"], 600),
            ("LargeIndexed", "BigDesc", ["ID", "Value"], 600),
            ("LargeUnique", "BigUnique", ["Value"], 12),
            ("LargeEmpty", "BigEmpty", ["Value"], 0),
        ]:
            expected = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                "docs/testing/IndexRowsOracle.java", path, table, index, *columns)
            assert len(expected.splitlines()) == count
            def query(where=""):
                sql = f"SELECT {', '.join('[' + col + ']' for col in columns)} FROM {table} {where}"
                return run(args.cli, "query", "run", sql, "--database", path, "--index", table, index)
            header = "\t".join(columns) + "\n"
            actual = query()
            assert actual == header + expected, (table, index, "exact indexed rows")
            normalized += actual
            null_rows = "".join(row + "\n" for row in expected.splitlines() if row.split("\t")[-1] == "NULL")
            assert query("WHERE [Value] IS NULL") == header + null_rows, (table, index, "NULL filter")
            large_rows = "".join(row + "\n" for row in expected.splitlines()
                if row.split("\t")[-1] != "NULL" and int(row.split("\t")[-1]) > 9007199254740992)
            assert query("WHERE [Value] > 9007199254740992") == header + large_rows
            if table == "LargeIndexed":
                def seek(*selection):
                    return run(args.ranges,path,"--table",table,"--index",index,"--width","int64",*selection)
                assert seek("all") == expected
                assert seek("null") == null_rows
                cases = [
                    (None,None,False,False),
                    (9007199254740993,9007199254740993,False,False),
                    (9007199254740992,9007199254740993,True,False),
                    (-9007199254740993,-1,False,True),
                    (-(1<<63),-(1<<63),False,False),
                    ((1<<63)-1,(1<<63)-1,False,False),
                    ((1<<63)-1,None,True,False),
                    (None,-(1<<63),False,True),
                    (1,0,False,False),
                    (0,0,False,True),
                ]
                for lower,upper,exclude_lower,exclude_upper in cases:
                    selection=["range"]
                    if lower is not None:selection.extend(["--lower",str(lower)])
                    if upper is not None:selection.extend(["--upper",str(upper)])
                    if exclude_lower:selection.append("--exclude-lower")
                    if exclude_upper:selection.append("--exclude-upper")
                    rows=[]
                    for row in expected.splitlines():
                        text=row.split("\t")[-1]
                        if text=="NULL":continue
                        value=int(text)
                        if lower is not None and (value<lower or exclude_lower and value==lower):continue
                        if upper is not None and (value>upper or exclude_upper and value==upper):continue
                        rows.append(row+"\n")
                    assert seek(*selection)=="".join(rows),(index,selection,"exact native seek")
                    clauses=[]
                    if lower is not None:clauses.append(f"[Value] {'>' if exclude_lower else '>='} {lower}")
                    if upper is not None:clauses.append(f"[Value] {'<' if exclude_upper else '<='} {upper}")
                    where="WHERE "+(" AND ".join(clauses) if clauses else "[Value] IS NOT NULL")
                    assert query(where)==header+"".join(rows),(index,where,"exact query seek")
            coverage = run(args.cli, "inspect", path, "--table", table, "--check")
            assert "unsupported" not in coverage.lower(), coverage
            assert "checked" in coverage.lower(), coverage
        # Both selected indexes are actual multi-page trees, not just leaf probes.
        metadata = run(args.cli, "inspect", path, "--table", "LargeIndexed", "--indexes")
        roots = [int(row.split("\t")[3]) for row in metadata.splitlines()
            if row.split("\t")[0] in ["BigAsc", "BigDesc"]]
        assert len(roots) == 2 and all(original[root * 4096] == 3 for root in roots), metadata
        assert path.read_bytes() == original
        print(path.name, "file", hashlib.sha256(original).hexdigest(), "ordered rows", 1212,
            "sha256", hashlib.sha256(normalized.encode()).hexdigest())
    print("Int64 keys/order, 24 exact native selections and 20 matching query intervals per format, NULL/precision filters, full checks and immutable inputs verified; native Access unrun.")


if __name__ == "__main__":
    main()
