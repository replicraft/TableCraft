"""Compare complete resumable scalar batches with inert Jackcess getters.

Uses the existing immutable large-map corpus; never regenerates input fixtures.
The tagged example is engineering output, without the CLI's atomic output bound.
"""
import argparse
from pathlib import Path
from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--rows", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    for profile, suffix in PROFILES:
        path = args.directory / f"LargeRows-{profile}.{suffix}"
        before = digest(path)
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "LargeTableOracle.java", path, 0, 6000).splitlines()
        assert len(oracle) == 6002, (profile, "oracle row count")
        expected = "".join("LargeRows\t" + row + "\n" for row in oracle[2:])
        for batch in [1, 127, 250]:
            actual = run(args.rows, path, "LargeRows", "--batch-size", batch)
            assert actual == expected, (profile, "batch", batch)
        assert digest(path) == before, (profile, "input changed")
        print(f"{profile}: all 6000 rows agree at batch sizes 1, 127, 250; unchanged SHA-256 {before}")


if __name__ == "__main__":
    main()
