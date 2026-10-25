"""Original getter/model replay for progressive unordered TOP counts."""

import argparse
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess

from compare_progressive_queries import BATCHES, PROFILES, ROOT, digest, display, getter_rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "baseline", "directory", "source_manifest", "report"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--baseline-sha256", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.baseline) == args.baseline_sha256
    assert digest(args.source_manifest) == args.manifest_sha256
    sources = dict(line.split("\0") for line in args.source_manifest.read_text().splitlines())
    paths = {args.cli, args.baseline, args.source_manifest, Path(__file__),
             ROOT / "docs/testing/compare_progressive_queries.py",
             ROOT / "docs/testing/ProgressiveQueryFixture.java"}
    paths.update(path for path in args.directory.iterdir() if path.is_file())
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path)
              for path in sorted(paths)}
    hashes.update(sources)
    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    observations = []
    matched_cells = 0

    def execute(label, database, sql, expected, batch=None, refusal=None,
                published=None, extra=(), baseline=False):
        nonlocal matched_cells
        binary = args.baseline if baseline else args.cli
        command = [str(binary), "--format", "tsv", "query", "run", "--database", str(database),
                   "--native-bytes", "268435456", "--native-rows", "1000000",
                   "--native-steps", "10000000", "--execution-bytes", "268435456",
                   "--execution-rows", "1000000", "--execution-steps", "10000000",
                   "--output-bytes", "16777216"]
        if batch is not None:
            command.extend(["--stream", "--batch-rows", str(batch)])
        for option, value in extra:
            assert option in command
            command[command.index(option) + 1] = value
        command.append(sql)
        result = subprocess.run(command, capture_output=True, timeout=180)
        actual = list(csv.reader(StringIO(result.stdout.decode("utf-8")),
                                 delimiter="\t", strict=True))
        stderr = result.stderr.decode("utf-8")
        if refusal is None:
            assert result.returncode == 0 and not stderr, (label, stderr)
            assert actual == expected, (label, actual[:2], len(actual), len(expected))
        else:
            assert result.returncode != 0 and refusal in stderr, (label, stderr)
            assert published is not None
            prefix = [] if published == 0 else expected[:published + 1]
            assert actual == prefix, (label, len(actual), published)
        cells = sum(map(len, actual[1:]))
        matched_cells += cells
        observations.append(dict(label=label, command=command, exit=result.returncode,
                                 stderr=stderr, matched_cells=cells,
                                 matched_rows=max(0, len(actual) - 1),
                                 stdout_bytes=len(result.stdout),
                                 stdout_sha256=hashlib.sha256(result.stdout).hexdigest(),
                                 expected_refusal=refusal, published_rows=published))

    for profile in PROFILES:
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        database = args.directory / (profile + suffix)
        rows = getter_rows(database.with_suffix(suffix + ".getters.tsv"))
        full = [["ID", "Score", "Label"], *[
            [str(identity), display(score), display(label)] for identity, score, label in rows
        ]]
        filtered = [["ID", "Score", "Label"], *[
            [str(identity), display(score), display(label)]
            for identity, score, label in rows if identity % 97 == 0
        ]]
        shapes = [
            ("one", "SELECT TOP 1 ID,Score,Label FROM StreamingRows", full[:2]),
            ("batch-edge", "SELECT TOP 129 ID,Score,Label FROM StreamingRows", full[:130]),
            ("over-cardinality", "SELECT TOP 20001 ID,Score,Label FROM StreamingRows", full),
            ("filtered", "SELECT TOP 129 ID,Score,Label FROM StreamingRows WHERE ID Mod 97=0",
             filtered[:130]),
            ("empty", "SELECT TOP 1 ID FROM StreamingRows WHERE ID<0", [["ID"]]),
            ("discarded-unicode", "SELECT TOP 1 IIf(ID=1,'ok',ChrW(55296)) AS Label "
             "FROM StreamingRows", [["Label"], [display("ok")]]),
        ]
        for name, sql, expected in shapes:
            for batch in (*BATCHES, None):
                execute(f"{profile}:{name}:{batch}", database, sql, expected, batch)
            execute(f"{profile}:{name}:baseline", database, sql, expected, baseline=True)
        sql = "SELECT TOP 129 ID,ID Mod (ID-15000) AS R FROM StreamingRows"
        late = [["ID", "R"], *[[str(identity), str(identity)] for identity in range(1, 130)]]
        for batch in (*BATCHES, None):
            published = 0 if batch is None else (129 // batch) * batch
            execute(f"{profile}:late-error:{batch}", database, sql, late, batch,
                    refusal="division by zero", published=published)
        execute(f"{profile}:late-error:baseline", database, sql, late,
                refusal="division by zero", published=0, baseline=True)
        sql = "SELECT TOP 129 ID,Score,Label FROM StreamingRows"
        for option in ("--native-rows", "--execution-rows"):
            execute(f"{profile}:post-cap-quota:{option}", database, sql, full[:130], 128,
                    refusal="resource limit", published=128, extra=((option, "1000"),))
        execute(f"{profile}:percent-refusal", database,
                "SELECT TOP 1 PERCENT ID FROM StreamingRows", [["ID"]], 128,
                refusal="query feature is not implemented", published=0)
        execute(f"{profile}:ordered-refusal", database,
                "SELECT TOP 1 ID FROM StreamingRows ORDER BY ID", [["ID"]], 128,
                refusal="query feature is not implemented", published=0)
    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    report = dict(scope="Original getter/model unordered TOP streaming evidence; "
                        "full-source validation and buffered baseline parity; "
                        "not native Access qualification",
                  controls=len(observations), matched_cells=matched_cells,
                  source_files_verified_before_after=len(sources), hashes=hashes,
                  observations=observations)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(dict(controls=len(observations), matched_cells=matched_cells,
                          hashes=len(hashes), report=str(args.report))))


if __name__ == "__main__":
    main()
