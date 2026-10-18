"""Compare all large usage-map fixture rows through the standalone native example."""
import argparse
import hashlib
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["example", "jar", "ace", "jet"]:
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    for format, path in [("ace", args.ace), ("jet4", args.jet)]:
        expected = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            "docs/testing/TableDataOracle.java", str(path), "Pages",
        ])
        actual = subprocess.check_output([
            str(args.example), str(path), "Pages", "--bytes", str(256 * 1024 * 1024),
        ])
        if actual != expected or len(actual.splitlines()) != 2000:
            raise RuntimeError(f"usage-map row disagreement: {format}")
        print(format, 2000, hashlib.sha256(actual).hexdigest(), flush=True)


if __name__ == "__main__":
    main()
