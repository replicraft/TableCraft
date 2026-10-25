"""Observe original public-writer deletes and frozen native reader results."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"
CLI_SHA256 = "d19b3d30ac01c34aafe765cb0f8c0a2cd54c8ba234c6021f8c15065715caf041"
PROFILES = ("V2000", "V2007", "V2016", "V2019")
STAGES = ("before", "delete-one", "delete-null", "delete-all")
TABLES = ("Indexed", "Unindexed", "Sentinel")
SLOT_BYTES = 2
ROW_OFFSET_MASK = 0x1FFF
TOMBSTONE_FLAGS = 0xC000
PAGE_OWNER_OFFSET = 4
PAGE_FREE_OFFSET = 2
PAGE_PREFIX_OFFSET = 24
INDEX_LEAF_KIND = 4
INDEX_PAGE_VERSION = 1
ROW_REFERENCE_BYTES = 4
ROW_REFERENCE_SHIFT = 8
CHECK_NATIVE_STEPS = 10_000_000


def digest(data):
    return hashlib.sha256(data).hexdigest()


def label(path):
    return Path(path).resolve().relative_to(ROOT).as_posix()


def pin(path):
    data = Path(path).read_bytes()
    return {"path": label(path), "sha256": digest(data), "bytes": len(data)}


class Commands:
    def __init__(self, path):
        self.path = path
        self.count = 0
        self.cli_controls = []
        self.disagreements = []

    def run(self, arguments):
        command = [str(argument) for argument in arguments]
        completed = subprocess.run(
            command,
            cwd=ROOT,
            capture_output=True,
            timeout=60,
            check=False,
        )
        self.count += 1
        record = {
            "ordinal": self.count,
            "command": command,
            "exit_code": completed.returncode,
            "stdout": completed.stdout.decode("utf-8"),
            "stderr": completed.stderr.decode("utf-8"),
        }
        with self.path.open("a", encoding="utf-8") as output:
            output.write(json.dumps(record, ensure_ascii=False) + "\n")
        return record

    def require(self, arguments):
        record = self.run(arguments)
        if record["exit_code"] != 0:
            raise AssertionError(f"Producer/getter failed; command ordinal {record['ordinal']}")
        return record["stdout"]

    def cli(self, arguments, expected_rows=None, refusal=False, contains=()):
        record = self.run(arguments)
        if refusal:
            valid = record["exit_code"] != 0 and record["stdout"] == ""
            valid = valid and record["stderr"] == "error: query feature is not implemented\n"
        else:
            valid = record["exit_code"] == 0 and record["stderr"] == ""
            if expected_rows is not None:
                actual = list(csv.reader(io.StringIO(record["stdout"]), delimiter="\t"))
                valid = valid and actual[1:] == expected_rows
            valid = valid and all(text in record["stdout"] for text in contains)
        result = {
            "command_ordinal": record["ordinal"],
            "expected": "unsupported selected ignore-NULL index" if refusal else "getter/model agreement",
            "agrees": valid,
            "exit_code": record["exit_code"],
            "stdout_sha256": digest(record["stdout"].encode()),
            "stderr_sha256": digest(record["stderr"].encode()),
        }
        self.cli_controls.append(result)
        if not valid:
            self.disagreements.append(result)


def oracle(text):
    result = {name: {"rows": {}, "locations": {}, "indexes": {}} for name in TABLES}
    for line in text.splitlines():
        cells = line.split("\t")
        tag, table = cells[:2]
        item = result[table]
        if tag == "TABLE":
            item["geometry"] = dict(zip(
                (
                    "live_rows", "definition_page", "page_bytes", "count_offset",
                    "row_start", "mask_offset", "mask_bytes",
                ),
                map(int, cells[2:]),
            ))
        elif tag == "LOCATION":
            item["locations"][int(cells[2])] = tuple(map(int, cells[3:]))
        elif tag == "ROW":
            item["rows"][int(cells[2])] = cells[3:]
        elif tag == "INDEX":
            item["indexes"][cells[2]] = {
                "physical": int(cells[3]),
                "unique_entry_count": int(cells[4]),
                "counter_offset": int(cells[5]),
                "entry_count": int(cells[6]),
                "owned_pages": int(cells[7]),
                "unique": cells[8] == "true",
                "ignore_nulls": cells[9] == "true",
                "members": [],
            }
        elif tag == "ENTRY":
            item["indexes"][cells[2]]["members"].append(int(cells[3]))
        else:
            raise AssertionError(f"Unknown getter record {tag}")
    return result


def u16(data, offset):
    return int.from_bytes(data[offset:offset + 2], "little")


def u32(data, offset):
    return int.from_bytes(data[offset:offset + 4], "little")


def raw_rows(data, baseline):
    result = {}
    for table, info in baseline.items():
        geometry = info["geometry"]
        page_bytes = geometry["page_bytes"]
        observations = []
        for identifier, (page, slot) in info["locations"].items():
            offset = page * page_bytes + geometry["row_start"] + SLOT_BYTES * slot
            word = u16(data, offset)
            start = page * page_bytes + (word & ROW_OFFSET_MASK)
            if slot == 0:
                end = (page + 1) * page_bytes
            else:
                preceding_word = u16(data, offset - SLOT_BYTES)
                end = page * page_bytes + (preceding_word & ROW_OFFSET_MASK)
            observations.append({
                "id": identifier,
                "page": page,
                "slot": slot,
                "word_offset": offset,
                "word": word,
                "flags": word & TOMBSTONE_FLAGS,
                "payload_sha256": digest(data[start:end]),
            })
        result[table] = observations
    return result


def raw_index_pages(data, info):
    geometry = info["geometry"]
    width = geometry["page_bytes"]
    start = geometry["mask_offset"] + geometry["mask_bytes"]
    result = []
    locations = {tuple(location): identifier for identifier, location in info["locations"].items()}
    for number in range(len(data) // width):
        page = data[number * width:(number + 1) * width]
        if page[:2] != bytes((INDEX_LEAF_KIND, INDEX_PAGE_VERSION)):
            continue
        if u32(page, PAGE_OWNER_OFFSET) != geometry["definition_page"]:
            continue
        boundaries = [
            byte * 8 + bit
            for byte, value in enumerate(page[geometry["mask_offset"]:start])
            for bit in range(8)
            if value & (1 << bit)
        ]
        prefix = u16(page, PAGE_PREFIX_OFFSET)
        previous = 0
        first = b""
        references = []
        for end in boundaries:
            encoded = page[start + previous:start + end]
            if previous == 0:
                first = encoded
            else:
                encoded = first[:prefix] + encoded
            reference = int.from_bytes(encoded[-ROW_REFERENCE_BYTES:], "big")
            references.append(locations[(reference >> ROW_REFERENCE_SHIFT, reference & 0xFF)])
            previous = end
        assert u16(page, PAGE_FREE_OFFSET) == width - start - previous
        result.append({
            "page": number,
            "free_bytes": u16(page, PAGE_FREE_OFFSET),
            "prefix_bytes": prefix,
            "entry_count": len(boundaries),
            "member_ids": references,
            "sha256": digest(page),
        })
    return result



def reference_change_locality(before, after, baseline, row_slots, leaves):
    assert len(before) == len(after)
    allowed = []
    for table in ("Indexed", "Unindexed"):
        geometry = baseline[table]["geometry"]
        count = geometry["definition_page"] * geometry["page_bytes"] + geometry["count_offset"]
        allowed.append((count, count + 4))
        for row in row_slots[table]:
            if row["flags"]:
                allowed.append((row["word_offset"], row["word_offset"] + SLOT_BYTES))
    width = baseline["Indexed"]["geometry"]["page_bytes"]
    allowed.extend((page["page"] * width, (page["page"] + 1) * width) for page in leaves)
    changed = [offset for offset, (left, right) in enumerate(zip(before, after)) if left != right]
    outside = [offset for offset in changed if not any(start <= offset < end for start, end in allowed)]
    assert not outside
    return {
        "changed_bytes": len(changed),
        "changed_bytes_outside_row_slots_live_counts_and_owned_index_pages": len(outside),
        "same_image_length": True,
    }


def deleted_ids(stage, baseline):
    if stage == "before":
        return set()
    if stage == "delete-all":
        return set(baseline)
    return {2 if stage == "delete-one" else 3} & set(baseline)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--workdir", type=Path, required=True)
    parser.add_argument("--inputs", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--cli-sha256", default=CLI_SHA256)
    parser.add_argument("--compiled-revision", required=True)
    args = parser.parse_args()
    jar = args.jar.resolve()
    cli = args.cli.resolve()
    work = args.workdir.resolve()
    report_path = args.report.resolve()
    if digest(jar.read_bytes()) != JAR_SHA256 or digest(cli.read_bytes()) != args.cli_sha256:
        raise AssertionError("Reviewed artifact hash differs")
    if work.exists() or report_path.exists():
        raise AssertionError("Work directory and report must be fresh")
    work.mkdir(parents=True)
    commands = Commands(work / "commands.jsonl")
    tools = [jar, cli, Path(__file__).resolve(), ROOT / "docs/testing/DeleteStorageFixture.java"]
    tool_before = [pin(path) for path in tools]
    captures = []
    observations = []
    java = ["java", "-Dstdout.encoding=UTF-8", "-cp", label(jar), "docs/testing/DeleteStorageFixture.java"]

    for profile in PROFILES:
        inputs = args.inputs.resolve() if args.inputs else work
        if args.inputs is None:
            commands.require([*java, "generate", label(work), profile])
        paths = {
            stage: inputs / f"Delete-{profile}-{stage}.accdb"
            for stage in STAGES
        }
        original_pins = {stage: pin(path) for stage, path in paths.items()}
        getter = {}
        images = {}
        for stage, path in paths.items():
            text = commands.require([*java, "oracle", label(path)])
            capture = work / f"Getter-{profile}-{stage}.tsv"
            capture.write_text(text)
            captures.append(pin(capture))
            getter[stage] = oracle(text)
            images[stage] = path.read_bytes()
        baseline = getter["before"]
        before_raw = raw_rows(images["before"], baseline)
        for stage, path in paths.items():
            current = getter[stage]
            raw = raw_rows(images[stage], baseline)
            for name, before_table in baseline.items():
                removed = deleted_ids(stage, before_table["rows"]) if name != "Sentinel" else set()
                expected_rows = {
                    identifier: row
                    for identifier, row in before_table["rows"].items()
                    if identifier not in removed
                }
                assert current[name]["rows"] == expected_rows
                geometry = current[name]["geometry"]
                count_address = geometry["definition_page"] * geometry["page_bytes"] + geometry["count_offset"]
                assert geometry["live_rows"] == len(expected_rows) == u32(images[stage], count_address)
                for old, now in zip(before_raw[name], raw[name]):
                    assert old["id"] == now["id"]
                    assert old["payload_sha256"] == now["payload_sha256"]
                    assert now["word"] == old["word"] | (TOMBSTONE_FLAGS if old["id"] in removed else 0)
                for index, before_index in before_table["indexes"].items():
                    item = current[name]["indexes"][index]
                    expected_members = [
                        identifier
                        for identifier in before_index["members"]
                        if identifier not in removed
                    ]
                    assert item["members"] == expected_members
                    assert item["entry_count"] == len(item["members"])
                    counter_address = (
                        geometry["definition_page"] * geometry["page_bytes"]
                        + item["counter_offset"]
                    )
                    assert item["unique_entry_count"] == u32(images[stage], counter_address)
                    assert item["unique_entry_count"] == before_index["unique_entry_count"]
                    assert item["owned_pages"] == 1
            leaves = raw_index_pages(images[stage], baseline["Indexed"])
            physical = {item["physical"] for item in current["Indexed"]["indexes"].values()}
            assert len(leaves) == len(physical) == 6
            if stage == "delete-all":
                assert all(page["entry_count"] == page["prefix_bytes"] == 0 for page in leaves)
            observations.append({
                "profile": profile,
                "stage": stage,
                "tables": current,
                "row_slots": raw,
                "indexed_root_leaf_pages": leaves,
                "reference_change_locality": reference_change_locality(
                    images["before"], images[stage], baseline, raw, leaves,
                ),
            })
            common = [label(cli), "--format", "tsv"]
            commands.cli(
                [*common, "inspect", label(path), "--table", "Indexed", "--indexes"],
                contains=("PrimaryKey", "GroupTwin", "GroupIgnoreNulls"),
            )
            commands.cli(
                [*common, "inspect", label(path), "--table", "Indexed", "--check",
                 "--native-steps", str(CHECK_NATIVE_STEPS)],
                contains=(
                    f"Rows checked\t{len(current['Indexed']['rows'])}",
                    "GroupIgnoreNulls\tNot checked: unsupported profile",
                ),
            )
            for name in ("Indexed", "Unindexed"):
                columns = "ID,GroupKey,Part,Score" if name == "Indexed" else "ID,Score"
                selected_cells = 3 if name == "Indexed" else 1
                expected = [
                    [str(identifier), *row[:selected_cells]]
                    for identifier, row in current[name]["rows"].items()
                ]
                commands.cli(
                    [*common, "query", "run", "--database", label(path),
                     f"SELECT {columns} FROM {name}"],
                    expected_rows=expected,
                )
            commands.cli(
                [*common, "query", "run", "--database", label(path),
                 "SELECT Count(*) FROM Indexed"],
                expected_rows=[[str(len(current["Indexed"]["rows"]))]],
            )
            for index, item in current["Indexed"]["indexes"].items():
                expected = [[str(identifier)] for identifier in item["members"]]
                commands.cli(
                    [*common, "query", "run", "--database", label(path),
                     "--index", "Indexed", index, "SELECT ID FROM Indexed"],
                    expected_rows=expected,
                    refusal=item["ignore_nulls"],
                )
            for selected_stage, selected_path in paths.items():
                assert pin(selected_path) == original_pins[selected_stage]
        captures.extend(original_pins.values())

    assert [pin(path) for path in tools] == tool_before
    report = {
        "date": "2026-10-25",
        "scope": "Original public-writer deletes and read-only frozen native reader replay",
        "native_access_qualified": False,
        "compiled_revision": args.compiled_revision,
        "input_reuse": args.inputs is not None,
        "check_native_steps": CHECK_NATIVE_STEPS,
        "producer_profile_states": len(observations),
        "cli_controls": len(commands.cli_controls),
        "cli_disagreements": len(commands.disagreements),
        "known_selected_ignore_null_refusals": sum(
            control["expected"].startswith("unsupported")
            for control in commands.cli_controls
        ),
        "observations": observations,
        "controls": commands.cli_controls,
        "disagreements": commands.disagreements,
        "pins": [*tool_before, *captures, pin(commands.path)],
        "artifact_limits": [
            "Fresh original reference-writer files; no native Microsoft Access "
            "provenance or reference SQL execution.",
            "Public builders/delete/getters plus pinned implementation metadata "
            "getters; raw slot/count/root-leaf bytes independently decoded in Python.",
            "Ignore-NULL index membership is observed through reference getters; "
            "frozen native selected reads retain existing unsupported refusal.",
            "Unique-entry counters remain unchanged in these four states despite "
            "changing live cardinality; no universal counter semantics inferred.",
        ],
        "excluded": [
            {
                "path": ".cache/tablecraft-fixtures/delete-storage-probe-20261025",
                "reason": "Initial V2016 capability probe before final oracle "
                "geometry/runner; preserved, excluded from accepted counts.",
            },
            {
                "report": pin(ROOT / ".cache/delete-storage-initial-report-20261025.json"),
                "runner": pin(ROOT / ".cache/delete-storage-initial-runner-20261025.py"),
                "log": pin(ROOT / ".cache/delete-storage-replay-20261025.log"),
                "controls": 192,
                "initial_disagreements": 28,
                "classification": {
                    "known_ignore_null_refusal_wording": 16,
                    "full_check_default_steps_resource_refusals": 12,
                    "confirmed_delete_reader_corruption": 0,
                },
                "reason": "Initial runner matched the wrong unsupported wording, "
                "and full checks exhausted default work on existing Memo/OLE values. "
                "Originals remain unchanged; final checks explicitly authorize "
                "10,000,000 work steps.",
            },
        ],
    }
    report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps({
        "report": label(report_path),
        "cli_controls": len(commands.cli_controls),
        "disagreements": len(commands.disagreements),
    }))
    return 1 if commands.disagreements else 0


if __name__ == "__main__":
    sys.exit(main())
