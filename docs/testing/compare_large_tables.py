"""Compare bounded native pages above 4,096 owned pages with inert Jackcess getters.

Original engineering fixtures and the reviewed external JAR remain local. This
checks stored rows, not native Access, SQL execution, snapshots or live-file safety.
"""
import argparse
import hashlib
from pathlib import Path
import subprocess
from compare_browsing import PROFILES, RECIPES, display, run


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar",type=Path,required=True)
    parser.add_argument("--cli",type=Path,required=True)
    parser.add_argument("--directory",type=Path,required=True)
    parser.add_argument("--generate",action="store_true")
    args=parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile,suffix in PROFILES:
            run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,RECIPES/"LargeTableFixture.java",args.directory/f"LargeRows-{profile}.{suffix}",profile)
    for profile,suffix in PROFILES:
        path=args.directory/f"LargeRows-{profile}.{suffix}"
        before=digest(path)
        for offset,limit in [(0,2),(1,2),(500,2),(1000,2)]:
            oracle=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,RECIPES/"LargeTableOracle.java",path,offset,limit).splitlines()
            kind,rows,owned=oracle[0].split("\t")
            assert kind=="Meta" and int(rows)==6000 and int(owned)>4096,(profile,"fixture size")
            expected=oracle[1]+"\n"+"".join("\t".join(display(tag) for tag in row.split("\t"))+"\n" for row in oracle[2:])
            assert run(args.cli,"rows",path,"LargeRows","--offset",offset,"--limit",limit)==expected,(profile,"page",offset)
        assert run(args.cli,"tables",path,"--count")=="Name\tKind\tRow Count\nLargeRows\tUser\t6000\n",(profile,"count")
        for command in [["rows",path,"LargeRows","--offset","5000","--limit","1"],["query","run","SELECT TOP 1 ID FROM LargeRows","--database",path]]:
            result=subprocess.run([str(args.cli),*map(str,command)],capture_output=True,text=True)
            assert result.returncode==2 and not result.stdout and "limit" in result.stderr,(profile,"bounded refusal")
        assert digest(path)==before,(profile,"input changed")
        print(f"{profile}: 6000 rows, {owned} owned pages; 7 page/count/refusal controls agree; {path.stat().st_size} bytes; unchanged SHA-256 {before}")


if __name__=="__main__":
    main()
