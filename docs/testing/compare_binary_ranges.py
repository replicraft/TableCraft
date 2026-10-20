"""Compare typed Binary intervals with independent raw-byte filtering of native cursors."""
import argparse
from pathlib import Path
import struct
import subprocess
import tempfile
from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ["jar","cli","reader","directory"]:
        parser.add_argument("--"+name,type=Path,required=True)
    args=parser.parse_args()
    cases=[("all",None,None,False,False),("null",None,None,False,False),
           ("range",None,None,False,False),("range","","",False,False),
           ("range",None,"00",False,False),("range",None,"00",False,True),
           ("range","","00",True,False),("range","ff",None,False,False),
           ("range","00","80",False,False),("range","ff","00",False,False),
           ("range","00","00",True,False),("range","00"*8,None,False,False)]
    for profile,suffix in PROFILES:
        path=args.directory/f"BinaryIndexes-{profile}.{suffix}"
        before=digest(path)
        null_expected=None
        for index in ["BinaryAsc","BinaryDesc"]:
            original=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,RECIPES/"IndexRowsOracle.java",path,"BinaryIndexed",index,"ID","Bytes").splitlines()
            rows=[(line.split("\t")[0],None if line.split("\t")[1]=="NULL" else bytes.fromhex(line.split("\t")[1][2:])) for line in original]
            for mode,lower,upper,exclude_lower,exclude_upper in cases:
                low=None if lower is None else bytes.fromhex(lower)
                high=None if upper is None else bytes.fromhex(upper)
                def matches(value):
                    if mode=="all":return True
                    if mode=="null":return value is None
                    return value is not None and (low is None or (value>low if exclude_lower else value>=low)) and (high is None or (value<high if exclude_upper else value<=high))
                selected=[(id,value) for id,value in rows if matches(value)]
                expected="".join("I32:"+id+"\t"+("N" if value is None else "X"+value.hex())+"\n" for id,value in selected)
                projected="".join("I32:"+id+"\n" for id,_ in selected)
                command=[args.reader,path,"--index",index,"--mode",mode]
                if lower is not None:command.extend(["--lower",lower])
                if upper is not None:command.extend(["--upper",upper])
                if exclude_lower:command.append("--exclude-lower")
                if exclude_upper:command.append("--exclude-upper")
                assert run(*command)==expected,(profile,index,mode,lower,upper,"rows")
                assert run(*command,"--project-id")==projected,(profile,index,mode,lower,upper,"projection")
                if mode=="null" and index=="BinaryAsc":null_expected=expected
        metadata=run(args.cli,"inspect",path,"--table","BinaryIndexed","--indexes")
        root=next(int(line.split("\t")[3]) for line in metadata.splitlines() if line.startswith("BinaryAsc\t"))
        image=bytearray(path.read_bytes())
        tail=struct.unpack_from("<I",image,root*4096+20)[0]
        assert image[root*4096]==3 and tail!=0,"node/tail proof"
        image[tail*4096]=0
        with tempfile.TemporaryDirectory(prefix="tablecraft-binary-prune-") as directory:
            changed=Path(directory)/path.name;changed.write_bytes(image)
            assert run(args.reader,changed,"--mode","null")==null_expected,(profile,"disjoint subtree not read")
            failed=subprocess.run([str(args.reader),str(changed),"--mode","all"],capture_output=True)
            assert failed.returncode!=0 and not failed.stdout and b"Malformed" in failed.stderr,(profile,"full traversal detects damaged subtree")
        assert digest(path)==before
        print(f"{profile}: 48 interval/projection controls and 2 disjoint-subtree controls; unchanged {before}")


if __name__=="__main__":main()
