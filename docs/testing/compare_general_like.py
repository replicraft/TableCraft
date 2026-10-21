"""Compare the UTF-16 General LIKE model with independent complete-key matching.

The reviewed reference source is compiled only in a temporary directory with
index-only string trimming/prefix preparation disabled. This models literal
weights and finite recursive pattern paths, not native Access SQL execution.
"""
import argparse
import hashlib
import os
from pathlib import Path
import tempfile
from zipfile import ZipFile

from compare_browsing import RECIPES, run
from compare_general_search import literal
from compare_large_tables import digest

JAR_SHA256="19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
SOURCE_SHA256="4de1bc2f1663b0946ce39057dd90eace355f4f74b8f818c0fe167f0c65815428"
SOURCE_PATH="com/healthmarketscience/jackcess/impl/GeneralLegacyIndexCodes.java"


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ["jar","sources","cli"]:parser.add_argument("--"+name,type=Path,required=True)
    args=parser.parse_args()
    if digest(args.jar)!=JAR_SHA256 or digest(args.sources)!=SOURCE_SHA256:
        raise ValueError("unexpected reviewed reference artifacts")
    with ZipFile(args.sources) as archive:source=archive.read(SOURCE_PATH).decode("utf-8")
    start=source.index("protected static String toIndexCharSequence(Object value)")
    body=source.index("{",start)
    end=source.index("\n  /**",body)
    # Preserve the original source/license and encoder. Only the index-only
    # string adapter is replaced, explicitly retaining literal trailing spaces.
    modified=source[:body+1]+"\n    return ColumnImpl.toCharSequence(value).toString();\n  }\n"+source[end:]
    print("Reference raw-literal adapter SHA-256:",hashlib.sha256(modified.encode()).hexdigest(),flush=True)
    controls=0
    with tempfile.TemporaryDirectory(prefix="tablecraft-general-like-oracle-") as directory:
        classes=Path(directory)
        path=classes/SOURCE_PATH
        path.parent.mkdir(parents=True)
        path.write_bytes(modified.encode("utf-8"))
        run("javac","-encoding","UTF-8","-cp",args.jar,"-d",classes,path,RECIPES/"GeneralLikeOracle.java")
        classpath=os.pathsep.join([str(classes),str(args.jar)])
        for version,policy in [(0,"general-legacy"),(1,"general-2010")]:
            expected=run("java","-Dstdout.encoding=UTF-8","-cp",classpath,"com.healthmarketscience.jackcess.impl.GeneralLikeOracle",version)
            for row in expected.splitlines():
                mode,text_id,pattern_id,text,pattern,truth=row.split("\t")
                sql=f"SELECT ({literal(text)}) LIKE ({literal(pattern)}) AS R"
                actual=run(args.cli,"query","run",sql,"--collation",policy,"--like-mode",mode)
                if actual!="R\n"+truth+"\n":
                    raise ValueError(f"{policy} {mode} text {text_id} pattern {pattern_id}: independent model differs")
                controls+=1
            print(f"{policy}: completed {controls} cumulative independent pattern controls",flush=True)
    if digest(args.jar)!=JAR_SHA256 or digest(args.sources)!=SOURCE_SHA256:
        raise ValueError("reviewed reference artifact changed")
    print(f"Passed {controls} independent General LIKE controls; native Windows comparisons deferred.")


if __name__=="__main__":main()
