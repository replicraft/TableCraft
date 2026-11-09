"""Compare complete fresh/reused attachment decoding with matched cumulative pools."""

import argparse
import hashlib
import json
from pathlib import Path
import platform
import shlex
import subprocess
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    commands = []
    samples = {}
    for size, iterations in [(3, 20_000), (60_000, 1000)]:
        for case in ["stored-repeated", "zlib-repeated", "zlib-noise"]:
            for decoder in ["fresh", "reused"]:
                command = [str(args.binary), "--iterations", str(iterations),
                           "--file-bytes", str(size), "--case", case, "--decoder", decoder]
                key = f"{case}/{size}/{decoder}"
                samples[key] = subprocess.check_output(command, text=True)
                commands.append(shlex.join(command))

    with tempfile.TemporaryDirectory(prefix="tablecraft-decoder-bench-") as directory:
        raw = Path(directory) / "hyperfine.json"
        subprocess.run(["hyperfine", "--runs", "10", "--warmup", "2", "--export-json", str(raw), *commands], check=True)
        timings = json.loads(raw.read_text())
    for result in timings["results"]:
        words = shlex.split(result["command"])
        words[0] = "attachments-bench"
        result["command"] = shlex.join(words)

    source = Path("crates/engines/tablecraft-ace/src/table/attachment")
    sources = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(source.glob("*.rs"))}
    bench = Path("crates/engines/tablecraft-ace/benches/attachments.rs")
    sources[str(bench)] = hashlib.sha256(bench.read_bytes()).hexdigest()
    packages = tomllib.loads(Path("Cargo.lock").read_text())["package"]
    report = {
        "profile": "release; no target-cpu=native or LTO overrides",
        "platform": platform.system() + " " + platform.machine(),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "hyperfine_version": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "runs": 10,
        "warmups": 2,
        "measurement": "Complete process includes setup, one matched cumulative pool, wrapper validation and output destruction; reused mode also drops its session before the timer ends.",
        "qualification": "Authored repeated/noise inputs at 3/60000 bytes; no native Access or broad throughput claim.",
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "source_sha256": sources,
        "dependencies": {package["name"]: package["version"] for package in packages
                         if package["name"] in ["flate2", "miniz_oxide", "adler2", "crc32fast", "simd-adler32"]},
        "sample_inner_timings": samples,
        "hyperfine": timings,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    for result in timings["results"]:
        command = shlex.split(result["command"])
        iterations = int(command[command.index("--iterations") + 1])
        summary = result.get("summary", {}).get("time_wall_clock", result)
        print(result["command"], f"{summary['mean'] * 1_000_000 / iterations:.3f} us/payload")


if __name__ == "__main__":
    main()
