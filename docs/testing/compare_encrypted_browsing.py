"""Compare password-enabled CLI table/row/query reads with independent encrypted cursors."""
import argparse
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
from compare_agile_pages import PASSWORDS, RECIPES, run
from compare_browsing import display
from compare_encryption_envelopes import CASES


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ["jar","crypt-jar","bc-jar","cli","directory"]:
        parser.add_argument("--"+name,type=Path,required=True)
    args=parser.parse_args()
    classpath=os.pathsep.join(str(p) for p in [args.jar,args.crypt_jar,args.bc_jar])
    for name,password in PASSWORDS.items():
        path=args.directory/name
        before=hashlib.sha256(path.read_bytes()).hexdigest();assert before==CASES[name][0]
        table="Table1" if name.startswith("db2007") else "Customers"
        oracle=run(["java","-cp",classpath,RECIPES/"EncryptedPagesOracle.java",path,"--rows",table],password)
        assert oracle.returncode==0 and not oracle.stderr,oracle.stderr
        records=[line.split("\t") for line in oracle.stdout.decode().splitlines()]
        headings=records[0][1:];expected=[[display(v) for v in row[1:]] for row in records[1:]]
        def cli(*command,password_value=password,verbosity=False):
            invocation=[str(args.cli),"--format","tsv",*map(str,command)]
            if password_value is not None:invocation.extend(["-p",password_value])
            if verbosity:invocation.append("-vvv")
            return subprocess.run(invocation,capture_output=True,check=False)
        def cells(*command):
            result=cli(*command)
            assert result.returncode==0 and not result.stderr,(name,result.stderr)
            return list(csv.reader(io.StringIO(result.stdout.decode()),delimiter="\t",strict=True))
        assert cells("rows",path,table)==[headings,*expected]
        assert cells("query","run",f"SELECT * FROM [{table}] ORDER BY ID","--database",path)==[headings,*sorted(expected,key=lambda row:int(row[0]))]
        assert cells("query","run",f"SELECT Count(*) AS Total FROM [{table}]","--database",path)==[["Total"],[str(len(expected))]]
        tables=cells("tables",path,"--count")
        assert [table,"User",str(len(expected))] in tables
        checked=dict(cells("inspect",path,"--table",table)[:4])
        assert checked["Table"]==table and checked["Declared rows"]==str(len(expected))
        all_rows=cli("rows",path,"--all");assert all_rows.returncode==0 and not all_rows.stderr
        for password_value,message in [(None,b"database password is required"),("public-wrong-secret",b"incorrect database password")]:
            failed=cli("rows",path,table,password_value=password_value,verbosity=True)
            assert failed.returncode==2 and not failed.stdout and message in failed.stderr
            assert b"public-wrong-secret" not in failed.stderr and str(path).encode() not in failed.stderr
        assert hashlib.sha256(path.read_bytes()).hexdigest()==before
        print(f"{name}: {len(expected)} exact rows, inventory/schema/count/query/all-table and credential controls agree; unchanged {before}")


if __name__=="__main__":main()
