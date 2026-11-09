"""Measure complete sequential/qualified-index checks, with explicit coverage limits."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["binary", "database", "ace", "jet"]:
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--output", default=Path("target/benchmarks/checks.json"), type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "schema": "tablecraft-table-check-benchmarks-v1", "complete": False,
        "platform": platform.system(), "architecture": platform.machine(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "runs": 20, "warmup": 3, "iterations": 100, "cases": [],
    }
    for name, path, table, rows in [("candidate", args.database, "ROULETTE_TB", 38),
                                    ("ace", args.ace, "Indexed", 1000), ("jet4", args.jet, "Indexed", 1000)]:
        command = [str(args.binary), "check", "--iterations", "100", "--database", str(path), "--table", table]
        checksum = subprocess.check_output(command, text=True).strip()
        if checksum != str(rows * 100):
            raise RuntimeError("table-check row checksum disagreement")
        raw = args.output.parent / f"check-{name}-hyperfine.json"
        print(name, flush=True)
        subprocess.run(["hyperfine", "--shell=none", "--style=basic", "--runs", "20", "--warmup", "3",
                        "--export-json", str(raw), "--command-name", name, shlex.join(command)], check=True)
        result = json.loads(raw.read_text())["results"][0]
        report["cases"].append({
            "name": name, "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "checksum": checksum,
            "summary": {k: v for k, v in result["summary"].items()
                        if k in ["time_wall_clock", "memory_peak_resident"]},
            "wall_seconds": [m["time_wall_clock"]["value"] for m in result["measurements"]],
        })
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    report["complete"] = True
    args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
