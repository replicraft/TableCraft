"""Record warm-cache CLI equality timings after verifying immutable answers/inputs."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess

from compare_large_tables import digest
from compare_null_indexes import INPUT_HASHES
from compare_text_equality_queries import POOLS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["cli", "directory", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    metadata = args.output.with_suffix(".metadata.json")
    if args.output.exists() or metadata.exists():
        raise ValueError("Benchmark artifacts must be fresh; retain earlier runs")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    executable = digest(args.cli)
    inputs = {}
    commands = []
    answers = {}
    sql = "SELECT ID FROM TextIndexed WHERE Label='Straße'"
    for profile in ["V2000", "V2007", "V2016", "V2019"]:
        extension = "mdb" if profile == "V2000" else "accdb"
        for version in [0, 1]:
            label = f"Text-{profile}-{version}"
            path = args.directory / f"{label}.{extension}"
            assert digest(path) == INPUT_HASHES[label]
            inputs[label] = {"path": str(path), "sha256": INPUT_HASHES[label]}
            previous = None
            for mode in ["sequential", "automatic"]:
                command = [str(args.cli), "query", "run", sql, "--database", str(path), *POOLS]
                if mode == "automatic":
                    command.append("--auto-index")
                answer = subprocess.check_output(command)
                assert len(answer.splitlines()) == 95
                if previous is not None:
                    assert answer == previous
                previous = answer
                answers[label] = hashlib.sha256(answer).hexdigest()
                commands.extend(["--command-name", f"{profile}-g{version}-{mode}", shlex.join(command)])
    subprocess.run([
        "hyperfine", "--warmup", "3", "--runs", "15", "--export-json", str(args.output), *commands,
    ], check=True)
    assert digest(args.cli) == executable, "Executable changed during measurement"
    for value in inputs.values():
        assert digest(Path(value["path"])) == value["sha256"]
    metadata.write_text(json.dumps({
        "binary": str(args.cli), "binary_sha256": executable,
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "platform": platform.system(), "architecture": platform.machine(),
        "build": "debug default features; normal incremental compilation",
        "rows": 1000, "matched_rows": 94, "warmups": 3, "runs": 15,
        "inputs": inputs, "answer_sha256": answers,
        "scope": "CLI warm-cache General equality; independent correctness checked separately; native Access unrun",
    }, indent=2) + "\n")


if __name__ == "__main__":
    main()
