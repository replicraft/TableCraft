"""Measure complete bounded decodes with an already-built release benchmark."""
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
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary",required=True,type=Path)
    parser.add_argument("--output",required=True,type=Path)
    parser.add_argument("--iterations",type=int,default=20_000)
    parser.add_argument("--runs",type=int,default=20)
    parser.add_argument("--warmup",type=int,default=3)
    args=parser.parse_args()
    if not 1<=args.iterations<=100_000 or not 2<=args.runs<=100 or not 0<=args.warmup<=20:
        parser.error("benchmark counts exceed the bounded runner profile")
    commands=[]
    samples={}
    for case in ["stored-repeated","zlib-repeated","stored-noise","zlib-noise"]:
        command=[str(args.binary),"--iterations",str(args.iterations),"--case",case]
        output=subprocess.check_output(command,text=True)
        samples[case]=output
        commands.append(shlex.join(command))
    with tempfile.TemporaryDirectory(prefix="tablecraft-attachment-bench-") as directory:
        raw=Path(directory)/"hyperfine.json"
        subprocess.run(["hyperfine","--runs",str(args.runs),"--warmup",str(args.warmup),"--export-json",str(raw),*commands],check=True)
        timings=json.loads(raw.read_text())
    source=Path("crates/engines/tablecraft-ace/src/table/attachment")
    sources={str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(source.glob("*.rs"))}
    bench=Path("crates/engines/tablecraft-ace/benches/attachments.rs")
    sources[str(bench)]=hashlib.sha256(bench.read_bytes()).hexdigest()
    packages=tomllib.loads(Path("Cargo.lock").read_text())["package"]
    report={"profile":"release; no target-cpu=native or LTO overrides", "platform":platform.system()+" "+platform.machine(),
        "rustc":subprocess.check_output(["rustc","--version"],text=True).strip(),
        "hyperfine_version":subprocess.check_output(["hyperfine","--version"],text=True).strip(),
        "iterations":args.iterations,"payload_bytes":60_000,"warmups":args.warmup,"runs":args.runs,
        "measurement":"Full process timings include setup; each timed loop constructs budgets/decoder, validates wrapper and drops output. No native Access comparison.",
        "binary_sha256":hashlib.sha256(args.binary.read_bytes()).hexdigest(),"source_sha256":sources,
        "dependencies":{p["name"]:p["version"] for p in packages if p["name"] in ["flate2","miniz_oxide","adler2","crc32fast","simd-adler32"]},
        "sample_inner_timings":samples,"hyperfine":timings}
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(report,indent=2)+"\n")
    for result in timings["results"]:
        case=shlex.split(result["command"])[-1]
        summary=result.get("summary",{}).get("time_wall_clock",result)
        print(case,f"{summary['mean']*1_000_000/args.iterations:.3f} us/payload",f"SD {summary['stddev']*1_000_000/args.iterations:.3f} us")


if __name__=="__main__":main()
