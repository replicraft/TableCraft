"""Verify immutable first raw60 refusals without native or JVM calls."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
REPORT = '.cache/ace16-long-value-acceptance-first-20261027/report.json'
PLAN = '.cache/ace16-long-value-acceptance-next-20261027/plan.json'


def pin(path):
    data = (ROOT / path).read_bytes()
    return {'path': path, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def verify(expected):
    assert pin(expected['path']) == expected, expected['path']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', default=REPORT)
    parser.add_argument('--plan', default=PLAN)
    args = parser.parse_args()
    report_pin = pin(args.report)
    assert report_pin['bytes'] == 473929
    assert report_pin['sha256'] == 'fe315833bf2c09f01d343c872d64ca6f9ec0443fbd6eed54bb34099a150ca56b'
    plan_pin = pin(args.plan)
    assert plan_pin['sha256'] == 'bbcd7ad0a543b51a02f3e3ef4406eb5e1a6770a5578e8cc0a4d26e68db68fd08'
    report = json.loads((ROOT / args.report).read_text())
    plan = json.loads((ROOT / args.plan).read_text())
    before = json.loads((ROOT / report['before_first_call']['path']).read_text())
    verify(report['before_first_call'])
    unique = {}
    for expected in before['immutable']:
        verify(expected)
        unique[expected['path']] = expected
    for archive in before['archives']:
        verify(archive['source'])
        verify(archive['archive'])
        assert archive['source']['sha256'] == archive['archive']['sha256']
    for expected in plan['immutable_pins']:
        verify(expected)
    for selector in plan['cases']:
        verify(selector['request'])
    counts = Counter()
    assert report['attempted'] == len(report['cases']) == 60
    assert report['matched_candidates'] == 0
    assert report['unmet_controls'] == 60
    assert report['command_count'] == len(report['commands']) == 60
    assert report['fatal'] is None and report['immutable_inputs_unchanged']
    for selector, case in zip(plan['cases'], report['cases'], strict=True):
        assert case['selector'] == selector
        assert not case['complete'] and case['atomic_refusal_observed']
        assert not (ROOT / case['candidate_path']).exists()
        command = case['command']
        assert command['status'] == 1 and not command['timed_out']
        verify(command['stdout'])
        verify(command['stderr'])
        assert command['stdout']['bytes'] == 0
        expected_error = 'Error: Unsupported(Column)\n'
        if selector['profile'] == 'V2019':
            expected_error = 'Error: Inspect(Malformed)\n'
        assert case['actual_error'] == expected_error
        assert (ROOT / command['stderr']['path']).read_text() == expected_error
        counts[(selector['profile'], selector['column'], expected_error.strip())] += 1
    git_pins = report['build']['source_pins']
    revision = report['build']['compiled_revision']
    for expected in git_pins:
        process = subprocess.run(
            ['git', 'show', revision + ':' + expected['path']],
            cwd=ROOT, capture_output=True, check=True, timeout=30,
        )
        assert len(process.stdout) == expected['bytes']
        assert hashlib.sha256(process.stdout).hexdigest() == expected['sha256']
    result = {
        'report': report_pin,
        'plan': plan_pin,
        'verified_unique_precall_pins': len(unique),
        'verified_compiled_git_objects': len(git_pins),
        'attempted': 60,
        'matched': 0,
        'unmet': 60,
        'native_or_jvm_calls': 0,
        'all_candidate_paths_absent': True,
        'error_counts': [
            {'profile': key[0], 'column': key[1], 'error': key[2], 'count': value}
            for key, value in sorted(counts.items())
        ],
    }
    if args.output.exists():
        raise FileExistsError(args.output)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'verified': True, 'unmet': 60, 'native_or_jvm_calls': 0}))


if __name__ == '__main__':
    main()
