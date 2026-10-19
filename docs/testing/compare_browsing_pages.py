"""Compare bounded CLI pages of original >16,384-row Jet 4/ACE 16 tables.

Requires the reviewed Jackcess artifact. Generated database bytes stay local;
only the original MIT recipe is tracked. No native Access qualification implied.
"""
import argparse
import hashlib
from pathlib import Path
from compare_browsing import RECIPES, PROFILES, display, escaped, run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", required=True, type=Path)
    parser.add_argument("--cli", required=True, type=Path)
    parser.add_argument("--directory", required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, suffix in PROFILES:
            run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / "BrowsingFixture.java", args.directory / f"Browsing-{profile}.{suffix}", profile)
    for profile, suffix in PROFILES:
        path = args.directory / f"Browsing-{profile}.{suffix}"
        before = path.read_bytes()
        family = {"V2000": 1, "V2016": 5, "V2019": 6}[profile]
        assert before[20:24] == bytes([family, 0, 0, 0])
        def oracle(recipe, *extra):
            return run("java", "-Dstdout.encoding=UTF-8", "-cp", args.jar, RECIPES / f"{recipe}.java", path, *extra)
        header = "\t".join(escaped(line.split("\t")[1]) for line in oracle("TableSchemaOracle", "Browse rows").splitlines()) + "\n"
        rows = ["\t".join(display(tag) for tag in line.split("\t")[1:]) + "\n" for line in oracle("TableDataOracle", "Browse rows").splitlines()]
        assert len(rows) == 20_050
        for offset, limit in [(0, 1), (0, 100), (199, 3), (4999, 2), (16383, 3), (17999, 4), (19999, 2), (20048, 10), (20050, 1), (4294967295, 1)]:
            actual = run(args.cli, "rows", path, "Browse rows", "--offset", offset, "--limit", limit)
            assert actual == header + "".join(rows[offset:offset + limit]), (profile, offset, limit)
        assert path.read_bytes() == before
        print(f"{profile}: 20,050 independent rows, 10 page controls; unchanged SHA-256 {hashlib.sha256(before).hexdigest()}")


if __name__ == "__main__":
    main()
