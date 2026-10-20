"""Compare bounded materialized native queries with independent stored integer rows.

Uses existing immutable large-map inputs. Native Access SQL semantics, streaming
execution and shared-file consistency are separate qualification targets.
"""
import argparse
from pathlib import Path
import subprocess
from compare_browsing import PROFILES, RECIPES, run
from compare_large_tables import digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--query", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    for profile, suffix in PROFILES:
        path = args.directory / f"LargeRows-{profile}.{suffix}"
        before = digest(path)
        oracle = run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar,
                     RECIPES / "LargeTableOracle.java", path, 0, 6000).splitlines()
        ids = [row.split("\t")[0] for row in oracle[2:]]
        assert len(ids) == 6000, "oracle row count"
        cases = [
            ("SELECT ID FROM LargeRows ORDER BY ID", ids),
            ("SELECT TOP 3 ID FROM LargeRows ORDER BY ID DESC", list(reversed(ids[-3:]))),
            ("SELECT ID FROM LargeRows WHERE ID >= 5998 ORDER BY ID", ids[-3:]),
            ("SELECT Count(*) AS N FROM LargeRows", ["I64:6000"]),
            ("SELECT A.ID FROM (SELECT TOP 3 ID FROM LargeRows ORDER BY ID DESC) AS A WHERE EXISTS (SELECT B.ID FROM LargeRows AS B WHERE B.ID = A.ID) ORDER BY A.ID", ids[-3:]),
        ]
        for sql, expected in cases:
            assert run(args.query, path, sql, "--bytes", 64 * 1024 * 1024) == "".join(row + "\n" for row in expected), (profile, sql)
        refused = subprocess.run([str(args.query), str(path), cases[0][0]], capture_output=True)
        assert refused.returncode != 0 and not refused.stdout and b"Limit" in refused.stderr, (profile, "default byte budget")
        assert digest(path) == before, (profile, "input changed")
        print(f"{profile}: 5 complete query controls and default-budget refusal agree; unchanged SHA-256 {before}")


if __name__ == "__main__":
    main()
