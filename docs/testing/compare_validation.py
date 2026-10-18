"""Compare bounded validation coverage with the original Jackcess rule recipe."""
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
    fields={"Ranges":("Score",3,"Checked"),"Nullable":("Score",2,"Checked"),
        "Calls":("Label",1,"Not checked: unsupported rule"),"EmptyCalls":("Label",0,"Not checked: unsupported rule"),
        "UnknownNull":("Score",1,"Not checked: unsupported rule")}
    for label,path in [(args.ace_label,args.ace),(args.jet_label,args.jet)]:
        before=hashlib.sha256(path.read_bytes()).hexdigest()
        oracle=subprocess.check_output(["java","-Dstdout.encoding=UTF-8","-cp",str(args.jar),"docs/testing/ValidationFixture.java","--inspect",str(path)],text=True)
        expected_rows=dict(line.split("\t") for line in oracle.splitlines())
        for table,(column,count,coverage) in fields.items():
            output=subprocess.check_output([str(args.cli),"inspect",str(path),"--table",table,"--constraints"],text=True)
            lines=[line.split("\t") for line in output.splitlines()]
            record=next(row for row in lines if row[0]==column)
            if record[-1]!=coverage or f"Rows checked\t{count}\n" not in output or int(expected_rows[table])!=count:
                raise RuntimeError(f"{label}: field rule disagreement")
            print(label,table,count,coverage,flush=True)
        output=subprocess.check_output([str(args.cli),"inspect",str(path),"--table","Bounds","--constraints"],text=True)
        if "Table validation rule\tChecked\n" not in output or "Rows checked\t2\n" not in output or expected_rows["Bounds"]!="2":
            raise RuntimeError(f"{label}: table rule disagreement")
        if hashlib.sha256(path.read_bytes()).hexdigest()!=before:
            raise RuntimeError(f"{label}: input changed")
        print(label,"Bounds",2,"Checked",before,flush=True)


if __name__=="__main__":
    main()
