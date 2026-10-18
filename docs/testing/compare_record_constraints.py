"""Check record-audit coverage over the original relationship fixtures."""
import argparse
import hashlib
from pathlib import Path
import subprocess


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for argument in ["cli","ace","jet"]:
        parser.add_argument("--"+argument,required=True,type=Path)
    parser.add_argument("--ace-label", default="ace")
    parser.add_argument("--jet-label", default="jet4")
    args=parser.parse_args()
    expected={"Children":(3,"ParentChildren","Checked: Int32 key membership"),
        "UniqueChildren":(1,"OneToOne","Checked: Int32 key membership"),
        "NullableChildren":(1,"SetNull","Checked: Int32 key membership"),
        "CompositeChildren":(2,"Composite","Not checked: unsupported profile"),
        "LooseChildren":(1,"Unenforced","Not enforced")}
    for label,path in [(args.ace_label,args.ace),(args.jet_label,args.jet)]:
        before=hashlib.sha256(path.read_bytes()).hexdigest()
        for table,(rows,relationship,status) in expected.items():
            output=subprocess.check_output([str(args.cli),"inspect",str(path),"--table",table,"--constraints"],text=True)
            if f"Rows checked\t{rows}\n" not in output or f"{relationship}\t{status}\n" not in output:
                raise RuntimeError(f"{label}: audit disagreement")
            print(label,table,rows,status,flush=True)
        if hashlib.sha256(path.read_bytes()).hexdigest()!=before:
            raise RuntimeError(f"{label}: input changed")


if __name__=="__main__":
    main()
