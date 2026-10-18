"""Compare query execution with native source constraints accepted or declined."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--ace", required=True, type=Path)
    parser.add_argument("--jet", required=True, type=Path)
    parser.add_argument("--output", default=Path("target/benchmarks/query-pushdown.json"), type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    report = {
        "schema": "tablecraft-query-pushdown-benchmarks-v1", "complete": False,
        "platform": platform.system(), "architecture": platform.machine(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "runs": 20, "warmup": 3, "iterations": 500, "cases": [],
    }
    for format, path in [("ace", args.ace), ("jet4", args.jet)]:
        for workload in ["range", "equality", "null"]:
            commands = [[str(args.binary), mode, "--iterations", "500", "--database", str(path),
                         "--workload", workload] for mode in ["full", "seek"]]
            outputs = [subprocess.check_output(command, text=True).strip() for command in commands]
            if len(set(outputs)) != 1:
                raise RuntimeError("full/seek query checksum disagreement")
            name = f"{format}-{workload}"
            raw = args.output.parent / f"query-{name}-hyperfine.json"
            run = ["hyperfine", "--shell=none", "--style=basic", "--runs", "20", "--warmup", "3",
                   "--export-json", str(raw)]
            for label, command in zip(["full", "seek"], commands, strict=True):
                run.extend(["--command-name", label, shlex.join(command)])
            print(name, flush=True)
            subprocess.run(run, check=True)
            results = json.loads(raw.read_text())["results"]
            report["cases"].append({
                "name": name, "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "checksum_rows_id_sum_nulls": outputs[0],
                "results": [{"label": label, "summary": {k: v for k, v in result["summary"].items()
                             if k in ["time_wall_clock", "memory_peak_resident"]},
                             "wall_seconds": [m["time_wall_clock"]["value"] for m in result["measurements"]]}
                            for label, result in zip(["full", "seek"], results, strict=True)],
            })
            args.output.write_text(json.dumps(report, indent=2) + "\n")
    report["complete"] = True
    args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
