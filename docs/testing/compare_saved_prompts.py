"""Independent literal-prompt rendering, raw getters and explicit row/refusal controls."""
import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import RECIPES
from compare_saved_forests import PROFILES


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("jar", "cli", "directory"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
    prompts = {
        "PromptDot": "Enter No. of items", "PromptDotQuoted": "Enter No. of items",
        "PromptShortDot": "P.Q", "PromptBang": "Floor!", "PromptBangQuoted": "Ask!Now",
        "PromptBangSpaces": "Ask! now", "PromptGrave": "Rate `value`",
        "PromptEquals": "Floor>= minimum", "PromptLooksLikeCode": "NeverRunMe()",
        "PromptNested": "Floor.", "PromptUnion": "P.Q", "PromptCrosstab": "Floor.",
    }
    controls = 0
    with tempfile.TemporaryDirectory(prefix="tablecraft-prompts-oracle-") as classes:
        sources = [RECIPES / (name + ".java") for name in
                   ("SavedForestFixture", "SavedPromptFixture", "SavedReadOracle", "SavedQueryOracle")]
        subprocess.run(["javac", "-cp", str(args.jar), "-d", classes, *map(str, sources)], check=True)

        def java(name, *values):
            return subprocess.check_output([
                "java", "-Dstdout.encoding=UTF-8", "-cp",
                str(args.jar) + os.pathsep + classes, name, *map(str, values)
            ], text=True, encoding="utf-8")

        def cli(*values, accepted=True):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True)
            if accepted:
                assert result.returncode == 0, (values, result.stderr)
            else:
                assert result.returncode == 2 and not result.stdout, (values, result)
            return result.stdout

        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java("SavedPromptFixture", args.directory / ("prompt-" + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ("prompt-" + suffix)
            before = path.read_bytes()
            names = java("SavedReadOracle", path).splitlines()
            assert len(names) == 27
            names = [name for name in names if name.startswith("Prompt")]
            assert set(names) == set(prompts) | {"PromptControl"}
            for name in names:
                raw = java("SavedQueryOracle", path, name)
                report = cli("inspect", path, "--query", name)
                assert list(csv.reader(StringIO(report), delimiter="\t")) == [line.split("\t") for line in raw.splitlines()]
                controls += 1
                sql = java("SavedReadOracle", path, name)
                if name == "PromptControl":
                    assert "Forms!F!C" in sql
                    controls += 1
                    cli("query", "saved", "--database", path, name, accepted=False)
                else:
                    assert cli("query", "saved", "--database", path, name) == cli("query", "parse", sql)
                controls += 1
                for query in (sql, None, "SELECT * FROM [" + name + "]"):
                    command = ["query", "run"]
                    command += ["--saved", name] if query is None else [query]
                    command += ["--database", path]
                    if name in prompts:
                        command += ["--param", "[" + prompts[name] + "]=long:2"]
                    accepted = name != "PromptControl" and (name != "PromptCrosstab" or query is None or query == sql)
                    output = cli(*command, accepted=accepted)
                    if accepted:
                        wanted = [["KeyA"], ["2"], ["4"]]
                        if name == "PromptUnion":
                            wanted = [["KeyA"], ["2"], ["2"], ["4"]]
                        elif name == "PromptCrosstab":
                            wanted = [["Bucket", "Total", "1", "2", "4"], ["0", "2", "NULL", "1", "1"]]
                        assert list(csv.reader(StringIO(output), delimiter="\t")) == wanted, (name, output, wanted)
                    controls += 1
            assert path.read_bytes() == before
            print(path.name, len(before), hashlib.sha256(before).hexdigest())
    print(f"Passed {controls} literal-prompt getter/rendering/execution/refusal controls; native Access unrun.")


if __name__ == "__main__":
    main()
