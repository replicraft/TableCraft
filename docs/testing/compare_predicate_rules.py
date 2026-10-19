"""Check original numeric predicate rules, retaining unsupported text/NULL cases."""

from cli_tsv import command_output
import argparse
import hashlib
from pathlib import Path
import subprocess


def run(*args):return command_output(args, subprocess.check_output(list(map(str,args)),text=True,encoding="utf-8"))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for k in ["cli","rows","jar","ace2016","ace2019"]:p.add_argument("--"+k,required=True,type=Path)
    a=p.parse_args()
    for path,code in [(a.ace2016,5),(a.ace2019,6)]:
        original=path.read_bytes();assert original[20:24]==bytes([code,0,0,0])
        counts=dict(line.split("\t") for line in run("java","-Dstdout.encoding=UTF-8","-cp",a.jar,"docs/testing/PredicateRuleFixture.java","--inspect",path).splitlines())
        tagged=""
        for table,column,count,status in [("Ranges","Score",3,"Checked"),("Reversed","Score",3,"Checked"),("Allowed","Score",3,"Checked"),("Excluded","Score",3,"Checked"),("EmptyText","Label",0,"Not checked: unsupported rule"),("NullMembership","Score",1,"Not checked: unsupported rule")]:
            output=run(a.cli,"inspect",path,"--table",table,"--constraints")
            record=next(row.split("\t") for row in output.splitlines() if row.startswith(column+"\t"))
            assert record[-1]==status and f"Rows checked\t{count}\n" in output and counts[table]==str(count)
            expected=run("java","-Dstdout.encoding=UTF-8","-cp",a.jar,"docs/testing/TableDataOracle.java",path,table)
            actual=run(a.rows,path,table);assert expected==actual;tagged+=actual
        assert path.read_bytes()==original
        print(code,"rules",6,"rows",13,hashlib.sha256(original).hexdigest(),hashlib.sha256(tagged.encode()).hexdigest())
    print("Predicate audits/independent rows match; text collation, unknown NULL results and native Access remain unqualified.")


if __name__=="__main__":main()
