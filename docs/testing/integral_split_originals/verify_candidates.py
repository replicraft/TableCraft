"""Verify retained typed candidates without generation, updates or JVM calls."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from integral_split_originals import action_model, candidate_model
from integral_split_originals.verify_sources import all_pins, check_pin, normalized

NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


def verify(path):
    original_report = path.read_bytes()
    report = json.loads(original_report)
    # Compiler-input identities belong to immutable Git objects. The working
    # tree may already contain the next independent engine change.
    live = {key: value for key, value in report.items() if key != 'compiled_source_pins'}
    live_pins = list(all_pins(live))
    revision = report['binding']['compiled_revision']
    for expected in report['compiled_source_pins']:
        data = subprocess.check_output(['git', 'show', revision + ':' + expected['path']], cwd=ROOT)
        assert len(data) == expected['bytes']
        assert hashlib.sha256(data).hexdigest() == expected['sha256']

    assert report['completed'] == 8 and report['unmet'] == 0
    assert report['inputs_unchanged'] and report['pins_before'] == report['pins_after']
    plan = json.loads(check_pin(report['binding']['plan']).read_text())
    published_proof = Path(candidate_model.__file__).read_bytes()
    assert len(published_proof) == plan['proof']['bytes']
    assert hashlib.sha256(published_proof).hexdigest() == plan['proof']['sha256']
    source_report = json.loads(check_pin(plan['source_report']).read_text())
    profiles = {item['profile']: item for item in source_report['profiles']}
    expected_cases = {item['id']: item for item in plan['cases']}
    commands = report['commands']
    assert len(commands) == 16
    assert all(item['status'] == 0 and item['stopped_for'] is None for item in commands)
    by_label = {item['label']: item for item in commands}
    assert len(by_label) == 16
    observed = set()
    for case in report['cases']:
        identity = case['id']
        assert identity not in observed
        observed.add(identity)
        expected = expected_cases[identity]
        assert case['agrees'] and case['source'] == case['source_after'] == expected['source']
        before = check_pin(case['source'])
        candidate = check_pin(case['candidate'])
        image, facts = candidate_model.expected_image(expected, before.read_bytes())
        assert facts == expected['expected_image'] == case['whole_image_proof']
        assert candidate.read_bytes() == image
        native = by_label[identity + '-native']
        assert native['argv'] == [report['binding']['artifact']['path'], case['source']['path'],
                                  case['candidate']['path'], expected['table'], expected['column'],
                                  str(expected['selected_ordinal']), expected['request_kind'],
                                  str(expected['expected_units']), str(expected['replacement_units'])]
        assert check_pin(native['stdout']).read_text() == 'candidate_changed_rows=1\n'
        assert check_pin(native['stderr']).read_text() == NOTICE
        describe = by_label[identity + '-describe']
        assert describe['argv'][-1] == case['candidate']['path']
        assert check_pin(describe['stderr']).read_bytes() == b''
        facts = action_model.analyze(before, candidate, profiles[expected['profile']],
                                     expected['table'], check_pin(describe['stdout']).read_text())
        assert normalized(facts) == case['typed_index_map_analysis']
    assert observed == set(expected_cases) and len(observed) == 8

    post = json.loads((path.parent / 'postcheck.json').read_text())
    post_pins = list(all_pins(post))
    assert post['terminal_session'] == 99347 and post['exit_status'] == 0
    assert post['completed'] == 8 and post['unmet'] == 0
    assert post['fixture_commands'] == 16 and post['all_fixture_statuses_zero']
    assert post['compiled_git_objects'] == len(report['compiled_source_pins'])
    assert post['raw_report_before'] == post['raw_report_after']
    assert post['raw_report_after']['sha256'] == hashlib.sha256(original_report).hexdigest()
    assert len(post['checks']) == post['input_pin_occurrences']
    assert all(item['matches'] and item['expected'] == item['actual'] for item in post['checks'])
    assert path.read_bytes() == original_report
    return {'read_only': True, 'fixture_processes_repeated': 0, 'historical_fixture_commands': 16,
            'complete_candidate_images': 8, 'typed_getter_comparisons': 8,
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
