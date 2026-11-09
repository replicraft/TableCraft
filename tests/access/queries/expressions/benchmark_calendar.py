"""Compare built time/Jiff calendar adapters; never compile during measurement."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path, help="original time adapter harness")
    parser.add_argument("--before", required=True, type=Path, help="original Jiff adapter with direction workloads")
    parser.add_argument("--current", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=Path("target/benchmarks/calendar.json"))
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    binaries = {"time": args.baseline, "jiff-before": args.before, "jiff-current": args.current}
    report = {
        "schema": "tablecraft-calendar-benchmarks-v1", "complete": False,
        "platform": platform.system(), "architecture": platform.machine(),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "binary_sha256": {name: hashlib.sha256(path.read_bytes()).hexdigest()
                          for name, path in binaries.items()},
        "runs": 20, "warmup": 3, "cases": [],
    }
    for workload, iterations, labels in [
        ("calendar", 1_000_000, list(binaries)),
        ("calendar", 5_000_000, list(binaries)),
        ("calendar-decode", 5_000_000, ["jiff-before", "jiff-current"]),
        ("calendar-encode", 5_000_000, ["jiff-before", "jiff-current"]),
    ]:
        commands = [[str(binaries[label]), workload, "--iterations", str(iterations)] for label in labels]
        checksums = [subprocess.check_output(command, text=True).strip() for command in commands]
        if len(set(checksums)) != 1:
            raise RuntimeError(f"{workload}: conversion checksum disagreement")
        name = f"{workload}-{iterations}"
        raw = args.output.parent / f"{name}-hyperfine.json"
        invocation = ["hyperfine", "--shell=none", "--style=basic", "--runs", "20", "--warmup", "3",
                      "--export-json", str(raw)]
        for label, command in zip(labels, commands, strict=True):
            invocation.extend(["--command-name", label, shlex.join(command)])
        print(name, flush=True)
        subprocess.run(invocation, check=True)
        results = json.loads(raw.read_text())["results"]
        report["cases"].append({
            "name": name, "iterations": iterations, "checksum": checksums[0],
            "results": [{"label": label,
                         "summary": {k: v for k, v in result["summary"].items()
                                     if k in ["time_wall_clock", "memory_peak_resident"]},
                         "wall_seconds": [m["time_wall_clock"]["value"] for m in result["measurements"]]}
                        for label, result in zip(labels, results, strict=True)],
        })
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    report["complete"] = True
    args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
