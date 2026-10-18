"""Compare built native API workloads/kernels with Hyperfine; never build during timing."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--current", required=True, type=Path)
    parser.add_argument("--bitmaps", required=True, type=Path)
    parser.add_argument("--database", required=True, type=Path)
    parser.add_argument("--generated-ace", type=Path)
    parser.add_argument("--generated-jet", type=Path)
    parser.add_argument("--output", type=Path, default=Path("target/benchmarks/native-libraries.json"))
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--warmup", type=int, default=3)
    args = parser.parse_args()
    if args.runs < 2 or args.warmup < 0:
        parser.error("use at least two runs and nonnegative warmup")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for density in ["zero", "sparse", "mixed", "dense"]:
        outputs = [subprocess.check_output([str(args.bitmaps), kernel, "--density", density,
                    "--iterations", "1"], text=True).strip() for kernel in ["scalar", "words", "simd"]]
        if len(set(outputs)) != 1:
            raise RuntimeError(f"{density}: kernel preflight checksum disagreement")
    report = {
        "schema": "tablecraft-native-benchmarks-v1",
        "complete": False,
        "platform": platform.system(), "architecture": platform.machine(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "runs": args.runs, "warmup": args.warmup,
        "binary_sha256": {name: digest(path) for name, path in [
            ("baseline", args.baseline), ("current", args.current), ("bitmaps", args.bitmaps)]},
        "input_sha256": {"registered_candidate": digest(args.database)},
        "cases": [],
    }

    def compare(name, commands, iterations):
        checksums = [subprocess.check_output(command, text=True).strip() for _, command in commands]
        if len(set(checksums)) != 1:
            raise RuntimeError(f"{name}: benchmark checksum disagreement")
        raw = args.output.parent / f"{name}-hyperfine.json"
        command = ["hyperfine", "--shell=none", "--style=basic", "--runs", str(args.runs),
                   "--warmup", str(args.warmup), "--export-json", str(raw)]
        for label, invocation in commands:
            command.extend(["--command-name", label, shlex.join([str(part) for part in invocation])])
        print(name, flush=True)
        subprocess.run(command, check=True)
        measurements = json.loads(raw.read_text())
        case = {"name": name, "iterations": iterations, "checksum": checksums[0], "results": []}
        # Keep samples/statistics, omit executable/input paths and host-specific metadata.
        for (label, _), result in zip(commands, measurements["results"], strict=True):
            case["results"].append({
                "label": label,
                "summary": result["summary"], "measurements": result["measurements"],
            })
        report["cases"].append(case)
        args.output.write_text(json.dumps(report, indent=2) + "\n")

    workloads = [
        ("header", 5_000_000, ["--database", args.database]),
        ("calendar", 1_000_000, []),
        ("catalog", 500, ["--database", args.database]),
        ("sequential", 100, ["--database", args.database, "--table", "TAG_NME_TB"]),
        ("indexed", 500, ["--database", args.database]),
    ]
    for workload, iterations, extra in workloads:
        compare(workload, [(label, [str(binary), workload, "--iterations", str(iterations), *map(str, extra)])
                          for label, binary in [("baseline", args.baseline), ("current", args.current)]], iterations)
    for name, path in [("ace", args.generated_ace), ("jet", args.generated_jet)]:
        if path is not None:
            report["input_sha256"][name] = digest(path)
            compare("indexed-" + name, [(label, [str(binary), "indexed", "--iterations", "100",
                        "--database", str(path), "--table", "Indexed", "--index", "PrimaryKey"])
                    for label, binary in [("baseline", args.baseline), ("current", args.current)]], 100)
    for density in ["zero", "sparse", "mixed", "dense"]:
        iterations = 20_000
        compare("bitmap-" + density, [(kernel, [str(args.bitmaps), kernel, "--density", density,
                "--iterations", str(iterations)]) for kernel in ["scalar", "words", "simd"]], iterations)
    report["complete"] = True
    args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
