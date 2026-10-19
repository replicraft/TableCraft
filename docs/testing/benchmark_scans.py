"""Compare built native scan executables with Hyperfine, retaining every sample."""
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
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--baseline-metadata", type=Path, required=True)
    parser.add_argument("--current", type=Path, required=True)
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--generated-ace", type=Path, required=True)
    parser.add_argument("--composite-ace", type=Path)
    parser.add_argument("--guid-ace", type=Path)
    parser.add_argument("--guid-composite-ace", type=Path)
    parser.add_argument("--boolean-ace", type=Path)
    parser.add_argument("--floating-ace", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=20)
    parser.add_argument("--warmup", type=int, default=3)
    args = parser.parse_args()
    if args.runs < 2 or args.warmup < 0:
        parser.error("use at least two runs and nonnegative warmup")
    root = Path(__file__).resolve().parents[2]
    sources = [
        "crates/engines/tablecraft-ace/benches/native.rs",
        "crates/engines/tablecraft-ace/src/format/reader.rs",
        "crates/engines/tablecraft-ace/src/catalog/inventory.rs",
        "crates/engines/tablecraft-ace/src/table/snapshot.rs",
    ]
    sources += sorted(path.relative_to(root).as_posix()
        for path in (root / "crates/engines/tablecraft-ace/src/table/indexes").rglob("*.rs"))
    sources.append("docs/testing/benchmark_scans.py")
    baseline_metadata = json.loads(args.baseline_metadata.read_text())
    lock_hash = digest(root / "Cargo.lock")
    rustc = subprocess.check_output(["rustc", "-V"], text=True).strip()
    if baseline_metadata["binary_sha256"] != digest(args.baseline):
        raise RuntimeError("baseline executable differs from its recorded build")
    if (baseline_metadata["cargo_lock_sha256"] != lock_hash
            or baseline_metadata["rustc"] != rustc):
        raise RuntimeError("baseline toolchain/lockfile differs from current")
    report = {
        "schema": "tablecraft-scan-benchmarks-v1", "complete": False,
        "platform": platform.system(), "architecture": platform.machine(),
        "rustc": rustc,
        "hyperfine": subprocess.check_output(["hyperfine", "--version"], text=True).strip(),
        "baseline_build": baseline_metadata,
        "current_parent_commit": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
        "current_build": "CARGO_INCREMENTAL=0 cargo build -p tablecraft-ace --bench native --release --locked",
        "current_source_sha256": {name: digest(root / name) for name in sources},
        "current_binary_sha256": digest(args.current),
        "cargo_lock_sha256": lock_hash,
        "input_sha256": {"registered_candidate": digest(args.database),
                         "generated_ace": digest(args.generated_ace)},
        "runs": args.runs, "warmup": args.warmup,
        "measurement": "Full processes load the database once, then repeat public APIs over a memory-backed Cursor. Includes setup, catalog/schema discovery and output drops; not disk I/O or a Microsoft Access comparison. No samples removed.",
        "cases": [],
    }
    workloads = [
        ("header-control", "header", 1_000_000, "registered_candidate", []),
        ("catalog", "catalog", 500, "registered_candidate", []),
        ("sequential", "sequential", 500, "registered_candidate", ["--table", "TAG_NME_TB"]),
        ("indexed", "indexed", 500, "registered_candidate", []),
        ("sequential-ace", "sequential", 100, "generated_ace", ["--table", "Indexed"]),
        ("indexed-ace", "indexed", 100, "generated_ace", ["--table", "Indexed", "--index", "PrimaryKey"]),
    ]
    inputs = {"registered_candidate": args.database, "generated_ace": args.generated_ace}
    if args.composite_ace is not None:
        inputs["composite_ace"] = args.composite_ace
        report["input_sha256"]["composite_ace"] = digest(args.composite_ace)
        workloads += [("numeric-five-control", "indexed", 100, "composite_ace",
                       ["--table", "MixedIndexed", "--index", "FiveMixed"]),
                      ("numeric-ten-control", "indexed", 100, "composite_ace",
                       ["--table", "TenIndexed", "--index", "TenMixed"])]
    if args.guid_ace is not None:
        inputs["guid_ace"] = args.guid_ace
        report["input_sha256"]["guid_ace"] = digest(args.guid_ace)
        workloads.append(("single-guid", "indexed", 100, "guid_ace",
            ["--table", "GuidIndexed", "--index", "GuidAsc"]))
    if args.guid_composite_ace is not None:
        inputs["guid_composite_ace"] = args.guid_composite_ace
        report["input_sha256"]["guid_composite_ace"] = digest(args.guid_composite_ace)
    for label, path in (("boolean_ace", args.boolean_ace), ("floating_ace", args.floating_ace)):
        if path is not None:
            inputs[label] = path
            report["input_sha256"][label] = digest(path)
    args.output.parent.mkdir(parents=True, exist_ok=True)

    def invocation(binary, workload, iterations, input_name, extra):
        return [str(binary), workload, "--iterations", str(iterations),
                "--database", str(inputs[input_name]), *extra]

    def compare(name, commands, iterations, details):
        checksums = [subprocess.check_output(command, text=True).strip()
                     for _, command in commands]
        if len(set(checksums)) != 1:
            raise RuntimeError(f"{name}: preflight checksums differ")
        raw = args.output.parent / f"{args.output.stem}-{name}-raw.json"
        invocation = ["hyperfine", "--shell=none", "--style=basic", "--runs", str(args.runs),
                      "--warmup", str(args.warmup), "--export-json", str(raw)]
        for label, command in commands:
            invocation.extend(["--command-name", label, shlex.join(command)])
        print(name, flush=True)
        subprocess.run(invocation, check=True)
        measurements = json.loads(raw.read_text())
        case = {
            "name": name, "iterations": iterations, **details, "checksum": checksums[0],
            "results": [{"label": label, "summary": result["summary"],
                         "measurements": result["measurements"]}
                        for (label, _), result in zip(commands, measurements["results"], strict=True)],
        }
        report["cases"].append(case)
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    for name, workload, iterations, input_name, extra in workloads:
        compare(name, [(label, invocation(binary, workload, iterations, input_name, extra))
            for label, binary in [("baseline", args.baseline), ("current", args.current)]],
            iterations, {"workload": workload, "input": input_name, "arguments": extra})
    compound_cases = []
    if args.composite_ace is not None:
        compound_cases += [("five-component", "MixedIndexed", "FiveMixed", "composite_ace"),
                           ("ten-component", "TenIndexed", "TenMixed", "composite_ace")]
    if args.guid_composite_ace is not None:
        compound_cases += [("guid-five-component", "GuidMixedIndexed", "MixedDirections", "guid_composite_ace"),
                           ("guid-ten-component", "GuidTenIndexed", "TenMixed", "guid_composite_ace")]
    if args.boolean_ace is not None:
        compound_cases.append(("boolean-single", "BooleanIndexed", "BoolAsc", "boolean_ace"))
    if args.floating_ace is not None:
        compound_cases += [("single-ieee", "SingleIndexed", "FloatAsc", "floating_ace"),
                           ("double-ieee", "DoubleIndexed", "FloatAsc", "floating_ace"),
                           ("floating-mixed", "FloatingMixed", "MixedDirections", "floating_ace")]
    for name, table, index, input_name in compound_cases:
        iterations = 100
        commands = [("sequential-current", invocation(args.current, "sequential", iterations,
                     input_name, ["--table", table])),
                    ("indexed-current", invocation(args.current, "indexed", iterations,
                     input_name, ["--table", table, "--index", index]))]
        compare(name, commands, iterations, {"input": input_name, "table": table,
            "index": index, "comparison": "sequential versus indexed full read on the same current binary; no before implementation"})
    report["complete"] = True
    args.output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
