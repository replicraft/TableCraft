"""Compare native readers before/after linear usage-map ordering checks."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["baseline", "current", "ace", "jet"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--output", default=Path("target/benchmarks/usage-maps.json"), type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    binaries = [("baseline", args.baseline), ("current", args.current)]
    report = {
        "schema": "tablecraft-usage-map-benchmarks-v1", "complete": False,
        "platform": platform.system(), "architecture": platform.machine(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "binary_sha256": {label: hashlib.sha256(path.read_bytes()).hexdigest() for label, path in binaries},
        "baseline_implementation": "e083f6e", "runs": 20, "warmup": 3, "iterations": 10, "cases": [],
    }
    for format, path in [("ace", args.ace), ("jet4", args.jet)]:
        for mode in ["sequential", "indexed"]:
            commands = [[str(binary), mode, "--iterations", "10", "--database", str(path),
                         "--table", "Pages", "--index", "PrimaryKey"] for _, binary in binaries]
            outputs = [subprocess.check_output(command, text=True).strip() for command in commands]
            if len(set(outputs)) != 1 or outputs[0] != "20000":
                raise RuntimeError("usage-map reader checksum disagreement")
            name = format + "-" + mode
            raw = args.output.parent / f"usage-{name}-hyperfine.json"
            run = ["hyperfine", "--shell=none", "--style=basic", "--runs", "20", "--warmup", "3", "--export-json", str(raw)]
            for (label, _), command in zip(binaries, commands, strict=True):
                run.extend(["--command-name", label, shlex.join(command)])
            print(name, flush=True)
            subprocess.run(run, check=True)
            results = json.loads(raw.read_text())["results"]
            report["cases"].append({
                "name": name, "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "checksum": outputs[0],
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
