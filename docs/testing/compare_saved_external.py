"""Getter-backed saved IN reconstruction and explicitly mapped CLI replay.

Original fragment authoring and independent raw getters are engineering evidence,
not native Microsoft Access saved-query execution or save/reopen qualification.
"""

import argparse
import base64
from collections import Counter
import csv
import hashlib
from io import StringIO
import json
from pathlib import Path
import subprocess


ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2016", "V2019", "V2000", "V2007")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
CLI_SHA256 = "dc67d1ae174734112b412892c5bc8054d4408abffc1cc5f79ac6769bdbd09007"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def decode(value):
    if value == "N":
        return None
    if value.startswith("I"):
        return int(value[1:])
    data = base64.b64decode(value[1:], validate=True)
    return data if value.startswith("B") else data.decode("utf-8")


def display(value):
    if value is None:
        return "NULL"
    if isinstance(value, bytes):
        return "0x" + value.hex()
    if isinstance(value, str):
        escaped = {"\\": "\\\\", '"': '\\"', "\t": "\\t", "\n": "\\n", "\r": "\\r"}
        return '"' + "".join(escaped.get(character, character) for character in value) + '"'
    return str(value)


def read_getters(path):
    return [[decode(value) for value in line.split("\t")] for line in path.read_text().splitlines()]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("cli", "jar", "directory", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    assert not args.report.exists(), "Refuse to overwrite evidence"
    assert digest(args.cli) == CLI_SHA256
    assert digest(args.jar) == JAR_SHA256
    paths = {args.cli, args.jar, Path(__file__)}
    paths.update(ROOT / "docs/testing" / name for name in (
        "SavedExternalFixture.java", "SavedExternalOracle.java",
    ))
    paths.update(args.directory.glob("*/*"))
    paths.add(args.directory / "generation.json")
    paths.add(ROOT / "Cargo.lock")
    for directory in (
        "crates/backends/tablecraft-access/src/saved",
        "crates/backends/tablecraft-access/src/source",
        "apps/tablecraft-cli/src/commands/query",
        "crates/query/tablecraft-sql/src/select",
    ):
        paths.update((ROOT / directory).rglob("*.rs"))
    paths.update(ROOT / name for name in subprocess.check_output([
        "git", "show", "--pretty=format:", "--name-only", "97a0ab35",
    ], text=True).splitlines() if name.endswith(".rs"))
    hashes = {str(path.relative_to(ROOT) if path.is_absolute() else path): digest(path) for path in sorted(paths)}
    observations = []

    def check(label, command, expected=None, refusal=False, contains=()):
        command = list(map(str, command))
        result = subprocess.run(command, capture_output=True, text=True, timeout=120)
        if refusal:
            assert result.returncode != 0 and not result.stdout, (label, result.stdout, result.stderr)
            if ":positive-runtime-limit:" in label:
                message = "output limit exceeded" if label.endswith("--output-bytes") else "resource limit exceeded"
            elif label.endswith(":late-error"):
                message = "division by zero"
            elif label.endswith(":duplicate-map"):
                message = "invalid query run parameter"
            else:
                message = "query feature is not implemented"
            assert message in result.stderr, (label, message, result.stderr)
        else:
            assert result.returncode == 0, (label, result.stderr)
            if expected is not None:
                actual = list(csv.reader(StringIO(result.stdout), delimiter="\t", strict=True))
                assert actual == expected, (label, actual, expected)
            for fragment in contains:
                assert fragment in result.stdout, (label, fragment, result.stdout)
        observations.append({
            "label": label, "command": command, "exit": result.returncode,
            "stdout": result.stdout, "stderr": result.stderr,
        })
        return result.stdout

    for profile in PROFILES:
        directory = args.directory / profile
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        frontend = directory / ("frontend" + suffix)
        backend = directory / ("backend" + suffix)
        front_rows = read_getters(frontend.with_suffix(suffix + ".getters.tsv"))
        back_rows = read_getters(backend.with_suffix(suffix + ".getters.tsv"))
        queries = {row[1]: row for row in front_rows if row[0] == "QUERY"}
        remotes = {row[1]: row[2:] for row in front_rows if row[0] == "REMOTE"}
        assert len(queries) == 12
        assert "IN ''" in queries["EmptyPath"][5]
        assert "IN '' [MDB_FILE;]" in queries["NullPathConnection"][5]
        assert "IN 'remote-token' []" in queries["EmptyConnection"][5]
        assert remotes["EmptyPath"] == ["", None]
        assert remotes["EmptyConnection"] == ["remote-token", ""]
        assert remotes["NullPathConnection"] == [None, "MDB_FILE;"]
        assert remotes["LiteralPunctuation"] == ["opaque ' é [x]; -- token", "TYPE]; ' marker"]
        people = [row[2] for row in back_rows if row[:2] == ["CELL", "People"]]
        root_people = [row[2] for row in front_rows if row[:2] == ["CELL", "People"]]
        orders = [row[2:] for row in back_rows if row[:2] == ["CELL", "Orders"]]
        prefix = [args.cli, "--format", "tsv"]
        run = prefix + ["query", "run", "--database", frontend, "--number-format", "invariant"]
        mapped = run + ["--external", "remote-token", backend, "--external", "second-token", frontend]

        for name, query in sorted(queries.items()):
            raw = [row[2:] for row in front_rows if row[0] == "RAW" and row[1] == query[2]]
            output = check(profile + ":inspect:" + name, prefix + ["inspect", frontend, "--query", name])
            inspected = list(csv.reader(StringIO(output), delimiter="\t", strict=True))
            assert inspected[:4] == [
                ["Query", name], ["Native ID", str(query[2])],
                ["Flags", f"0x{query[3]:08x}"], ["Fragment rows", str(len(raw))],
            ]
            assert Counter(map(tuple, inspected[5:])) == Counter(tuple(map(display, row)) for row in raw)
            fragments = []
            if name in remotes:
                path, connection = remotes[name]
                fragments.append(" IN '" + (path or "").replace("'", "''") + "'")
                if connection is not None:
                    fragments.append(" [" + connection.replace("]", "]]") + "]")
            check(profile + ":reconstruct:" + name,
                  prefix + ["query", "saved", "--database", frontend, name], contains=fragments)

        persons = [[str(person)] for person in people]
        check(profile + ":saved-path", mapped + ["--saved", "PathOnly"], [["Person"], *persons])
        check(profile + ":root-named", mapped + ["SELECT Person FROM PathOnly ORDER BY Person"],
              [["Person"], *sorted(persons)])
        check(profile + ":empty-token", run + ["--external", "", backend, "--saved", "EmptyPath"],
              [["Person"], *persons])
        check(profile + ":union", mapped + ["--saved", "UnionExternal"],
              [["Person"], *persons, *[[str(person)] for person in root_people]])
        check(profile + ":join-param", mapped + ["--saved", "JoinParameter", "--param", "Floor=long:2"],
              [["Person"], *[[str(person)] for _, person, _ in orders if person >= 2]])
        pivot = [[str(person), *[
            str(sum(1 for _, owner, value in orders if owner == person and value == quarter))
            if any(owner == person and value == quarter for _, owner, value in orders) else "NULL"
            for quarter in (1, 2)
        ]] for person in people]
        for name in ("CrossExternal", "CrossDynamic"):
            check(profile + ":" + name, mapped + ["--saved", name], [["Person", "1", "2"], *pivot])
        check(profile + ":root-domain", mapped + ["--saved", "RootDomain"],
              [["Person", "RootCount"], *[[str(person), str(len(root_people))] for person in people]])
        for name in ("WithConnection", "EmptyConnection", "NullPathConnection", "LiteralPunctuation"):
            check(profile + ":provider-refusal:" + name, mapped + ["--saved", name], refusal=True)
        check(profile + ":missing-map", run + ["--saved", "PathOnly"], refusal=True)
        check(profile + ":wrong-token", run + ["--external", "REMOTE-TOKEN", backend, "--saved", "PathOnly"],
              refusal=True)
        check(profile + ":duplicate-map", mapped + ["--external", "remote-token", backend, "--saved", "PathOnly"],
              refusal=True)
        check(profile + ":late-error", mapped + ["--saved", "LateError"], refusal=True)
        for option in ("--native-bytes", "--native-steps", "--execution-bytes", "--execution-rows",
                       "--execution-steps", "--output-bytes"):
            check(profile + ":positive-runtime-limit:" + option,
                  mapped + ["--saved", "PathOnly", option, "1"], refusal=True)
        check(profile + ":unmapped-inert-root", run + ["SELECT ID FROM People"],
              [["ID"], *[[str(person)] for person in root_people]])

    for name, before in hashes.items():
        assert digest(ROOT / name) == before, name
    report = {
        "scope": "Original inert system-fragment authoring, reopened getters and explicit host-mapped execution; "
                 "not native Microsoft Access query qualification.",
        "profiles": list(PROFILES), "controls": len(observations), "hashes": hashes,
        "observations": observations,
    }
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({"controls": len(observations), "hashes": len(hashes), "report": str(args.report)}))


if __name__ == "__main__":
    main()
