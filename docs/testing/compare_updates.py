"""Compare fresh-copy fixed Int32 edits with Jackcess; never modify the input."""

from cli_tsv import display_tsv
import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ["example", "cli", "jar", "jet"]:
        parser.add_argument("--" + argument, required=True, type=Path)
    args = parser.parse_args()
    before = args.jet.read_bytes()

    def inspect(path):
        return subprocess.check_output(["java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            "docs/testing/UpdateFixture.java", "--inspect", str(path)], text=True)

    original = "1\t10\t8400\t12288\n2\t20\t8400\t12288\n3\t30\t8400\t12288\n"
    if inspect(args.jet) != original:
        raise RuntimeError("Unexpected original recipe contents")
    with tempfile.TemporaryDirectory(prefix="tablecraft-native-updates-") as directory:
        output = Path(directory) / "updated.mdb"
        subprocess.run([str(args.example), str(args.jet), str(output), "Items", "Score", "1", "20", "27"], check=True)
        if inspect(output) != original.replace("2\t20\t", "2\t27\t"):
            raise RuntimeError("Independent reopen disagrees")
        after = output.read_bytes()
        if len(after) != len(before) or [(a, b) for a, b in zip(before, after) if a != b] != [(20, 27)]:
            raise RuntimeError("Unexpected byte changes outside the one-byte replacement")
        subprocess.run([str(args.cli), "inspect", str(output), "--table", "Items", "--check"], check=True, stdout=subprocess.DEVNULL)
        cli_output = Path(directory) / "cli-updated.mdb"
        report = subprocess.check_output([str(args.cli), "query", "update", "UPDATE Items SET Score=Score+7 WHERE Id=2", "--database", str(args.jet), "--output", str(cli_output)], text=True)
        if report != "Matched rows\t1\nChanged rows\t1\n" or cli_output.read_bytes() != after or inspect(cli_output) != original.replace("2\t20\t", "2\t27\t"):
            raise RuntimeError("CLI UPDATE differs from the native plan/independent reopen")
        indexed=Path(directory)/"indexed.mdb"
        subprocess.run([str(args.example),str(args.jet),str(indexed),"Items","Id","1","2","27"],check=True)
        if inspect(indexed)!=original.replace("2\t20\t","27\t20\t"):
            raise RuntimeError("Indexed update changed independent record values")
        ordered=subprocess.check_output(["java","-cp",str(args.jar),"docs/testing/IndexRowsOracle.java",str(indexed),"Items","PK_Items","Id"],text=True)
        if ordered!="1\n3\n27\n":raise RuntimeError("Independent index references disagree after key reordering")
        metadata=display_tsv(subprocess.check_output([str(args.cli),"inspect",str(args.jet),"--table","Items","--indexes"],text=True)).splitlines()
        root=int(next(line.split("\t")[3] for line in metadata if line.startswith("PK_Items\t")))
        page=before[root*4096:(root+1)*4096]
        if page[:2]!=bytes([4,1]) or page[24:26]!=bytes(2):raise RuntimeError("Expected original uncompressed root leaf")
        entry_end=4096-int.from_bytes(page[2:4],"little")
        reference=int.from_bytes(page[480+9+5:480+9+9],"big")
        data_page=reference>>8;slot=reference&255
        field=data_page*4096+(int.from_bytes(before[data_page*4096+14+slot*2:data_page*4096+16+slot*2],"little")&0x1fff)+2
        indexed_bytes=indexed.read_bytes()
        allowed=[range(field,field+4),range(root*4096+480,root*4096+entry_end)]
        if len(indexed_bytes)!=len(before) or any(a!=b and not any(offset in span for span in allowed) for offset,(a,b) in enumerate(zip(before,indexed_bytes))):
            raise RuntimeError("Indexed update changed a byte outside its field/entry spans")
        subprocess.run([str(args.cli),"inspect",str(indexed),"--table","Items","--check"],check=True,stdout=subprocess.DEVNULL)
        cli_indexed=Path(directory)/"cli-indexed.mdb"
        report=subprocess.check_output([str(args.cli),"query","update","UPDATE Items SET Id=27 WHERE Id=2","--database",str(args.jet),"--output",str(cli_indexed)],text=True)
        if report!="Matched rows\t1\nChanged rows\t1\n" or cli_indexed.read_bytes()!=indexed_bytes:
            raise RuntimeError("CLI indexed UPDATE differs from native plan")
        for number, (column, expected, value) in enumerate([("Score", 20, 101), ("Score", 99, 27), ("Id", 2, 1)]):
            rejected = Path(directory) / f"rejected-{number}.mdb"
            result = subprocess.run([str(args.example), str(args.jet), str(rejected), "Items", column, "1", str(expected), str(value)], capture_output=True)
            if result.returncode == 0 or rejected.exists():
                raise RuntimeError("Unqualified edit published an output")
        for number, sql in enumerate(["UPDATE Items SET Score=101 WHERE Id=2", "UPDATE Items SET Id=1 WHERE Id=2"]):
            rejected = Path(directory) / f"cli-rejected-{number}.mdb"
            result = subprocess.run([str(args.cli), "query", "update", sql, "--database", str(args.jet), "--output", str(rejected)], capture_output=True)
            if result.returncode == 0 or rejected.exists():
                raise RuntimeError("CLI published a rejected rule/index edit")
        print("Jet4 independent row/index reopen; indexed key reorder; exact byte preservation; rule/stale/duplicate rejections passed")
        print("before", hashlib.sha256(before).hexdigest(), "after", hashlib.sha256(after).hexdigest())
        print("indexed_after",hashlib.sha256(indexed_bytes).hexdigest())
    if args.jet.read_bytes() != before:
        raise RuntimeError("Original fixture changed")


if __name__ == "__main__":
    main()
