"""Compare bounded native Binary index cursors with independent read-only getters."""
import argparse
from pathlib import Path
import subprocess
from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ["jar","cli","rows","directory"]:
        parser.add_argument("--"+name,type=Path,required=True)
    parser.add_argument("--generate",action="store_true")
    args=parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile,suffix in PROFILES:
            run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,RECIPES/"BinaryIndexFixture.java",args.directory/f"BinaryIndexes-{profile}.{suffix}",profile)
    for profile,suffix in PROFILES:
        path=args.directory/f"BinaryIndexes-{profile}.{suffix}"
        before=digest(path)
        metadata=run(args.cli,"inspect",path,"--table","BinaryIndexed","--indexes")
        image=path.read_bytes()
        for name in ["BinaryAsc","BinaryDesc"]:
            root=next(int(line.split("\t")[3]) for line in metadata.splitlines() if line.startswith(name+"\t"))
            assert image[root*4096]==3,(profile,name,"multi-page node root")
        for table,index,count in [("BinaryIndexed","BinaryAsc",1000),("BinaryIndexed","BinaryDesc",1000),("BinaryUnique","UniqueBinary",32),("BinaryEmpty","EmptyBinary",0)]:
            expected=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,RECIPES/"IndexRowsOracle.java",path,table,index,"ID","Bytes")
            assert len(expected.splitlines())==count,(profile,table,"count")
            if table=="BinaryIndexed":
                assert "\t0x\n" in expected and "\tNULL\n" in expected,(profile,"empty/NULL distinction")
            tagged="".join(table+"\tI32:"+line.split("\t")[0]+"\t"+("N" if line.split("\t")[1]=="NULL" else "X"+line.split("\t")[1][2:])+"\n" for line in expected.splitlines())
            assert run(args.rows,path,table,"--index",index)==tagged,(profile,index,"tagged indexed rows")
            sql=f"SELECT [ID],[Bytes] FROM [{table}]"
            assert run(args.cli,"query","run",sql,"--database",path,"--index",table,index)=="ID\tBytes\n"+expected,(profile,index,"cursor")
            projected="ID\n"+"".join(line.split("\t")[0]+"\n" for line in expected.splitlines())
            assert run(args.cli,"query","run",f"SELECT [ID] FROM [{table}]","--database",path,"--index",table,index)==projected,(profile,index,"projection")
            report=run(args.cli,"inspect",path,"--table",table,"--check")
            assert "unsupported" not in report.lower() and "checked" in report.lower(),(profile,index,"check")
        for table,index in [("BinaryIgnored","IgnoredBinary"),("BinaryCompound","CompoundBinary")]:
            refused=subprocess.run([str(args.cli),"query","run",f"SELECT [ID] FROM [{table}]","--database",str(path),"--index",table,index],capture_output=True)
            assert refused.returncode==2 and not refused.stdout,(profile,index,"explicit refusal")
        assert digest(path)==before
        print(f"{profile}: 16 tagged/full/projected/check controls and 2 explicit refusals; unchanged {before}; {path.stat().st_size} bytes")


if __name__=="__main__":
    main()
