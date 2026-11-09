"""Recheck saved mixed-map candidates without native, JVM or database writes."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from mixed_lval_originals import candidate_facts, proof

NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


def check_pin(expected):
    path = (ROOT / expected['path']).resolve()
    path.relative_to(ROOT)
    data = path.read_bytes()
    assert len(data) == expected['bytes']
    assert hashlib.sha256(data).hexdigest() == expected['sha256']
    return path


def all_pins(value):
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            check_pin(value)
            yield value['path']
        for child in value.values():
            yield from all_pins(child)
    elif isinstance(value, list):
        for child in value:
            yield from all_pins(child)


def normalized(value):
    return json.loads(json.dumps(value))


def verify(path):
    original_report = path.read_bytes()
    report = json.loads(original_report)
    live = {key: value for key, value in report.items() if key != 'compiled_source_pins'}
    live_pins = list(all_pins(live))
    revision = report['binding']['compiled_revision']
    for expected in report['compiled_source_pins']:
        data = subprocess.check_output(['git', 'show', revision + ':' + expected['path']], cwd=ROOT)
        assert len(data) == expected['bytes']
        assert hashlib.sha256(data).hexdigest() == expected['sha256']
    assert report['completed'] == 12 and report['unmet'] == 0
    assert report['inputs_unchanged'] and report['pins_before'] == report['pins_after']
    plan = json.loads(check_pin(report['binding']['plan']).read_text())
    source_report = json.loads(check_pin(plan['source_report']).read_text())
    raw_model = proof.RAW_MODEL.read_bytes()
    assert len(raw_model) == plan['raw_model']['bytes']
    assert hashlib.sha256(raw_model).hexdigest() == plan['raw_model']['sha256']
    prescribed = {item['label']: item['argv'] for item in report['binding']['process_inventory']}
    commands = report['commands']
    assert len(commands) == len(prescribed) == 25
    by_label = {item['label']: item for item in commands}
    assert len(by_label) == 25
    for command in commands:
        assert command['argv'] == prescribed[command['label']]
        assert command['status'] == 0 and command['stopped_for'] is None

    planned = {case['id']: case for case in plan['cases']}
    observed = set()
    growth = 0
    public_nulls = 0
    forwarded = 0
    for case in report['cases']:
        identity = case['id']
        assert identity not in observed
        observed.add(identity)
        expected = planned[identity]
        assert case['agrees'] and case['source'] == case['source_after'] == expected['source']
        assert case['request'] == expected['request']
        before = check_pin(case['source'])
        candidate = check_pin(case['candidate'])
        prior = source_report['profiles'][expected['profile']]['before']
        image, facts = proof.expected_image(expected, before.read_bytes(), prior)
        assert facts == expected['expected_image'] == case['whole_image_proof']
        assert candidate.read_bytes() == image
        growth += len(image) - len(before.read_bytes())
        native = by_label[identity + '-native']
        assert native['argv'] == [report['binding']['artifact']['path'], case['source']['path'],
                                  case['candidate']['path'], expected['table'], expected['column'],
                                  case['request']['path']]
        changed = 0 if expected['label'] == 'noop' else 1
        assert check_pin(native['stdout']).read_text() == f'candidate_changed_rows={changed}\n'
        assert check_pin(native['stderr']).read_text() == NOTICE
        inspect = by_label[identity + '-inspect']
        assert inspect['argv'][-1] == case['candidate']['path']
        assert check_pin(inspect['stderr']).read_bytes() == b''
        actual = candidate_facts.snapshot(candidate, check_pin(inspect['stdout']))
        assert normalized(actual) == case['fresh_snapshot']
        comparison = candidate_facts.compare(prior, actual, expected)
        assert comparison == case['fresh_getter_map_comparison']
        public_nulls += comparison['public_null']
        forwarded += facts.get('parent') == 'forwarded'
    assert observed == set(planned) and len(observed) == 12
    assert growth == 32768 and public_nulls == forwarded == 2

    post = json.loads((path.parent / 'postcheck.json').read_text())
    post_pins = list(all_pins(post))
    assert post['terminal_session'] == 21544 and post['exit_status'] == 0
    assert post['completed'] == 12 and post['unmet'] == 0
    assert post['fixture_commands'] == 25 and post['all_fixture_statuses_zero']
    assert post['compiled_git_objects'] == len(report['compiled_source_pins'])
    assert post['raw_report_before'] == post['raw_report_after']
    assert post['raw_report_after']['sha256'] == hashlib.sha256(original_report).hexdigest()
    assert len(post['checks']) == post['input_pin_occurrences']
    assert all(item['matches'] and item['expected'] == item['actual'] for item in post['checks'])
    assert path.read_bytes() == original_report
    return {'read_only': True, 'fixture_processes_repeated': 0, 'historical_fixture_commands': 25,
            'complete_images': 12, 'fresh_getter_map_comparisons': 12, 'public_nulls': public_nulls,
            'forwarded_inline_cases': forwarded, 'total_growth_bytes': growth,
            'compiled_git_objects': len(report['compiled_source_pins']),
            'live_pin_occurrences': len(live_pins), 'postcheck_pin_occurrences': len(post_pins),
            'raw_report_unchanged': True}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    path = (ROOT / args.report).resolve()
    path.relative_to(ROOT)
    print(json.dumps(verify(path), indent=2))


if __name__ == '__main__':
    main()
