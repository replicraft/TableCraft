"""Read frozen populated pairs through independent sequential and index cursors."""
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


def observation(text):
    result = parse(text)
    result['native'] = []
    result['rows'] = {'sequential': [], 'indexed': []}
    for line in text.splitlines():
        fields = line.split('\t')
        if fields[0] == 'NATIVE':
            result['native'].append(fields[1:])
        elif fields[0] == 'ROW':
            result['rows'][fields[1]].append([
                int(fields[2]), None if fields[3] == 'NULL' else int(fields[3]),
            ])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--predictions-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--java-bin', type=Path, required=True)
    args = parser.parse_args()
    root = args.output_dir
    root.mkdir(exist_ok=False)
    prediction_path = args.predictions_dir / 'predictions.json'
    frozen = json.loads(prediction_path.read_text())
    assert digest(args.jar.read_bytes()) == SDK_SHA256
    for name, pin in frozen['model_pins'].items():
        assert digest((HERE / name).read_bytes()) == pin
    pins = {name: digest((HERE / name).read_bytes()) for name in [
        'PopulatedRenameObserver.java', 'run_populated.py', 'run.py',
    ]}
    (root / 'inputs-before-observation.json').write_text(json.dumps({
        'source_pins': pins, 'sdk_sha256': SDK_SHA256,
        'prediction_sha256': digest(prediction_path.read_bytes()),
        'mode': 'Read-only; expression evaluation disabled; linked targets refused',
    }, indent=2) + '\n')
    classes = root / 'classes'
    classes.mkdir()
    compiled = execute([
        str(args.java_bin / 'javac'), '-cp', str(args.jar), '-d', str(classes),
        str(HERE / 'PopulatedRenameObserver.java'),
    ], root, 'compile')
    assert compiled.returncode == 0
    classpath = str(classes) + os.pathsep + str(args.jar)
    baselines = {}
    cases = []

    def read(label, path, name, values, pin):
        before = path.read_bytes()
        assert digest(before) == pin
        tokens = ','.join('NULL' if value is None else str(value) for value in values)
        process = execute([
            str(args.java_bin / 'java'), '-Xmx256m', '-cp', classpath,
            'PopulatedRenameObserver', str(path), name, tokens,
        ], root, label)
        item = {'label': label, 'sha256': pin, 'field': name, 'expected_values': values,
                'sdk_exit': process.returncode, 'bytes_unchanged': path.read_bytes() == before,
                'stdout_sha256': digest(process.stdout.encode()),
                'stderr_sha256': digest(process.stderr.encode())}
        if process.returncode == 0:
            item['observation'] = observation(process.stdout)
        cases.append(item)
        (root / 'observations.json').write_text(json.dumps({
            'source_pins': pins, 'sdk_sha256': SDK_SHA256, 'cases': cases,
        }, indent=2) + '\n')
        assert item['bytes_unchanged']
        assert process.returncode == 0, label + ': reader/schema/row mismatch retained in evidence'
        return item

    for item in frozen['cases']:
        label = item['case']
        if label not in baselines:
            baselines[label] = read(
                label + '-before', args.predictions_dir / (label + '-before.accdb'),
                'Score', item['values'], item['before_sha256'],
            )['observation']
        actual = read(
            label + '-' + item['new_name'],
            args.predictions_dir / (label + '-' + item['new_name'] + '.accdb'),
            item['new_name'], item['values'], item['after_sha256'],
        )
        expected = copy.deepcopy(baselines[label])
        expected['columns'][1]['name'] = item['new_name']
        actual['metadata_and_rows_match_except_requested_name'] = actual['observation'] == expected
        (root / 'observations.json').write_text(json.dumps({
            'source_pins': pins, 'sdk_sha256': SDK_SHA256, 'cases': cases,
        }, indent=2) + '\n')
        assert actual['metadata_and_rows_match_except_requested_name']
        print(label, item['new_name'], 'exact rows/metadata through both cursors; bytes unchanged', flush=True)


if __name__ == '__main__':
    main()
