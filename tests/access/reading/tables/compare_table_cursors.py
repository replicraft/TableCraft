"""Compare complete resumable scalar batches with inert Jackcess getters.

Uses the existing immutable large-map corpus; never regenerates input fixtures.
The tagged example is engineering output, without the CLI's atomic output bound.
"""
import argparse
from pathlib import Path
import subprocess
from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--rows", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--mixed", type=Path, action="append", default=[],
                        help="Existing CalculatedScalarFixture input (repeatable)")
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
            # Request reversed names; native column order remains ID, P2, P5.
            selected = "".join("LargeRows\t" + "\t".join(row.split("\t")[i] for i in [0, 3, 6]) + "\n" for row in oracle[2:])
            projected = run(args.rows, path, "LargeRows", "--batch-size", batch,
                            "--column", "P5", "--column", "ID", "--column", "P2")
            assert projected == selected, (profile, "projected batch", batch)
        assert digest(path) == before, (profile, "input changed")
        print(f"{profile}: all 6000 full/projected rows agree at batch sizes 1, 127, 250; unchanged SHA-256 {before}")
    for path in args.mixed:
        before = digest(path)
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "TableDataOracle.java", path, "CalculatedScalars").splitlines()
        assert len(oracle) == 5, "mixed oracle row count"
        expected = "".join("\t".join(row.split("\t")[:2]) + "\n" for row in oracle)
        for batch in [1, 2, 3]:
            assert run(args.rows, path, "CalculatedScalars", "--batch-size", batch,
                       "--column", "Id") == expected, "mixed ordinary scalar cursor"
        for selected in [[], ["--column", "Flag"]]:
            result = subprocess.run([str(args.rows), str(path), "CalculatedScalars",
                                     "--batch-size", "2", *selected], capture_output=True)
            assert result.returncode != 0 and not result.stdout, "mixed unavailable selection"
        assert digest(path) == before, "mixed input changed"
        print(f"Mixed: 5 ordinary-scalar/refusal controls agree; unchanged SHA-256 {before}")


if __name__ == "__main__":
    main()
