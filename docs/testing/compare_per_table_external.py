"""Original getter-backed per-table external selector engineering replay.

Jackcess reads metadata and ordinary records only; it never executes query SQL.
The result model is ordinary integer selection/join, not a native Access oracle.
"""

import argparse
import base64
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019", "V2000", "V2007")
NODES = ("root", "target", "other")
JAR_HASH = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
TOKEN = "remote-token"
PUNCTUATED = "opaque ' é [x]; -- token"
WINDOWS = "C:\\temp\\Database1.accdb"
PRIMARY = "https://learn.microsoft.com/en-us/troubleshoot/microsoft-365-apps/access/operation-is-not-supported-error"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return str(path.resolve().relative_to(ROOT))


def decode(value):
    if value == "N":
        return None
    if value.startswith("I"):
        return int(value[1:])
    data = base64.b64decode(value[1:], validate=True)
    return data.hex() if value.startswith("B") else data.decode("utf-8")


def getter_rows(path):
    return [[decode(cell) for cell in line.split("\t")]
            for line in path.read_text().splitlines()]


def bracket(token):
    return "[" + token.replace("]", "]]") + "]"


def invoke(command, observations):
    command = [relative(value) if isinstance(value, Path) else str(value)
               for value in command]
    result = subprocess.run(
        command, capture_output=True, text=True, timeout=120, cwd=ROOT,
    )
    observations.append({
        "command": command,
        "exit": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
    })
    return result


def generate(args):
    assert not args.directory.exists(), "Refuse to overwrite an original corpus"
    assert digest(args.jar) == JAR_HASH
    recipe = ROOT / "docs/testing/PerTableExternalFixture.java"
    args.directory.mkdir(parents=True)
    classes = args.directory / "classes"
    classes.mkdir()
    (args.directory / "generation-runner.py").write_bytes(Path(__file__).read_bytes())
    observations = []
    result = invoke([
        "javac", "-cp", args.jar, "-d", classes, recipe,
    ], observations)
    assert result.returncode == 0, result.stderr
    classpath = f"{args.jar}:{classes}"
    records = []
    for profile in PROFILES:
        folder = args.directory / profile
        folder.mkdir()
        result = invoke([
            "java", "-cp", classpath, "PerTableExternalFixture", "write", folder, profile,
        ], observations)
        assert result.returncode == 0, result.stderr
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        for node in NODES:
            path = folder / (node + suffix)
            before = digest(path)
            result = invoke([
                "java", "-cp", classpath, "PerTableExternalFixture", "read", path,
            ], observations)
            assert result.returncode == 0, result.stderr
            getter = folder / (node + "-getters.tsv")
            getter.write_text(result.stdout)
            assert digest(path) == before, "Getter must not mutate original bytes"
            rows = getter_rows(getter)
            records.append({
                "profile": profile,
                "node": node,
                "path": relative(path),
                "sha256": before,
                "getter": relative(getter),
                "getter_sha256": digest(getter),
                "records": rows,
            })
    report = {
        "scope": "Original inert fragments and public ordinary getters; no SQL execution",
        "license": "Repository MIT original authoring and replay code",
        "primary": PRIMARY,
        "recipe": relative(recipe),
        "recipe_sha256": digest(recipe),
        "runner": relative(Path(__file__)),
        "runner_sha256": digest(Path(__file__)),
        "jar": relative(args.jar),
        "jar_sha256": digest(args.jar),
        "class": relative(classes / "PerTableExternalFixture.class"),
        "class_sha256": digest(classes / "PerTableExternalFixture.class"),
        "originals": records,
        "commands": observations,
    }
    (args.directory / "generation.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Generated {len(records)} unchanged original databases across four profiles")


def replay(args):
    assert args.cli and args.cli_sha256 and args.source_manifest and args.manifest_sha256
    assert args.report and not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == args.cli_sha256
    assert digest(args.source_manifest) == args.manifest_sha256
    assert digest(args.jar) == JAR_HASH
    generation = args.directory / "generation.json"
    authored = json.loads(generation.read_text())
    assert digest(ROOT / authored["recipe"]) == authored["recipe_sha256"]
    assert digest(ROOT / authored["class"]) == authored["class_sha256"]
    for original in authored["originals"]:
        assert digest(ROOT / original["path"]) == original["sha256"]
        assert digest(ROOT / original["getter"]) == original["getter_sha256"]
    pins = {args.cli, args.jar, generation, args.source_manifest, Path(__file__).resolve()}
    pins.add(ROOT / "docs/testing/PerTableExternalFixture.java")
    pins.add(ROOT / "Cargo.lock")
    pins.update(path for path in args.directory.rglob("*") if path.is_file())
    for line in args.source_manifest.read_text().splitlines():
        name, expected = line.split("\0")
        path = ROOT / name
        assert digest(path) == expected, name
        pins.add(path)
    generation_runner = args.directory / "generation-runner.py"
    if generation_runner.exists():
        assert digest(generation_runner) == authored["runner_sha256"]
        pins.add(generation_runner)
    before = {relative(path): digest(path) for path in sorted(pins)}
    observations = []
    failures = []
    modeled_cells = 0

    def check(label, command, expected=None, refusal=False, contains=None):
        nonlocal modeled_cells
        result = invoke(command, observations)
        entry = observations[-1]
        entry["label"] = label
        entry["expected_refusal"] = refusal
        entry["expected_rows"] = expected
        problem = None
        if refusal:
            if result.returncode == 0 or result.stdout:
                problem = "Expected buffered refusal without stdout"
            if contains and contains not in result.stderr:
                problem = "Missing expected diagnostic"
        elif result.returncode != 0:
            problem = "Unexpected command failure"
        elif expected is not None:
            actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
            if actual != expected:
                problem = "Getter-model row disagreement"
                entry["actual_rows"] = actual
            modeled_cells += sum(len(row) for row in expected[1:])
        if problem:
            failures.append({"label": label, "problem": problem})
        return result.stdout

    for profile in PROFILES:
        entries = {item["node"]: item for item in authored["originals"]
                   if item["profile"] == profile}
        paths = {node: ROOT / item["path"] for node, item in entries.items()}
        rows = {node: item["records"] for node, item in entries.items()}
        remote = sorted(row[2] for row in rows["target"]
                        if row[:2] == ["CELL", "Remote"])
        other = sorted(row[2] for row in rows["other"]
                       if row[:2] == ["CELL", "Remote"])
        local = {row[2]: row[3] for row in rows["root"]
                 if row[:2] == ["CELL", "Local"]}
        assert remote == [1, 2, 3] and other == [2, 3, 4]
        assert [row[2] for row in rows["root"] if row[:2] == ["CELL", "Remote"]] == [999]
        raw = [row for row in rows["root"] if row[0] == "RAW" and row[2] == 5]
        assert any(row[3] == PUNCTUATED and row[6] == "Remote" for row in raw)
        assert any(row[3] == WINDOWS and row[6] == "Remote" for row in raw)
        assert all(row[4:6] == [None, None] for row in raw)
        globals_by_name = {row[1]: row[2:] for row in rows["root"]
                           if row[0] == "GLOBAL"}
        assert globals_by_name["GlobalAndLeaf"] == ["default-token", None]
        assert all(value == [None, None] for name, value in globals_by_name.items()
                   if name != "GlobalAndLeaf")
        getters = [row for row in rows["root"] if row[0] == "FROM"]
        assert any(f"[{PUNCTUATED}].Remote" in row[2] for row in getters)
        base = [args.cli, "query", "run", "--database", paths["root"]]
        bindings = ["--external", TOKEN, paths["target"]]
        target_rows = [["ID"], *[[str(value)] for value in remote]]
        for name in globals_by_name:
            command = [args.cli, "query", "saved", "--database", paths["root"], name]
            if name == "EmptyQualifier":
                check(f"{profile}/reconstruct-empty", command, refusal=True)
                continue
            normalized = check(f"{profile}/reconstruct/{name}", command)
            if normalized:
                check(f"{profile}/reconstruct-owned-reparse/{name}",
                      [args.cli, "query", "parse", normalized])
        check(f"{profile}/root-shadow", [*base, "SELECT ID FROM Remote"], [["ID"], ["999"]])
        for sql in [
            "SELECT ID FROM [remote-token].Remote",
            "SELECT r.ID FROM [remote-token].[Remote] r",
            "SELECT r.* FROM [remote-token].Remote AS r",
        ]:
            check(f"{profile}/direct/{sql}", [*base, sql, *bindings], target_rows)
        for name in ("RemoteOnly", "RemoteAlias", "Wrapper"):
            check(f"{profile}/saved/{name}", [*base, "--saved", name, *bindings],
                  [["Person"], *[[str(value)] for value in remote]])
        check(f"{profile}/named-root", [*base, "SELECT Person FROM Wrapper", *bindings],
              [["Person"], *[[str(value)] for value in remote]])
        joined = [["Person", "Factor"], *[[str(value), str(local[value])]
                  for value in remote if value in local]]
        check(f"{profile}/saved-mixed-join", [*base, "--saved", "MixedJoin", *bindings], joined)
        check(f"{profile}/direct-mixed-join", [*base,
              "SELECT r.ID,l.Factor FROM Local l INNER JOIN [remote-token].Remote r ON l.ID=r.ID ORDER BY r.ID",
              *bindings], [["ID", "Factor"], *joined[1:]])
        for token, name in ((PUNCTUATED, "PunctuatedToken"), (WINDOWS, "WindowsPath")):
            authorized = ["--external", token, paths["target"]]
            sql = f"SELECT r.ID FROM {bracket(token)}.Remote r"
            check(f"{profile}/opaque-direct/{name}", [*base, sql, *authorized], target_rows)
            check(f"{profile}/opaque-saved/{name}", [*base, "--saved", name, *authorized],
                  [["Person"], *target_rows[1:]])
            normalized = check(f"{profile}/owned-display/{name}", [args.cli, "query", "parse", sql])
            if normalized:
                repeated = check(
                    f"{profile}/owned-reparse/{name}",
                    [args.cli, "query", "parse", normalized],
                )
                if repeated != normalized:
                    failures.append({
                        "label": f"{profile}/owned-reparse/{name}",
                        "problem": "Owned normalized display is not stable on reparse",
                    })
        for label, extra in (
            ("missing", []),
            ("case-changed-binding", ["--external", "Remote-token", paths["target"]]),
        ):
            check(f"{profile}/missing-exact/{label}", [*base,
                  "SELECT ID FROM [remote-token].Remote", *extra], refusal=True)
        check(f"{profile}/casechanged-selector", [*base,
              "SELECT ID FROM [Remote-token].Remote", *bindings], refusal=True)
        check(f"{profile}/empty-selector", [*base, "--saved", "EmptyQualifier", *bindings], refusal=True)
        for sql, saved in (("SELECT ID,Score FROM [remote-token].TargetView", False),
                           ("TargetNamed", True)):
            command = [*base, "--saved", sql, *bindings] if saved else [*base, sql, *bindings]
            check(f"{profile}/target-named/{saved}", command,
                  [["ID", "Score"], *[[str(value), str(value * 10)] for value in remote]])
        combined = [*bindings, "--external", "default-token", paths["other"]]
        overlap = sorted(set(remote) & set(other))
        check(f"{profile}/global-and-leaf-saved", [*base, "--saved", "GlobalAndLeaf", *combined],
              [["RemoteID", "GlobalID"], *[[str(value), str(value)] for value in overlap]])
        check(f"{profile}/global-and-leaf-direct", [*base,
              "SELECT r.ID AS RemoteID,g.ID AS GlobalID FROM [remote-token].Remote r INNER JOIN Remote g ON r.ID=g.ID IN 'default-token' ORDER BY r.ID",
              *combined], [["RemoteID", "GlobalID"], *[[str(value), str(value)] for value in overlap]])
        check(f"{profile}/positive-quotas", [*base, "SELECT ID FROM [remote-token].Remote", *bindings,
              "--execution-bytes", "1000000", "--execution-rows", "10000", "--execution-steps", "100000"], target_rows)
        check(f"{profile}/late-error", [*base, "SELECT ID,1/(3-ID) FROM [remote-token].Remote", *bindings],
              refusal=True, contains="division by zero")
        check(f"{profile}/cumulative-quota", [*base, "SELECT ID FROM [remote-token].Remote", *bindings,
              "--execution-steps", "1"], refusal=True)
    after = {name: digest(ROOT / name) for name in before}
    assert before == after, "Pinned original/source/tool bytes changed"
    report = {
        "scope": "Original public getters plus independent per-table integer row model; no native Access SQL execution",
        "primary": PRIMARY,
        "pins": before,
        "command_count": len(observations),
        "modeled_cells": modeled_cells,
        "commands": observations,
        "failures": failures,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"{len(observations)} commands; {modeled_cells} modeled cells; {len(failures)} disagreements; {len(before)} unchanged pins")
    assert not failures, failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--cli-sha256")
    parser.add_argument("--source-manifest", type=Path)
    parser.add_argument("--manifest-sha256")
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    for name in ("directory", "jar", "cli", "source_manifest", "report"):
        value = getattr(args, name)
        if value is not None:
            setattr(args, name, Path(relative(value)))
    if args.generate:
        generate(args)
    else:
        replay(args)


if __name__ == "__main__":
    main()
