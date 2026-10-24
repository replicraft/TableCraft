#!/usr/bin/env python3
"""Original byte-search correctness probes and fixed-binary timing capture."""

import argparse
import hashlib
import json
import platform
import shlex
import subprocess
import tempfile
from pathlib import Path


SOURCES = [
    "crates/query/tablecraft-expressions/examples/byte_search.rs",
    "crates/query/tablecraft-expressions/examples/byte_search/algorithm.rs",
    "crates/query/tablecraft-expressions/examples/byte_search/tests.rs",
    "docs/testing/benchmark_byte_search.py",
]
CASES = [
    ("front", 32, 1_000_000),
    ("absent", 1, 1000),
    ("absent", 32, 100),
    ("late", 32, 100),
    ("random", 32, 1000),
    ("raw", 3, 1000),
    ("absent", 33, 100),
]
ALGORITHMS = ["scalar", "bounded", "reference"]
INPUT_BYTES = 65_536


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inputs(scenario, length):
    haystack = bytearray(b"a" * INPUT_BYTES)
    needle = bytearray(b"a" * length)
    needle[-1] = ord("b")
    if scenario == "front":
        haystack[:length] = needle
    elif scenario == "late":
        haystack[-length:] = needle
    elif scenario == "random":
        state = 0x499238152817AE61
        mask = (1 << 64) - 1
        for position in range(len(haystack)):
            state = (state ^ (state << 13)) & mask
            state ^= state >> 7
            state = (state ^ (state << 17)) & mask
            haystack[position] = state & 255
        start = (len(haystack) - length) // 2
        needle[:] = haystack[start:start + length]
    elif scenario == "raw":
        pattern = bytes([0, 0xD8, 0x61, 0, 0xFF, 0x3E, 0xD8])
        haystack[:] = (pattern * (len(haystack) // len(pattern) + 1))[:len(haystack)]
        needle[:] = b"\xff" * length
        start = (len(haystack) - length) // 2
        haystack[start:start + length] = needle
    return bytes(haystack), bytes(needle)


def command(binary, algorithm, scenario, length, iterations):
    return [str(binary), "--algorithm", algorithm, "--scenario", scenario,
            "--bytes", str(INPUT_BYTES), "--needle-bytes", str(length),
            "--iterations", str(iterations)]


def probe(binary, algorithm, scenario, length, expected):
    output = subprocess.check_output(
        command(binary, algorithm, scenario, length, 1), text=True,
    ).strip().split("\t")
    if len(output) != 9:
        raise RuntimeError("invalid probe output")
    found = None if output[5] == "None" else int(output[5][5:-1])
    if found != expected or int(output[8]) != (0 if expected is None else expected + 1):
        raise RuntimeError("independent byte-position mismatch")
    return {"work": int(output[6]), "checkpoints": int(output[7])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path,
                        default=Path("target/release/examples/byte_search"))
    parser.add_argument("--output", type=Path)
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--runs", type=int, default=15)
    parser.add_argument("--warmup", type=int, default=3)
    args = parser.parse_args()
    if args.binary.is_absolute() or ".." in args.binary.parts:
        parser.error("binary must use a repository-relative path")
    if not 1 <= args.runs <= 100 or not 0 <= args.warmup <= 20:
        parser.error("invalid run/warmup count")
    if not args.check_only and args.output is None:
        parser.error("--output is required for timing capture")
    metadata_path = None
    if args.output is not None:
        metadata_path = args.output.with_suffix(".metadata.json")
        if args.output.exists() or metadata_path.exists():
            parser.error("output paths must be fresh")

    binary_hash = digest(args.binary)
    source_hashes = {path: digest(path) for path in SOURCES}
    cases = []
    commands = []
    for scenario, length, iterations in CASES:
        haystack, needle = inputs(scenario, length)
        offset = haystack.find(needle)
        expected = None if offset == -1 else offset
        probes = {
            algorithm: probe(args.binary, algorithm, scenario, length, expected)
            for algorithm in ALGORITHMS
        }
        cases.append({
            "scenario": scenario, "bytes": INPUT_BYTES, "needle_bytes": length,
            "iterations": iterations, "first_offset": expected,
            "haystack_sha256": hashlib.sha256(haystack).hexdigest(),
            "needle_sha256": hashlib.sha256(needle).hexdigest(), "probes": probes,
        })
        commands.extend(
            shlex.join(command(args.binary, algorithm, scenario, length, iterations))
            for algorithm in ALGORITHMS
        )
    if args.check_only:
        print(json.dumps({"independent_probes": len(cases) * len(ALGORITHMS),
                          "binary_sha256": binary_hash}, sort_keys=True))
        return

    Path(".cache").mkdir(exist_ok=True)
    with tempfile.NamedTemporaryFile(prefix="byte-search-", suffix=".json",
                                     dir=".cache", delete=False) as capture:
        raw_path = Path(capture.name)
    subprocess.run([
        "hyperfine", "--warmup", str(args.warmup), "--runs", str(args.runs),
        "--export-json", str(raw_path), *commands,
    ], check=True)
    if digest(args.binary) != binary_hash or any(
        digest(path) != expected for path, expected in source_hashes.items()
    ):
        raise RuntimeError("binary or experiment sources changed during timing; raw capture retained")
    raw = raw_path.read_bytes()
    if b"/Users/" in raw or b"/private/" in raw:
        raise RuntimeError("private path found; original raw capture retained under .cache")
    metadata = {
        "binary": str(args.binary), "binary_sha256": binary_hash,
        "source_sha256": source_hashes, "system": platform.system(),
        "architecture": platform.machine(), "python": platform.python_version(),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "profile": "release, normal incremental settings, default features",
        "runs": args.runs, "warmup": args.warmup, "cases": cases,
        "policy": "benchmark-only; bounded units differ; whole reference is not production-safe",
        "native_access": "unrun",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("xb") as output:
        output.write(raw)
    with metadata_path.open("x") as output:
        json.dump(metadata, output, indent=2, sort_keys=True)
        output.write("\n")
    print(json.dumps({"independent_probes": 21, "timed_commands": len(commands),
                      "output": str(args.output)}, sort_keys=True))


if __name__ == "__main__":
    main()
