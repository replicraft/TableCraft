"""Observe frozen fitted rename images through the retained read-only SDK helper."""
from pathlib import Path
import argparse
import copy
import hashlib
import json
import os

from run import SDK_SHA256, execute, parse

HERE = Path(__file__).resolve().parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--java-bin', type=Path, required=True)
    args = parser.parse_args()
    root = args.output_dir
    prediction_file = root / 'predictions-before-observation.json'
    frozen = json.loads(prediction_file.read_text())
    assert digest((HERE / 'fitting.py').read_bytes()) == frozen['model_sha256']
    assert digest((HERE / 'model.py').read_bytes()) == frozen['baseline_model_sha256']
    assert digest(args.jar.read_bytes()) == SDK_SHA256
    assert not (root / 'fitting-observations.json').exists()
    pins = {name: digest((HERE / name).read_bytes()) for name in [
        'fitting.py', 'model.py', 'run.py', 'run_fitting.py', 'FieldRenameObserver.java',
    ]}
    execution = root / 'observer-inputs-before-execution.json'
    assert not execution.exists()
    execution.write_text(json.dumps({
        'source_pins': pins,
        'prediction_sha256': digest(prediction_file.read_bytes()),
        'sdk_sha256': SDK_SHA256,
        'scope': 'Read-only first-party raw fixture observations, no SDK rename writes',
    }, indent=2) + '\n')
    classes = root / 'fitting-classes'
    classes.mkdir(exist_ok=False)
    compiled = execute([
        str(args.java_bin / 'javac'), '-cp', str(args.jar), '-d', str(classes),
        str(HERE / 'FieldRenameObserver.java'),
    ], root, 'fitting-compile')
    assert compiled.returncode == 0
    classpath = str(classes) + os.pathsep + str(args.jar)
    baselines = {}
    for item in frozen['cases']:
        if item['case'] in baselines:
            continue
        original = args.sources_dir / item['source']
        before = original.read_bytes()
        assert digest(before) == item['before_sha256']
        baseline = root / 'files' / (item['case'] + '-fitting-before.accdb')
        assert not baseline.exists()
        baseline.write_bytes(before)
        process = execute([
            str(args.java_bin / 'java'), '-Xmx256m', '-cp', classpath,
            'FieldRenameObserver', str(baseline),
        ], root, item['case'] + '-fitting-before')
        assert process.returncode == 0
        assert baseline.read_bytes() == original.read_bytes() == before
        baselines[item['case']] = parse(process.stdout)
    observations = []
    for item in frozen['cases']:
        path = root / item['after']
        before = path.read_bytes()
        assert digest(before) == item['after_sha256']
        label = path.stem
        process = execute([
            str(args.java_bin / 'java'), '-Xmx256m', '-cp', classpath,
            'FieldRenameObserver', str(path),
        ], root, label)
        entry = {
            **item,
            'sdk_exit': process.returncode,
            'readonly_output_unchanged': path.read_bytes() == before,
            'original_unchanged': digest((args.sources_dir / item['source']).read_bytes())
            == item['before_sha256'],
        }
        if process.returncode == 0:
            actual = parse(process.stdout)
            expected = copy.deepcopy(baselines[item['case']])
            selected = [column for column in expected['columns'] if column['name'] == item['old_name']]
            assert len(selected) == 1
            selected[0]['name'] = item['new_name']
            entry['observation'] = actual
            entry['metadata_except_requested_name_matches'] = actual == expected
        observations.append(entry)
        (root / 'fitting-observations.json').write_text(json.dumps({
            'source_pins': pins,
            'sdk_sha256': SDK_SHA256,
            'baselines': baselines,
            'cases': observations,
        }, indent=2) + '\n')
        print(item['case'], item['new_name'], item['variant'], process.returncode,
              entry.get('metadata_except_requested_name_matches'), flush=True)
        assert entry['original_unchanged'] and entry['readonly_output_unchanged']
        if item['variant'] == 'coordinated':
            assert process.returncode == 0 and entry['metadata_except_requested_name_matches']
        # Damaged controls record what this reader actually does. A tolerant
        # getter is not native writer qualification or a structural validator.


if __name__ == '__main__':
    main()
