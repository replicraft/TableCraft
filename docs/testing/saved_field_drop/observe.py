"""Execute only inert SDK getters against previously frozen raw copies.

Failed read hypotheses remain observations, not silently discarded retries. This
runner does not call a schema/data mutation API or rewrite any database image.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
SDK_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def execute(command, output, label):
    result = subprocess.run(command, capture_output=True, text=True)
    (output / (label + '.stdout')).write_text(result.stdout)
    (output / (label + '.stderr')).write_text(result.stderr)
    return result


def parsed(text):
    result = {}
    for line in text.splitlines():
        fields = line.split('\t')
        result.setdefault(fields[0], []).append(fields[1:])
    return result


def compare(baseline, actual, selected, variant):
    expected_columns = [row for row in baseline.get('COLUMN', []) if row[2] != selected]
    expected_properties = [row for row in baseline.get('PROPERTY', []) if row[0] != selected]
    before_table = baseline['TABLE'][0]
    expected_table = [[str(int(before_table[0]) - 1), *before_table[1:]]]
    expected_groups = baseline.get('GROUP', [])
    if variant == 'append-live-group-removed':
        expected_groups = [row for row in expected_groups if row[1] != selected]
    return {
        'surviving_native_identity_storage_matches': actual.get('COLUMN', []) == expected_columns,
        'surviving_policies_match': actual.get('PROPERTY', []) == expected_properties,
        'active_count_changed_highwater_retained': actual.get('TABLE') == expected_table,
        'group_hypothesis_matches': actual.get('GROUP', []) == expected_groups,
        'index_and_key_unchanged': all(actual.get(key) == baseline.get(key) for key in ['INDEX', 'KEY']),
        'explicit_off_unchanged': actual.get('OPTION') == baseline.get('OPTION'),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--java-bin', type=Path, required=True)
    args = parser.parse_args()
    output = args.output_dir
    report_path = output / 'observations.json'
    assert not report_path.exists(), 'Preserve previous observations; never rerun in the same directory'
    frozen_path = output / 'predictions-before-observation.json'
    frozen = json.loads(frozen_path.read_text())
    for name, expected in frozen['local_helpers'].items():
        assert digest((HERE / name).read_bytes()) == expected
    for name, expected in frozen['shared_helpers'].items():
        assert digest((args.repo / name).read_bytes()) == expected
    assert digest(args.jar.read_bytes()) == SDK_SHA256
    classes = output / 'classes'
    classes.mkdir(exist_ok=False)
    report = {'sdk_sha256': SDK_SHA256, 'prediction_record_sha256': digest(frozen_path.read_bytes()),
              'scope': 'Raw hypotheses/read-only getters; not SDK DROP or native Access Save',
              'baselines': {}, 'cases': []}

    def persist():
        report_path.write_text(json.dumps(report, indent=2) + '\n')

    compiled = execute([str(args.java_bin / 'javac'), '-cp', str(args.jar), '-d', str(classes),
                        str(HERE / 'FieldDropObserver.java')], output, 'compile')
    report['compile_exit'] = compiled.returncode
    persist()
    if compiled.returncode != 0:
        return
    classpath = os.pathsep.join([str(classes), str(args.jar)])

    def observe(path, label, expected):
        before = path.read_bytes()
        assert digest(before) == expected
        result = execute([str(args.java_bin / 'java'), '-Xmx256m', '-cp', classpath,
                          'FieldDropObserver', str(path)], output, label)
        assert path.read_bytes() == before
        return {'sdk_exit': result.returncode, 'readonly_bytes_exact': True,
                'observed': parsed(result.stdout)}

    for item in frozen['cases']:
        assert 'prediction_failure' not in item
        case = item['case']
        original = args.sources_dir / item['source']
        assert digest(original.read_bytes()) == item['before_sha256']
        if case not in report['baselines']:
            path = output / 'files' / (case + '-before.accdb')
            report['baselines'][case] = observe(path, case + '-before', item['before_sha256'])
            persist()
        observation = observe(output / item['after'], case + '-' + item['variant'], item['after_sha256'])
        entry = {**item, **observation}
        baseline = report['baselines'][case]
        if observation['sdk_exit'] == 0 and baseline['sdk_exit'] == 0:
            entry['comparisons'] = compare(baseline['observed'], observation['observed'],
                                           item['drop'], item['variant'])
        entry['original_bytes_exact'] = digest(original.read_bytes()) == item['before_sha256']
        report['cases'].append(entry)
        persist()
        assert entry['original_bytes_exact']
        print(case, item['variant'], 'SDK exit', observation['sdk_exit'])


if __name__ == '__main__':
    main()
