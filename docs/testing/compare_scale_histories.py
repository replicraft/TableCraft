"""Independent 100,000-insert history/scale controls over offline local fixtures.

Uses an original recipe and reviewed read-only Jackcess getters, not native
Access SQL or live-file coordination. All generated binaries remain local.
"""

import argparse
import csv
import hashlib
from io import StringIO
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display
from compare_union_types import JAR_SHA256

INPUT_HASHES = {
    "V2016": "7e3b0c44d6797d2e7f8a718cb4f1ecfbe9b2b0d2fa03cd2d9f99f87c031a1610",
    "V2007": "34440ebc35862f87d6aa70fef77ee67d2c6d376ca4cf99364c051b4704ca8c65",
    "V2000": "db5b19fc11ea155a5e47709fd3e02e8e905da7de4a0612d6fe6ab48c5b28cb44",
}


def digest(path):
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def expected_rows():
    values = {}
    for position in range(100_000):
        identity = position * 7_919 % 100_000 + 1
        if identity % 101 == 0:
            continue
        notes = f"n:{identity}"
        if identity % 97 == 0:
            notes = f"updated:{identity}:" + "é😀" * 128
        later = identity * 2 if position >= 50_000 else None
        values[identity] = (identity % 97 - 48, notes, later)
    for identity in range(100_001, 100_101):
        values[identity] = (identity % 97 - 48, f"append:{identity}", identity * 2)
    return values


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    profiles = [("V2016", "accdb"), ("V2007", "accdb"), ("V2000", "mdb")]
    if args.generate:
        args.directory.mkdir(exist_ok=False)
        for profile, extension in profiles:
            subprocess.run([
                "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
                str(RECIPES / "ScaleHistoryFixture.java"),
                str(args.directory / f"scale-{profile}.{extension}"), profile,
            ], check=True)

    model = expected_rows()
    assert len(model) == 99_110
    native = ["--native-bytes", "268435456", "--native-rows", "1000000", "--native-steps", "100000000"]
    execution = ["--execution-bytes", "268435456", "--execution-rows", "1000000",
                 "--execution-steps", "100000000"]
    controls = 0
    for profile, extension in profiles:
        path = args.directory / f"scale-{profile}.{extension}"
        before = digest(path)
        if not args.generate:
            assert before == INPUT_HASHES[profile], (profile, "unregistered fixture")
        with path.open("rb") as source:
            prefix = source.read(24)
        assert prefix[20:24] == bytes([{"V2000": 1, "V2007": 2, "V2016": 5}[profile], 0, 0, 0])
        reference = subprocess.check_output([
            "java", "-Dstdout.encoding=UTF-8", "-cp", str(args.jar),
            str(RECIPES / "TableDataOracle.java"), str(path), "ScaleRows",
        ], text=True, encoding="utf-8")
        stored = [line.split("\t")[1:] for line in reference.splitlines()]
        identifiers = [int(row[0].removeprefix("I32:")) for row in stored]
        assert len(identifiers) == len(set(identifiers)) == len(model)
        for row, identity in zip(stored, identifiers):
            score, notes, later = model[identity]
            assert row == [f"I32:{identity}", f"I32:{score}", "T" + notes.encode().hex(),
                           "N" if later is None else f"I32:{later}"]

        def query(sql, expected=None, options=(), diagnostic=None):
            nonlocal controls
            result = subprocess.run([
                str(args.cli), "query", "run", sql, "--database", str(path), *options,
            ], capture_output=True, text=True, encoding="utf-8")
            if diagnostic:
                assert result.returncode == 2 and not result.stdout, (profile, sql, result)
                assert diagnostic in result.stderr, (profile, sql, result.stderr)
            else:
                assert result.returncode == 0 and not result.stderr, (profile, sql, result.stderr)
                actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert actual[1:] == expected, (profile, sql, actual, expected)
            controls += 1

        query("SELECT Count(*),Sum(ID),Sum(Score),Count(Later) FROM ScaleRows",
              [[str(len(model)), str(sum(model)), str(sum(row[0] for row in model.values())),
                str(sum(row[2] is not None for row in model.values()))]], native + execution)
        query("SELECT Sum(Len(Notes)) FROM ScaleRows",
              [[str(sum(len(row[1].encode("utf-16-le")) // 2 for row in model.values()))]], native + execution)
        query("SELECT TOP 3 ID FROM ScaleRows ORDER BY ID DESC",
              [[str(identity)] for identity in sorted(model, reverse=True)[:3]], native + execution)
        query("SELECT Count(*) FROM ScaleRows WHERE ID<=100000 AND ID Mod 101=0", [["0"]], native + execution)
        query("SELECT Count(*) FROM ScaleRows", options=execution, diagnostic="limit")
        query("SELECT Count(*),Sum(ID),Sum(Score),Count(Later) FROM ScaleRows",
              options=native + ["--execution-rows", "1000000", "--execution-steps", "100000000"],
              diagnostic="Bytes resource limit exceeded")
        query("SELECT ID FROM ScaleRows",
              options=native + ["--execution-bytes", "268435456", "--execution-steps", "100000000"],
              diagnostic="Rows resource limit exceeded")

        streamed = subprocess.run([
            str(args.cli), "rows", str(path), "ScaleRows", "--stream", "--limit", "257",
            *native,
        ], capture_output=True, text=True, encoding="utf-8")
        assert streamed.returncode == 0 and not streamed.stderr, (profile, streamed.stderr)
        actual = list(csv.reader(StringIO(streamed.stdout), delimiter="\t", strict=True))
        assert actual[0] == ["ID", "Score", "Notes", "Later"]
        assert actual[1:] == [[display(value) for value in row] for row in stored]
        controls += 1
        assert digest(path) == before, (profile, "input changed")
        print(f"{profile}: {len(model)} retained rows; unchanged SHA-256 {before}", flush=True)
    print(f"Passed {controls} history/scale controls; native Access unrun.")


if __name__ == "__main__":
    main()
