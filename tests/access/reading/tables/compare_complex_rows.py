"""Compare raw complex child rows, without claiming attachment file extraction."""

from cli_tsv import command_output
import argparse
import hashlib
from pathlib import Path
import subprocess


def run(*args):
    return command_output(args, subprocess.check_output(list(map(str,args)),text=True,encoding="utf-8"))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for key in ["cli","rows","jar","ace2016","ace2019"]:
        parser.add_argument("--"+key,required=True,type=Path)
    args=parser.parse_args()
    for path,code in [(args.ace2016,5),(args.ace2019,6)]:
        original=path.read_bytes()
        assert original[20:24]==bytes([code,0,0,0])
        declarations=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,"docs/testing/ComplexRowsFixture.java","--tables",path)
        combined=""
        for line in declarations.splitlines():
            column,table,count=line.split("\t")
            assert count=="3"
            expected=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,"docs/testing/TableDataOracle.java",path,table)
            actual=run(args.rows,path,table)
            assert actual==expected,(code,column,"raw child mismatch")
            # Count and scalar records are available through the normal read-only
            # adapter. Parent complex fields remain outside scalar query carriers.
            assert run(args.cli,"query","run",f"SELECT Count(*) AS N FROM [{table}]","--database",path)=="N\n3\n"
            assert len(actual.splitlines())==3
            # Canonicalize generated physical table names for stable row digests.
            combined+="\n".join(column+"\t"+row.split("\t",1)[1] for row in actual.splitlines())+"\n"
        parent=subprocess.run([str(args.cli),"query","run","SELECT * FROM Collections","--database",str(path)],capture_output=True)
        assert parent.returncode==2 and not parent.stdout
        assert path.read_bytes()==original
        print(code, "raw child rows",9,hashlib.sha256(original).hexdigest(),hashlib.sha256(combined.encode()).hexdigest())
    print("Raw child storage agrees; attachment extraction, parent collection semantics and Access qualification remain separate.")


if __name__=="__main__":
    main()
