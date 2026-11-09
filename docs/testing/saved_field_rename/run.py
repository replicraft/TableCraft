"""Observe frozen raw copies through a read-only SDK; perform no SDK mutation."""
from pathlib import Path
import argparse
import copy
import hashlib
import json
import os
import subprocess

HERE = Path(__file__).resolve().parent
SDK_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def execute(command, root, label):
    result = subprocess.run(command, capture_output=True, text=True)
    (root / (label + ".stdout")).write_text(result.stdout)
    (root / (label + ".stderr")).write_text(result.stderr)
    return result


def parse(text):
    result = {"columns": [], "options": [], "table": []}
    for line in text.splitlines():
        fields = line.split("\t")
        if fields[0] == "COLUMN":
            result["columns"].append({
                "index": int(fields[1]),
                "id": int(fields[2]),
                "name": fields[3],
                "type": fields[4],
                "units": int(fields[5]),
                "compressed": fields[6],
                "properties": [],
            })
        elif fields[0] == "PROPERTY":
            result["columns"][int(fields[1])]["properties"].append(fields[2:])
        elif fields[0] == "OPTION":
            result["options"].append(fields[1:])
        elif fields[0] == "TABLE":
            result["table"] = fields[1:]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--java-bin", type=Path, required=True)
    args = parser.parse_args()
    root = args.output_dir
    frozen = json.loads((root / "predictions-before-observation.json").read_text())
    assert digest((HERE / "model.py").read_bytes()) == frozen["model_sha256"]
    assert digest((HERE / "FieldRenameObserver.java").read_bytes()) == frozen["observer_sha256"]
    assert digest(args.jar.read_bytes()) == SDK_SHA256
    assert not (root / "observations.json").exists()
    classes = root / "classes"
    classes.mkdir(exist_ok=False)
    compiled = execute([
        str(args.java_bin / "javac"), "-cp", str(args.jar), "-d", str(classes),
        str(HERE / "FieldRenameObserver.java"),
    ], root, "compile")
    assert compiled.returncode == 0
    classpath = str(classes) + os.pathsep + str(args.jar)
    baselines = {}
    for item in frozen["cases"]:
        case = item["case"]
        if case in baselines:
            continue
        path = root / "files" / (case + "-before.accdb")
        before = path.read_bytes()
        assert digest(before) == item["before_sha256"]
        process = execute([
            str(args.java_bin / "java"), "-Xmx256m", "-cp", classpath,
            "FieldRenameObserver", str(path),
        ], root, case + "-before")
        assert process.returncode == 0
        assert path.read_bytes() == before
        baselines[case] = parse(process.stdout)
    observations = []
    for item in frozen["cases"]:
        path = root / item["after"]
        before = path.read_bytes()
        assert digest(before) == item["after_sha256"]
        process = execute([
            str(args.java_bin / "java"), "-Xmx256m", "-cp", classpath,
            "FieldRenameObserver", str(path),
        ], root, item["case"] + "-" + item["variant"])
        entry = {
            **item,
            "sdk_exit": process.returncode,
            "readonly_output_unchanged": path.read_bytes() == before,
            "original_unchanged": digest((args.sources_dir / item["source"]).read_bytes())
                == item["before_sha256"],
        }
        if process.returncode == 0:
            actual = parse(process.stdout)
            expected = copy.deepcopy(baselines[item["case"]])
            selected = expected["columns"][item["index"]]
            assert selected["name"] == item["old_name"]
            selected["name"] = item["new_name"]
            entry["observation"] = actual
            entry["metadata_except_requested_name_matches"] = actual == expected
            entry["selected_field_properties_before"] = selected["properties"]
            entry["selected_field_properties_after"] = actual["columns"][item["index"]]["properties"]
        observations.append(entry)
        (root / "observations.json").write_text(json.dumps({
            "sdk_sha256": SDK_SHA256,
            "observer_sha256": frozen["observer_sha256"],
            "baselines": baselines,
            "cases": observations,
        }, indent=2) + "\n")
        print(item["case"], item["variant"], process.returncode,
              entry.get("metadata_except_requested_name_matches"), flush=True)
        assert entry["original_unchanged"] and entry["readonly_output_unchanged"]
        assert process.returncode == 0
        assert entry["metadata_except_requested_name_matches"] == (item["variant"] == "live-group")


if __name__ == "__main__":
    main()
