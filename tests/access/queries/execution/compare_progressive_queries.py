"""Original getter-based large progressive SQL replay; no reference SQL execution."""

import argparse
import base64
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2000", "V2007", "V2016", "V2019")
BATCHES = (1, 128, 1000)
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
ROWS = 20_000


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def display(value):
    if value is None:
        return "NULL"
    if not isinstance(value, str):
        return str(value)
    escapes = {"\\": "\\\\", '"': '\\"', "\t": "\\t", "\n": "\\n", "\r": "\\r"}
    pieces = []
    for character in value:
        if character in escapes:
            pieces.append(escapes[character])
        elif ord(character) < 32 or 127 <= ord(character) <= 159:
            pieces.append("\\u{" + format(ord(character), "04x") + "}")
        else:
            pieces.append(character)
    return '"' + "".join(pieces) + '"'


def getter_rows(path):
    columns = []
    rows = []
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if fields[0] == "COLUMN":
            columns.append(fields[1:])
        else:
            assert fields[0] == "ROW"
            identity = int(fields[1])
            score = None if fields[2] == "N" else int(fields[2])
            label = None if fields[3] == "N" else base64.b64decode(fields[3][1:]).decode("utf-8")
            rows.append((identity, score, label))
    assert columns == [["ID", "LONG"], ["Score", "LONG"], ["Label", "TEXT"]]
    assert [row[0] for row in rows] == list(range(1, ROWS + 1))
    assert any(label is not None and "🦀" in label for _, _, label in rows)
    assert any(score is None and label is None for _, score, label in rows)
    return rows


def trunc_remainder(left, right):
    assert right != 0
    quotient = abs(left) // abs(right)
    if (left < 0) != (right < 0):
        quotient = -quotient
    return left - quotient * right


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "source_manifest", "report"):
        parser.add_argument("--" + name.replace("_", "-"), type=Path, required=True)
    parser.add_argument("--cli-sha256", required=True)
    parser.add_argument("--manifest-sha256", required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.jar) == JAR_SHA256
    assert digest(args.source_manifest) == args.manifest_sha256
    source_hashes = dict(line.split("\0") for line in args.source_manifest.read_text().splitlines())
    paths = {args.cli, args.jar, args.source_manifest, Path(__file__)}
    paths.update(path for path in args.directory.iterdir() if path.is_file())
    generation = json.loads((args.directory / "generation.json").read_text())
    compiled = ROOT / generation["compiled_class"]
    assert digest(compiled) == generation["compiled_class_sha256"]
    paths.add(compiled)
    paths.update(ROOT / "docs/testing" / name for name in (
        "ProgressiveQueryFixture.java",
    ))
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path)
              for path in sorted(paths)}
    for name, expected in source_hashes.items():
        assert digest(ROOT / name) == expected, name
    hashes.update(source_hashes)
    observations = []
    matched_cells = 0

    def execute(label, database, sql, expected, batch=None, extra=(), refusal=None,
                exact_prefix=None, require_prefix=False):
        nonlocal matched_cells
        command = [str(args.cli), "--format", "tsv", "query", "run", "--database", str(database),
                   "--native-bytes", "268435456", "--native-rows", "1000000", "--native-steps", "10000000",
                   "--execution-bytes", "268435456", "--execution-rows", "1000000",
                   "--execution-steps", "10000000", "--output-bytes", "16777216"]
        if batch is not None:
            command.extend(["--stream", "--batch-rows", str(batch)])
        for option, value in extra:
            if option in command:
                command[command.index(option) + 1] = value
            elif value is None:
                command.append(option)
            else:
                command.extend([option, value])
        command.append(sql)
        result = subprocess.run(command, capture_output=True, timeout=180)
        stdout = result.stdout.decode("utf-8")
        stderr = result.stderr.decode("utf-8")
        actual = list(csv.reader(StringIO(stdout), delimiter="\t", strict=True))
        if refusal is None:
            assert result.returncode == 0 and not stderr, (label, stderr)
            assert actual == expected, (label, actual[:2], expected[:2], len(actual), len(expected))
        else:
            assert result.returncode != 0 and refusal in stderr, (label, stderr)
            if exact_prefix is not None:
                prefix = [] if exact_prefix == 0 else expected[:exact_prefix + 1]
                assert actual == prefix, (label, len(actual), exact_prefix)
            elif actual:
                assert actual[0] == expected[0]
                assert actual == expected[:len(actual)], label
                assert 0 < len(actual) - 1 < len(expected) - 1, label
                assert batch is not None and (len(actual) - 1) % batch == 0, label
            if require_prefix:
                assert len(actual) > 1, (label, "quota did not reach a published batch")
        cells = sum(map(len, actual[1:]))
        matched_cells += cells
        observations.append({"label": label, "command": command, "exit": result.returncode,
                             "stdout_bytes": len(result.stdout),
                             "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                             "matched_rows": max(0, len(actual) - 1), "matched_cells": cells,
                             "stderr": stderr, "expected_refusal": refusal,
                             "exact_successful_prefix_rows": exact_prefix})

    summaries = []
    for profile in PROFILES:
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        database = args.directory / (profile + suffix)
        rows = getter_rows(database.with_suffix(suffix + ".getters.tsv"))
        complete = [["ID", "Score", "Label"], *[
            [str(identity), display(score), display(label)] for identity, score, label in rows
        ]]
        filtered = [["Label", "ID"], *[
            [display(label), str(identity)] for identity, _, label in rows if identity % 97 == 0
        ]]
        declared = [["ID", "Eq", "Caption"], *[
            [str(identity), "FALSE", display(str(identity) + "x")]
            for identity, _, _ in rows if identity % 97 == 0
        ]]
        shapes = (
            ("full", "SELECT ID,Score,Label FROM StreamingRows", complete, ()),
            ("filtered", "SELECT Label,ID FROM StreamingRows WHERE ID Mod 97=0", filtered, ()),
            ("declared", "SELECT ID,'02'=CVar(CLng(2)) AS Eq,CStr(ID)&'x' AS Caption "
             "FROM StreamingRows WHERE ID Mod 97=0", declared,
             (("--declared-arithmetic", None), ("--number-format", "invariant"), ("--collation", "ordinal"))),
        )
        for name, sql, expected, extra in shapes:
            for batch in (*BATCHES, None):
                execute(f"{profile}:{name}:{batch}", database, sql, expected, batch, extra)
        execute(profile + ":empty", database, "SELECT ID FROM StreamingRows WHERE ID<0", [["ID"]], 128)
        late = [["ID", "R"], *[
            [str(identity), str(trunc_remainder(identity, identity - 15000))]
            for identity, _, _ in rows if identity < 15000
        ]]
        for batch in (*BATCHES, None):
            prefix = 0 if batch is None else (14999 // batch) * batch
            execute(f"{profile}:late-error:{batch}", database,
                    "SELECT ID,ID Mod (ID-15000) AS R FROM StreamingRows", late, batch,
                    refusal="division by zero", exact_prefix=prefix)
        for option, value in (("--native-rows", "1000"), ("--execution-rows", "1000"),
                              ("--native-steps", "1"), ("--execution-bytes", "1")):
            execute(profile + ":quota:" + option, database, shapes[0][1], complete, 128,
                    ((option, value),), refusal="resource limit",
                    require_prefix=option in ("--native-rows", "--execution-rows"))
        execute(profile + ":output-limit", database, shapes[0][1], complete, 128,
                (("--output-bytes", "1"),), refusal="output limit", exact_prefix=0)
        summaries.append({"profile": profile, "getter_rows": len(rows),
                          "null_scores": sum(score is None for _, score, _ in rows),
                          "null_labels": sum(label is None for _, _, label in rows),
                          "filtered_rows": len(filtered) - 1})
    for name, expected in hashes.items():
        assert digest(ROOT / name) == expected, name
    report = {"scope": "Original public getter/model engineering evidence for explicit native query streaming; "
                       "not native Access qualification or a performance benchmark.",
              "controls": len(observations), "matched_cells": matched_cells,
              "source_files_verified_before_after": len(source_hashes),
              "profiles": summaries, "hashes": hashes, "observations": observations}
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"controls": len(observations), "matched_cells": matched_cells,
                      "hashes": len(hashes), "report": str(args.report)}))


if __name__ == "__main__":
    main()
