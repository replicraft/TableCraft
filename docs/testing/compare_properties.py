"""Compare all original fixture properties against an independent Jackcess read."""

from cli_tsv import display_tsv
import argparse
import csv
import hashlib
import io
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ["cli", "jar", "ace", "jet"]:
        parser.add_argument("--" + argument, required=True, type=Path)
    parser.add_argument("--ace-label", default="ace")
    parser.add_argument("--jet-label", default="jet4")
    args = parser.parse_args()
    for label, path in [(args.ace_label, args.ace), (args.jet_label, args.jet)]:
        before = hashlib.sha256(path.read_bytes()).hexdigest()
        expected = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            "docs/testing/PropertyFixture.java", "--inspect", str(path),
        ], text=True, encoding="utf-8")
        actual = display_tsv(subprocess.check_output([
            str(args.cli), "inspect", str(path), "--table", "Rules", "--properties",
        ], text=True, encoding="utf-8"))
        # The CLI escapes identities/text while the original recipe uses no
        # control characters. Decode its escaped quotes before comparing cells.
        actual_rows = list(csv.reader(io.StringIO(actual), delimiter="\t", quoting=csv.QUOTE_NONE, escapechar="\\"))[2:]
        expected_rows = [line.split("\t") for line in expected.splitlines()]
        if sorted(actual_rows) != sorted(expected_rows) or len(actual_rows) != 11:
            raise RuntimeError(f"{label}: property disagreement")
        if hashlib.sha256(path.read_bytes()).hexdigest() != before:
            raise RuntimeError(f"{label}: input changed")
        canonical = "\n".join("\t".join(row) for row in sorted(actual_rows)).encode("utf-8")
        print(label, len(actual_rows), hashlib.sha256(canonical).hexdigest(), before, flush=True)


if __name__ == "__main__":
    main()
