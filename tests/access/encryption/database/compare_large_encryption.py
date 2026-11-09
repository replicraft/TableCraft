"""Check page-buffered encryption on unchanged, original large ACE 16 inputs.

Only fresh temporary copies are rewritten. Independent public getters, decoded
SID hashes, page digests and independent removal supply engineering evidence;
no Microsoft Access execution or application save qualification is inferred.
"""

import argparse
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile

from build_large_deployment import digest
from compare_large_deployment import MEMO_UNITS, ROWS, memo
from encryption_references import rewrite_classpath

ROOT = Path(__file__).resolve().parents[2]
RECIPES = Path(__file__).resolve().parent
INPUTS = {
    "V2016.accdb": "51b4a6bf4477b2c1dca061e95179b5ccd70de158623edeafc96c5e38186bf630",
    "V2019.accdb": "cbb12a433c622b6d2a91e72d627b1cfab0eb6192953eaca7510c8d97668cc4f7",
}
PUBLIC_PASSWORD = "public synthetic password"
REPLACEMENT = "replacement public password"
SOURCE_PATHS = [
    "apps/tablecraft-cli/src/commands/encryption/copying.rs",
    "apps/tablecraft-cli/src/commands/encryption/mod.rs",
    "crates/engines/tablecraft-ace/src/encryption/mod.rs",
    "crates/engines/tablecraft-ace/src/encryption/add.rs",
    "crates/engines/tablecraft-ace/src/encryption/rewrite.rs",
    "crates/engines/tablecraft-ace/src/encryption/streaming.rs",
    "crates/engines/tablecraft-ace/src/encryption/native_header.rs",
    "crates/engines/tablecraft-ace/src/encryption/legacy/mod.rs",
    "crates/engines/tablecraft-ace/src/encryption/legacy/crypto.rs",
    "crates/engines/tablecraft-ace/src/encryption/legacy/descriptor.rs",
    "crates/engines/tablecraft-ace/src/encryption/agile/create.rs",
    "crates/engines/tablecraft-ace/src/encryption/agile/crypto.rs",
    "crates/engines/tablecraft-ace/src/encryption/agile/rewrite.rs",
    "crates/engines/tablecraft-ace/src/table/security_identifiers.rs",
]
DRIVERS = [
    "LargeEncryptedReadOracle.java",
    "NativeSecurityOracle.java",
    "EncryptedPagesOracle.java",
    "RemoveAddedEncryptionOracle.java",
]


def model():
    expected = ["Columns\tId\tScore\tLabel\tMemo"]
    memo_hashes = {}
    all_memos = hashlib.sha256()
    for identity in range(1, ROWS + 1):
        value = memo(identity)
        value_hash = hashlib.sha256(value).hexdigest()
        memo_hashes[identity] = value_hash
        all_memos.update(value)
        expected.append(
            f"Row\t{identity}\t{identity * 17 - 9}\trow-{identity:06d}\t"
            f"{MEMO_UNITS}\t{value_hash}"
        )
    identities = ROWS * (ROWS + 1) // 2
    scores = identities * 17 - 9 * ROWS
    expected.append(
        f"Summary\t{ROWS}\t{identities}\t{scores}\t{ROWS * MEMO_UNITS}\t"
        f"{all_memos.hexdigest()}"
    )
    return ("\n".join(expected) + "\n").encode(), memo_hashes, scores


class Replay:
    def __init__(self, args, classpath):
        self.args = args
        self.classpath = classpath
        self.commands = []

    def run(self, label, kind, arguments, password=None, refusal=None):
        result = subprocess.run(
            [str(argument) for argument in arguments],
            input=None if password is None else password.encode() + b"\n",
            capture_output=True,
            timeout=180,
        )
        self.commands.append({
            "case": label,
            "kind": kind,
            "exit_code": result.returncode,
            "stdout_bytes": len(result.stdout),
            "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
            "stderr_sha256": hashlib.sha256(result.stderr).hexdigest(),
            "expected_refusal": refusal,
        })
        if refusal:
            if (result.returncode != 2 or result.stdout
                    or refusal not in result.stderr.decode().lower()):
                raise RuntimeError(f"Expected refusal differs: {label}: {result.stderr!r}")
        elif result.returncode or result.stderr:
            raise RuntimeError(f"Command failed: {label}: {result.stderr!r}")
        return result.stdout

    def cli(self, label, arguments, password=None, refusal=None, baseline=False):
        executable = self.args.baseline if baseline else self.args.cli
        invocation = [executable, "--format", "tsv", *arguments]
        if password is not None:
            invocation.extend(["-p", password])
        return self.run(label, "baseline" if baseline else "cli", invocation, refusal=refusal)

    def oracle(self, label, driver, path, password=None, *extra):
        return self.run(label, "java", [
            "java", "-Xmx512m", "-Dstdout.encoding=UTF-8", "-cp", self.classpath,
            driver, path, *extra,
        ], password=password)

    def browsing(self, label, path, password, memo_hashes, scores):
        options = ["--native-bytes", "268435456"]
        tables = self.cli(label + "/tables", ["tables", path, "--count", *options], password)
        if tables != b"Name\tKind\tRow Count\nLargeDeployment\tUser\t1100\n":
            raise RuntimeError("Table enumeration or stored count differs")
        for identity in (1, 550, ROWS):
            output = self.cli(label + f"/memo-{identity}", [
                "rows", path, "LargeDeployment", "--offset", str(identity - 1),
                "--limit", "1", *options,
            ], password)
            rows = list(csv.reader(io.StringIO(output.decode()), delimiter="\t", strict=True))
            if len(rows) != 2 or rows[0] != ["Id", "Score", "Label", "Memo"]:
                raise RuntimeError("Selected native row shape differs")
            row = rows[1]
            value = json.loads(row[3]).encode("ascii")
            if (row[:3] != [str(identity), str(identity * 17 - 9), json.dumps(f"row-{identity:06d}")]
                    or len(value) != MEMO_UNITS
                    or hashlib.sha256(value).hexdigest() != memo_hashes[identity]):
                raise RuntimeError("Selected native row differs from byte/integer model")
        query_options = ["--database", path, "--stream", *options, "--native-steps", "10000000"]
        aggregate = self.cli(label + "/aggregate", [
            "query", "run", "SELECT Count(*) AS N,Min(Id) AS Low,Max(Id) AS High,"
            "Sum(Score) AS S FROM LargeDeployment", *query_options,
        ], password)
        if aggregate != f"N\tLow\tHigh\tS\n{ROWS}\t1\t{ROWS}\t{scores}\n".encode():
            raise RuntimeError("Progressive aggregate differs from integer model")
        output = self.cli(label + "/stream", [
            "query", "run", "SELECT Id,Score,Label FROM LargeDeployment",
            *query_options, "--batch-rows", "128",
        ], password)
        expected = [["Id", "Score", "Label"]]
        expected.extend([
            str(identity), str(identity * 17 - 9), json.dumps(f"row-{identity:06d}"),
        ] for identity in range(1, ROWS + 1))
        if list(csv.reader(io.StringIO(output.decode()), delimiter="\t", strict=True)) != expected:
            raise RuntimeError("Complete progressive projection differs from model")


def snapshots(revision):
    result = {}
    for name in SOURCE_PATHS:
        content = subprocess.run(
            ["git", "show", f"{revision}:{name}"], check=True, capture_output=True,
        ).stdout
        result[name] = hashlib.sha256(content).hexdigest()
        if digest(ROOT / name) != result[name]:
            raise RuntimeError(f"Current source differs from selected revision: {name}")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["directory", "jar", "crypt-jar", "bc-jar", "cli", "baseline", "report", "work-directory"]:
        parser.add_argument("--" + name, required=True, type=Path)
    for name in ["cli-sha256", "baseline-sha256", "revision"]:
        parser.add_argument("--" + name, required=True)
    args = parser.parse_args()
    if args.report.exists():
        raise RuntimeError("Report must be fresh")
    if digest(args.cli) != args.cli_sha256 or digest(args.baseline) != args.baseline_sha256:
        raise RuntimeError("Executable digest differs from explicit frozen pin")
    classpath = rewrite_classpath(args.jar, args.crypt_jar, args.bc_jar)
    for name, expected in INPUTS.items():
        if digest(args.directory / name) != expected:
            raise RuntimeError("Original fixture digest differs")
    revision = subprocess.run(
        ["git", "rev-parse", "--verify", f"{args.revision}^{{commit}}"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()
    source_pins = snapshots(revision)
    immutable = [
        args.cli, args.baseline, args.jar, args.crypt_jar, args.bc_jar,
        Path(__file__), RECIPES / "encryption_references.py",
        RECIPES / "compare_large_deployment.py", RECIPES / "build_large_deployment.py",
        RECIPES / "compare_union_types.py", RECIPES / "compare_browsing.py",
        *(RECIPES / driver for driver in DRIVERS),
        *(args.directory / name for name in INPUTS),
    ]
    pins = {str(path.resolve().relative_to(ROOT)): digest(path) for path in immutable}
    expected_rows, memo_hashes, scores = model()
    report = {
        "date": "2026-10-25", "revision": revision,
        "scope": "Large native page-copy engineering replay; not Microsoft Access qualification.",
        "immutable_before": pins, "source_sha256": source_pins,
        "model_rows": ROWS, "model_memo_units": ROWS * MEMO_UNITS,
        "model_getter_stdout_sha256": hashlib.sha256(expected_rows).hexdigest(),
        "profiles": {}, "commands": [], "complete": False,
    }
    args.work_directory.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="large-encryption-", dir=args.work_directory) as work:
        work = Path(work)
        compilation = subprocess.run([
            "javac", "-cp", classpath, "-d", work,
            *(RECIPES / driver for driver in DRIVERS),
        ], check=True, capture_output=True, timeout=120)
        if compilation.stdout or compilation.stderr:
            raise RuntimeError("Unexpected compiler diagnostics")
        replay = Replay(args, os.pathsep.join([classpath, str(work)]))
        report["commands"] = replay.commands
        try:
            for name in INPUTS:
                source = args.directory / name
                label = source.stem
                outputs = {stage: work / f"{label}-{stage}.accdb" for stage in [
                    "added", "changed", "removed", "removed-changed", "independent-added", "independent-changed",
                ]}
                source_rows = replay.oracle(label + "/original/getters", "LargeEncryptedReadOracle", source)
                if source_rows != expected_rows:
                    raise RuntimeError("Original getter rows differ from independent model")
                source_sids = replay.oracle(label + "/original/sids", "NativeSecurityOracle", source)
                source_pages = replay.oracle(label + "/original/pages", "EncryptedPagesOracle", source)
                replay.browsing(label + "/original", source, None, memo_hashes, scores)
                replay.cli(label + "/baseline-add", [
                    "encryption", "add", source, "--output", work / f"{label}-baseline.accdb",
                    "--new-password", PUBLIC_PASSWORD,
                ], refusal="structural limit", baseline=True)
                if (work / f"{label}-baseline.accdb").exists():
                    raise RuntimeError("Baseline refusal published an output")

                replay.cli(label + "/add", [
                    "encryption", "add", source, "--output", outputs["added"],
                    "--new-password", PUBLIC_PASSWORD,
                ])
                replay.cli(label + "/change", [
                    "encryption", "change-password", outputs["added"], "--output", outputs["changed"],
                    "--new-password", REPLACEMENT,
                ], PUBLIC_PASSWORD)
                replay.cli(label + "/remove", [
                    "encryption", "remove", outputs["added"], "--output", outputs["removed"],
                ], PUBLIC_PASSWORD)
                replay.cli(label + "/remove-changed", [
                    "encryption", "remove", outputs["changed"], "--output", outputs["removed-changed"],
                ], REPLACEMENT)
                for stage, password in [("added", PUBLIC_PASSWORD), ("changed", REPLACEMENT)]:
                    replay.oracle(label + f"/independent-{stage}", "RemoveAddedEncryptionOracle",
                                  outputs[stage], password, outputs[f"independent-{stage}"])
                for stage in ["removed", "removed-changed", "independent-added", "independent-changed"]:
                    if digest(outputs[stage]) != INPUTS[name]:
                        raise RuntimeError(f"Complete independently recovered image differs: {stage}")

                page_hashes = {}
                for stage, password in [("added", PUBLIC_PASSWORD), ("changed", REPLACEMENT), ("removed", None)]:
                    rows = replay.oracle(label + f"/{stage}/getters", "LargeEncryptedReadOracle", outputs[stage], password)
                    if rows != expected_rows:
                        raise RuntimeError(f"Complete getter rows differ: {stage}")
                    sids = replay.oracle(label + f"/{stage}/sids", "NativeSecurityOracle", outputs[stage], password)
                    if sids != source_sids:
                        raise RuntimeError(f"Decoded native SIDs differ: {stage}")
                    pages = replay.oracle(label + f"/{stage}/pages", "EncryptedPagesOracle", outputs[stage], password)
                    if stage == "removed" and pages != source_pages:
                        raise RuntimeError("Recovered decoded page digests differ")
                    page_hashes[stage] = hashlib.sha256(pages).hexdigest()
                    replay.browsing(label + f"/{stage}", outputs[stage], password, memo_hashes, scores)
                if page_hashes["added"] != page_hashes["changed"]:
                    raise RuntimeError("Agile rewrap changed independently decoded pages")
                with outputs["added"].open("rb") as added, outputs["changed"].open("rb") as changed:
                    added.seek(4096)
                    changed.seek(4096)
                    while block := added.read(65536):
                        if changed.read(len(block)) != block:
                            raise RuntimeError("Agile rewrap changed ciphertext")
                    if changed.read(1):
                        raise RuntimeError("Agile rewrap changed file length")
                protected_hash = digest(outputs["added"])
                replay.cli(label + "/existing-output", [
                    "encryption", "add", source, "--output", outputs["added"],
                    "--new-password", PUBLIC_PASSWORD,
                ], refusal="alreadyexists")
                for stage, password in [("added", "wrong public password"), ("changed", PUBLIC_PASSWORD)]:
                    refused = work / f"{label}-{stage}-refused.accdb"
                    replay.cli(label + f"/{stage}/wrong-password", [
                        "encryption", "remove", outputs[stage], "--output", refused,
                    ], password, refusal="incorrect")
                    if refused.exists():
                        raise RuntimeError("Credential refusal published an output")
                if digest(outputs["added"]) != protected_hash:
                    raise RuntimeError("Existing output changed during refusal")
                report["profiles"][name] = {
                    "bytes": source.stat().st_size, "data_pages": source.stat().st_size // 4096 - 1,
                    "outputs": {stage: digest(path) for stage, path in outputs.items()},
                    "independent_page_stdout_sha256": page_hashes,
                    "decoded_sids_stdout_sha256": hashlib.sha256(source_sids).hexdigest(),
                    "getter_rows_compared": 4 * ROWS,
                    "cli_projection_rows_compared": 4 * ROWS,
                    "all_recovered_images_exact": True, "agile_data_ciphertext_unchanged": True,
                }
                print(f"{name}: add/change/remove, independent full images/getters/SIDs/pages and CLI queries pass", flush=True)
                for path in outputs.values():
                    path.unlink()
            report["immutable_after"] = {path: digest(ROOT / path) for path in pins}
            if report["immutable_after"] != pins or snapshots(revision) != source_pins:
                raise RuntimeError("Immutable inputs or source snapshots changed")
            report["complete"] = True
        finally:
            report["command_count"] = len(replay.commands)
            report["expected_refusals"] = sum(bool(item["expected_refusal"]) for item in replay.commands)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Recorded {report['command_count']} commands; {report['expected_refusals']} expected refusals", flush=True)


if __name__ == "__main__":
    main()
