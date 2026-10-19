"""Compare native relationship declarations against a read-only Jackcess oracle."""

from cli_tsv import display_tsv
import argparse
import hashlib
from pathlib import Path
import subprocess


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for argument in ["cli","jar","ace","jet"]:
        parser.add_argument("--"+argument,required=True,type=Path)
    parser.add_argument("--ace-label", default="ace")
    parser.add_argument("--jet-label", default="jet4")
    args=parser.parse_args()
    for label,path in [(args.ace_label,args.ace),(args.jet_label,args.jet)]:
        before=hashlib.sha256(path.read_bytes()).hexdigest()
        expected=subprocess.check_output([
            "java","-Dstdout.encoding=UTF-8","-cp",str(args.jar),
            "docs/testing/RelationshipFixture.java","--inspect",str(path),
        ],text=True,encoding="utf-8")
        actual=display_tsv(subprocess.check_output([str(args.cli),"inspect",str(path),"--relationships"],text=True,encoding="utf-8"))
        records=sorted(actual.splitlines()[1:])
        if records != sorted(expected.splitlines()) or len(records) < 6:
            raise RuntimeError(f"{label}: relationship disagreement")
        if hashlib.sha256(path.read_bytes()).hexdigest() != before:
            raise RuntimeError(f"{label}: input changed")
        canonical="\n".join(records).encode("utf-8")
        print(label,len(records),hashlib.sha256(canonical).hexdigest(),before,flush=True)


if __name__=="__main__":
    main()
