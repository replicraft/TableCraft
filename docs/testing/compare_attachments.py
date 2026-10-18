"""Compare bounded original attachment bytes with the independent public decoder."""
import argparse
import hashlib
from pathlib import Path
import subprocess

from compare_composite_constraints import run


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ["jar","example","ace2016","ace2019"]:
        parser.add_argument("--"+name,required=True,type=Path)
    args=parser.parse_args()
    for path in [args.ace2016,args.ace2019]:
        original=path.read_bytes()
        expected=run("java","-Dstdout.encoding=UTF-8","-cp",args.jar,"docs/testing/AttachmentFixture.java","--inspect",path)
        actual=run(args.example,path)
        assert actual==expected,"decoded attachment bytes disagree"
        rows=[row.split("\t") for row in actual.splitlines()]
        assert len(rows)==6 and [len(bytes.fromhex(row[3])) for row in rows]==[0,256,6,8,4,65536]
        assert [row[0] for row in rows]==["0"]*6
        modes=run("java","-cp",args.jar,"docs/testing/AttachmentFixture.java","--encodings",path)
        assert [int(row.split("\t")[2]) for row in modes.splitlines()]==[1,1,1,0,0,1]
        assert path.read_bytes()==original
        print(path.name,"files",6,"original bytes",sum(len(bytes.fromhex(row[3])) for row in rows),
            "file",hashlib.sha256(original).hexdigest(),"decoded",hashlib.sha256(actual.encode()).hexdigest())
    print("Exact original attachment bytes, type labels, empty/maximal files and unchanged inputs verified; native Access unrun.")


if __name__=="__main__":main()
