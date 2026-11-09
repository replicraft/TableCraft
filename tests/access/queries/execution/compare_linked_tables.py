"""Original public-writer/reopened-getter linked metadata, without target opens."""
import argparse
import base64
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
PROFILES = ("V2000", "V2007", "V2016", "V2019")
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def relative(path):
    return path.resolve().relative_to(ROOT).as_posix()


def java(jar, source, *arguments):
    return subprocess.run(
        ["java", "-Dfile.encoding=UTF-8", "-Dstdout.encoding=UTF-8", "-cp", str(jar),
         str(ROOT / "docs/testing" / source), *map(str, arguments)],
        check=True, capture_output=True, text=True, encoding="utf-8",
    ).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--reader", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--result", type=Path, required=True)
    parser.add_argument("--generate", action="store_true")
    args = parser.parse_args()
    assert digest(args.jar) == JAR_SHA256
    assert not args.result.exists(), "Result must be fresh"
    reader_digest = digest(args.reader)
    inputs = []
    controls = 0
    for profile in PROFILES:
        directory = args.directory / profile
        if args.generate:
            java(args.jar, "LinkedTableFixture.java", directory, profile)
        suffix = ".mdb" if profile == "V2000" else ".accdb"
        frontend = directory / ("frontend" + suffix)
        backend = directory / ("backend" + suffix)
        originals = {path: digest(path) for path in (frontend, backend)}
        oracle = java(args.jar, "LinkedTableOracle.java", frontend)
        rows = [line.split("\t") for line in oracle.splitlines()]
        assert len(rows) == 4
        for expected in rows:
            name = base64.b64decode(expected[0][1:], validate=True).decode("utf-8")
            result = subprocess.run(
                [str(args.reader), str(frontend), name], check=True,
                capture_output=True, text=True, encoding="utf-8",
            )
            assert result.stdout.rstrip("\n").split("\t") == expected, name
            controls += 1
        assert originals == {path: digest(path) for path in originals}
        inputs.append({
            "profile": profile,
            "frontend": {"path": relative(frontend), "sha256": originals[frontend]},
            "backend": {"path": relative(backend), "sha256": originals[backend]},
            "definitions": len(rows),
            "getter_catalog_agreement": True,
            "target_resolution_trap_calls": 0,
            "originals_unchanged": True,
        })
    assert digest(args.reader) == reader_digest
    sources = [
        ROOT / "docs/testing/LinkedTableFixture.java",
        ROOT / "docs/testing/LinkedTableOracle.java",
        Path(__file__).resolve(),
        ROOT / "crates/engines/tablecraft-ace/examples/linked_table_data.rs",
        ROOT / "crates/engines/tablecraft-ace/src/catalog/linked/fields.rs",
        ROOT / "crates/engines/tablecraft-ace/src/catalog/linked/metadata.rs",
        ROOT / "crates/engines/tablecraft-ace/src/catalog/linked/read.rs",
    ]
    report = {
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "scope": "Inert linked catalog metadata, not linked query execution or native Access qualification",
        "controls_passed": controls,
        "reader": {"path": relative(args.reader), "sha256": reader_digest},
        "jackcess_sha256": JAR_SHA256,
        "lockfile_sha256": digest(ROOT / "Cargo.lock"),
        "sources": {relative(path): digest(path) for path in sources},
        "inputs": inputs,
    }
    args.result.parent.mkdir(parents=True, exist_ok=True)
    args.result.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"Passed {controls} original linked metadata getter controls across four profiles; no targets opened")


if __name__ == "__main__":
    main()
