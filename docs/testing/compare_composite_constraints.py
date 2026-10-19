"""Compare original composite audits and reject corrupted disposable row copies."""

from cli_tsv import command_output
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile


def run(*args):
    return command_output(args, subprocess.check_output(list(map(str,args)),text=True,encoding="utf-8"))


def roots(data, owner):
    result=[]
    for page in range(len(data)//4096):
        start=page*4096
        if data[start:start+2]!=b"\x01\x01" or int.from_bytes(data[start+4:start+8],"little")!=owner: continue
        count=int.from_bytes(data[start+12:start+14],"little")
        for slot in range(count):
            raw=int.from_bytes(data[start+14+slot*2:start+16+slot*2],"little")
            if raw&0xe000:continue
            end=4096 if slot==0 else int.from_bytes(data[start+12+slot*2:start+14+slot*2],"little")&0x1fff
            result.append((start+(raw&0x1fff),start+end))
    return result


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for key in ["cli","rows","jar","ace2016","ace2019"]:parser.add_argument("--"+key,required=True,type=Path)
    args=parser.parse_args()
    for path,code in [(args.ace2016,5),(args.ace2019,6)]:
        original=path.read_bytes();assert original[20:24]==bytes([code,0,0,0])
        catalog=run(args.cli,"inspect",path,"--catalog", "--show-system").splitlines()
        header=next(i for i,line in enumerate(catalog) if line.startswith("Object ID\t"))
        fields=catalog[header].split("\t")
        ids={cells[fields.index("Name")]:int(cells[fields.index("Native ID")]) for cells in (r.split("\t") for r in catalog[header+1:])}
        tagged=""
        for table,count,relationship in [("Parents",6,"Pairs"),("Children",6,"Pairs"),("UniqueChildren",3,"UniquePairs")]:
            expected=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,"docs/testing/TableDataOracle.java",path,table)
            actual=run(args.rows,path,table);assert actual==expected;tagged+=actual
            output=run(args.cli,"inspect",path,"--table",table,"--constraints")
            assert f"Rows checked\t{count}\n" in output
            assert f"{relationship}\tChecked: exact key membership\n" in output
        child=roots(original,ids["Children"]);parent=roots(original,ids["Parents"]);unique=roots(original,ids["UniqueChildren"])
        cases=[]
        changed=bytearray(original);changed[child[0][0]+2:child[0][0]+6]=(999).to_bytes(4,"little",signed=True);cases.append(("Children",changed,"foreign key has no referenced record"))
        changed=bytearray(original);changed[child[0][1]-1]&=~1;cases.append(("Children",changed,"foreign key has no referenced record"))
        changed=bytearray(original);changed[parent[1][0]+2:parent[1][1]]=original[parent[0][0]+2:parent[0][1]];cases.append(("Parents",changed,"referenced key is not unique"))
        changed=bytearray(original);changed[unique[1][0]+2:unique[1][1]]=original[unique[0][0]+2:unique[0][1]];cases.append(("UniqueChildren",changed,"one-to-one relationship has duplicate dependent keys"))
        with tempfile.TemporaryDirectory(prefix="tablecraft-composite-") as directory:
            copy=Path(directory)/"copy.accdb"
            for table,data,error in cases:
                copy.write_bytes(data)
                result=subprocess.run([str(args.cli),"inspect",str(copy),"--table",table,"--constraints"],capture_output=True,text=True)
                assert result.returncode==2 and not result.stdout and error in result.stderr,(table,result.stderr)
        assert path.read_bytes()==original
        print(code,"rows",15,"corrupt copies",len(cases),hashlib.sha256(original).hexdigest(),hashlib.sha256(tagged.encode()).hexdigest())
    print("Independent row/audit checks and corrupt-copy rejection pass; native Access and physical composite index validation remain unqualified.")


if __name__=="__main__":main()
