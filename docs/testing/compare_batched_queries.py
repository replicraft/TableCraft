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
    parser.add_argument("--cli", type=Path, help="Also verify the explicit CLI native-byte option")
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
            if args.cli:
                header = "N" if "Count(*)" in sql else "ID"
                expected_cli = header + "\n" + "".join(row.split(":", 1)[1] + "\n" for row in expected)
                assert run(args.cli, "query", "run", sql, "--database", path,
                           "--native-bytes", 64 * 1024 * 1024) == expected_cli, (profile, "CLI", sql)
        refused = subprocess.run([str(args.query), str(path), cases[0][0]], capture_output=True)
        assert refused.returncode != 0 and not refused.stdout and b"Limit" in refused.stderr, (profile, "default byte budget")
        if args.cli:
            refused_cli = subprocess.run([str(args.cli), "query", "run", cases[0][0], "--database", str(path)], capture_output=True)
            assert refused_cli.returncode == 2 and not refused_cli.stdout and b"Bytes resource limit exceeded" in refused_cli.stderr, (profile, "CLI default byte budget")
            for sql, error in [
                ("SELECT ID,P0 FROM LargeRows", b"evaluation output limit exceeded"),
                ("SELECT ID,P0,P1,P2,P3,P4,P5 FROM LargeRows", b"Bytes resource limit exceeded"),
            ]:
                refused_result = subprocess.run([str(args.cli), "query", "run", sql, "--database", str(path), "--native-bytes", str(64 * 1024 * 1024)], capture_output=True)
                assert refused_result.returncode == 2 and not refused_result.stdout and error in refused_result.stderr, (profile, "CLI independent execution/output limit")
        assert digest(path) == before, (profile, "input changed")
        print(f"{profile}: 5 complete query controls and default-budget refusal agree" + (" in engineering and CLI hosts; execution/output ceilings also retained" if args.cli else "") + f"; unchanged SHA-256 {before}")


if __name__ == "__main__":
    main()
