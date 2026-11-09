"""Run only frozen fresh-copy predictions; preserve every original and log."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

import model


SDK_SHA256 = "19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def execute(command, directory, label):
    process = subprocess.run(command, capture_output=True)
    (directory / (label + ".stdout")).write_bytes(process.stdout)
    (directory / (label + ".stderr")).write_bytes(process.stderr)
    return {"command": command, "exit": process.returncode}


def compare(before, actual, predicted, facts):
    page = model.PAGE
    catalog = 4 * page
    lval = 10 * page
    old_rows = model.u16(before, catalog + 12)
    old_lval_rows = model.u16(before, lval + 12)
    old_lval_low = model.u16(before, lval + 14 + 2 * (old_lval_rows - 1))
    old_catalog_low = model.u16(before, catalog + 14 + 2 * (old_rows - 1)) & 0x1FFF
    db_start = model.u16(actual, catalog + 14 + 2 * facts["catalog_slot"])
    descriptor = actual[catalog + db_start + 44:catalog + db_start + 56]
    return {
        "complete_image_match": actual == predicted,
        "same_length": len(actual) == len(before),
        "catalog_logical_count": model.u32(actual, 2 * page + 16),
        "catalog_physical_count": model.u16(actual, catalog + 12),
        "db_catalog_slot": facts["catalog_slot"],
        "db_descriptor_hex": descriptor.hex(),
        "db_descriptor_matches": descriptor == predicted[catalog + db_start + 44:catalog + db_start + 56],
        "user_catalog_row_and_descriptor_unchanged": actual[catalog + old_catalog_low:catalog + page] == before[catalog + old_catalog_low:catalog + page],
        "old_catalog_directory_unchanged": actual[catalog + 14:catalog + 14 + 2 * old_rows] == before[catalog + 14:catalog + 14 + 2 * old_rows],
        "old_lval_rows_unchanged": actual[lval + old_lval_low:lval + page] == before[lval + old_lval_low:lval + page],
        "old_lval_directory_unchanged": actual[lval + 14:lval + 14 + 2 * old_lval_rows] == before[lval + 14:lval + 14 + 2 * old_lval_rows],
        "all_maps_page_unchanged": actual[3 * page:4 * page] == before[3 * page:4 * page],
        "changed_pages": [index for index in range(len(before) // page)
                          if actual[index * page:(index + 1) * page] != before[index * page:(index + 1) * page]],
        "mismatch_offsets_first_64": [index for index, pair in enumerate(zip(actual, predicted))
                                      if pair[0] != pair[1]][:64],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--java-bin", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).parent
    assert sha(args.jar.read_bytes()) == SDK_SHA256
    frozen = json.loads((root / "predictions-before-sdk.json").read_text())
    assert sha((root / "model.py").read_bytes()) == frozen["model_sha256"]
    assert not (root / "execution.json").exists()
    classes = root / "classes"
    classes.mkdir(exist_ok=True)
    compiled = execute([str(args.java_bin / "javac"), "-cp", str(args.jar), "-d", str(classes), str(root / "InsertOptions.java")], root, "compile")
    assert compiled["exit"] == 0
    report = {"started_unix": time.time(), "sdk_sha256": SDK_SHA256,
              "inert_helper_sha256": sha((root / "InsertOptions.java").read_bytes()),
              "frozen_prediction_sha256": sha((root / "predictions-before-sdk.json").read_bytes()),
              "compile": compiled, "cases": []}
    for case in frozen["cases"]:
        original = Path(case["source"])
        before = original.read_bytes()
        assert sha(before) == case["source_sha256"]
        output = root / "outputs" / (case["case"] + ".accdb")
        assert not output.exists()
        shutil.copyfile(root / "inputs" / output.name, output)
        assert output.read_bytes() == before
        command = [str(args.java_bin / "java"), "-Xmx256m", "-cp", str(classes) + os.pathsep + str(args.jar),
                   "InsertOptions", "insert-complete-off", str(output)]
        execution = execute(command, root, case["case"])
        actual = output.read_bytes()
        expected = (root / "expected" / output.name).read_bytes()
        entry = {**case, "execution": execution, "original_unchanged": original.read_bytes() == before,
                 "after_sha256": sha(actual), "after_bytes": len(actual)}
        if execution["exit"] == 0:
            entry.update(compare(before, actual, expected, case))
        report["cases"].append(entry)
        (root / "execution.json").write_text(json.dumps(report, indent=2) + "\n")
        print(case["case"], execution["exit"], entry.get("complete_image_match"), entry["after_sha256"], flush=True)
    report["finished_unix"] = time.time()
    (root / "execution.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
