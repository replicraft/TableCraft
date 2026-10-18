"""Measure chained definitions and check ordinary-reader regressions with Hyperfine."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["baseline", "current", "database", "ace", "jet"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--output", default=Path("target/benchmarks/definitions.json"), type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "schema": "tablecraft-definition-benchmarks-v1", "complete": False,
        "platform": platform.system(), "architecture": platform.machine(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "binary_sha256": {label: hashlib.sha256(path.read_bytes()).hexdigest()
                          for label, path in [("baseline", args.baseline), ("current", args.current)]},
        "baseline_implementation": "f761d69", "runs": 20, "warmup": 3, "cases": [],
    }
    cases = [(name, args.database, options, True) for name, options in [
        ("catalog", ["catalog", "--iterations", "500"]),
        ("sequential", ["sequential", "--iterations", "100", "--table", "TAG_NME_TB"]),
        ("indexed", ["indexed", "--iterations", "500"]),
    ]]
    for format, path in [("ace", args.ace), ("jet4", args.jet)]:
        cases.extend([
            (format + "-schema", path, ["schema", "--iterations", "1000", "--table", "Wide"], False),
            (format + "-indexed", path, ["indexed", "--iterations", "500", "--table", "Wide", "--index", "First"], False),
        ])
    for name, path, options, paired in cases:
        binaries = [("baseline", args.baseline), ("current", args.current)] if paired else [("current", args.current)]
        commands = [[str(binary), *options, "--database", str(path)] for _, binary in binaries]
        outputs = [subprocess.check_output(command, text=True).strip() for command in commands]
        if len(set(outputs)) != 1:
            raise RuntimeError("baseline/current checksum disagreement")
        raw = args.output.parent / f"definition-{name}-hyperfine.json"
        run = ["hyperfine", "--shell=none", "--style=basic", "--runs", "20", "--warmup", "3", "--export-json", str(raw)]
        for (label, _), command in zip(binaries, commands, strict=True):
            run.extend(["--command-name", label, shlex.join(command)])
        print(name, flush=True)
        subprocess.run(run, check=True)
        results = json.loads(raw.read_text())["results"]
        report["cases"].append({
            "name": name, "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "checksum": outputs[0], "options": options,
            "results": [{"label": label, "summary": {k: v for k, v in result["summary"].items()
                         if k in ["time_wall_clock", "memory_peak_resident"]},
                         "wall_seconds": [m["time_wall_clock"]["value"] for m in result["measurements"]]}
                        for (label, _), result in zip(binaries, results, strict=True)],
        })
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    report["complete"] = True
    args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
