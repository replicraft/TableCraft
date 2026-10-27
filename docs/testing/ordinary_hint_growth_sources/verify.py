"""Verify the four saved copied sources; never starts a fixture child."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ordinary_hint_growth_sources import measure, model

SOURCE = '.cache/ace16-ordinary-hint-growth-next-20261027'
FIRST = '.cache/ace16-ordinary-hint-growth-sources-first-20261027'
CONTINUATION = '.cache/ace16-ordinary-hint-growth-continuation-next-20261027'
AFTER = '.cache/ace16-ordinary-hint-growth-continuation-first-20261027'
FIRST_REPORT_SHA = 'd0155f5384342df984c50059348b5b62a401ec7e376d5e779ab95c7ae6ccdbd1'
FIRST_FREEZE_SHA = '5a31ea2ab0d5a8077a656330b08e64aa1b2a97336d3b10ed0b9865d3e959e839'
AFTER_REPORT_SHA = '2c8a13c604630f2a3c6fb9ded650cb63f7dc4aabb771b19f9227507c758ff2c6'
AFTER_FREEZE_SHA = 'd7e8cfa3575b523bbd5813536cfb7872e002a76df54c11bfcd93f6f7d5a4526f'


def path(value):
    result = (ROOT / value).resolve()
    result.relative_to(ROOT)
    return result


def pin(value):
    target = path(value)
    data = target.read_bytes()
    return {'path': target.relative_to(ROOT).as_posix(), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def load(value):
    return json.loads(path(value).read_text())


def normalized(value):
    return json.loads(json.dumps(value))


def verify():
    first_report_path = FIRST + '/report.json'
    first_freeze_path = SOURCE + '/freeze.json'
    after_report_path = AFTER + '/report.json'
    after_freeze_path = CONTINUATION + '/freeze.json'
    for value, expected in [
        (first_report_path, FIRST_REPORT_SHA), (first_freeze_path, FIRST_FREEZE_SHA),
        (after_report_path, AFTER_REPORT_SHA), (after_freeze_path, AFTER_FREEZE_SHA),
    ]:
        assert pin(value)['sha256'] == expected
    first = load(first_report_path)
    freeze = load(first_freeze_path)
    after = load(after_report_path)
    continuation = load(after_freeze_path)
    saved_first = load(CONTINUATION + '/saved-first-check.json')
    assert first['freeze'] == pin(first_freeze_path)
    assert not first['completed'] and not first['qualified_success']
    assert first['failure']['class'] == 'AssertionError'
    assert first['failure']['message'] == 'TextLegacy'
    assert "assert actual['tables'][name] == table" in first['failure']['traceback']
    assert after['freeze'] == pin(after_freeze_path)
    assert after['completed'] and after['qualified_success'] and after['failure'] is None
    assert continuation['original_freeze'] == pin(first_freeze_path)
    assert continuation['first_report'] == pin(first_report_path)
    assert continuation['argv'] == freeze['argv'][3:]
    assert continuation['cases'] == freeze['cases'][1:]
    assert len(first['commands']) == 3 and len(after['commands']) == 6
    assert len(freeze['argv']) == 9 and len(freeze['cases']) == 4
    assert saved_first['qualified_success'] and saved_first['fixture_processes'] == 0
    assert saved_first['inputs_unchanged'] and saved_first['failure'] is None
    assert (saved_first['rows_checked'], saved_first['map_roles_checked'],
            saved_first['original_pages_checked']) == (3085, 28, 36)
    assert after['saved_first_facts'] == continuation['first_facts'] == saved_first['facts']
    assert not after['initial_whole_phase_completed']
    assert after['initial_successful_fixture_commands'] == 3
    assert after['qualified_sources_including_saved_first'] == 4
    assert after['native_calls'] == after['public_update_calls'] == after['compile_calls'] == 0
    assert not first['inventory_errors'] and not after['inventory_errors']

    pins = {}
    occurrences = 0

    def check(item):
        nonlocal occurrences
        occurrences += 1
        prior = pins.setdefault(item['path'], item)
        assert prior == item
        assert pin(item['path']) == item

    for report, plan in [(first, freeze), (after, continuation)]:
        checks = report['input_postcheck']['checks']
        assert report['input_postcheck']['unchanged']
        assert len(checks) == len(plan['inputs']) + 1
        assert all(item['matches'] for item in checks)
        assert report['source_postcheck']['unchanged']
        assert all(item['matches'] for item in report['source_postcheck']['checks'])
        for item in plan['inputs'] + report['created_files'] + [report['freeze']]:
            check(item)
    for value in (first_report_path, after_report_path,
                  CONTINUATION + '/saved-first-check.json',
                  CONTINUATION + '/post-terminal-check.json'):
        check(pin(value))
    postcheck = load(CONTINUATION + '/post-terminal-check.json')
    assert postcheck['unchanged'] and postcheck['fixture_processes_in_postcheck'] == 0
    assert postcheck['raw_report'] == pin(after_report_path)
    for entry in postcheck['checks']:
        assert entry['matches'] and entry['actual'] == entry['expected']
        check(entry['expected'])

    # Preserve the unexecuted coverage-review correction as preparation history.
    archive = SOURCE + '/before-full-page-coverage'
    initial = load(archive + '/freeze.json')
    assert pin(archive + '/freeze.json')['sha256'] == 'b4f538c3c62d9b863063b5a88f9a10afc0805948c8304f8dc971cd962c7ff384'
    assert initial['argv'] == freeze['argv'] and initial['cases'] == freeze['cases']
    assert initial['limits'] == freeze['limits']
    preserved = load(archive + '/source-pins.json')
    assert preserved['static_exit_code'] == 0 and preserved['executed_fixture_processes'] == 0
    for item in preserved['sources']:
        old = pin(archive + '/' + Path(item['path']).name)
        assert (old['bytes'], old['sha256']) == (item['bytes'], item['sha256'])
        check(old)
    before_inputs = {item['path']: item for item in initial['inputs']}
    differences = {item['path'] for item in freeze['inputs'] if before_inputs[item['path']] != item}
    assert differences == {SOURCE + '/measure.py', SOURCE + '/PROTOCOL.md'}

    # The transport repair is exactly one comparison expression. The original
    # already imported json; no values, framing or ownership rules changed.
    old = path(SOURCE + '/measure.py').read_text()
    corrected = path(SOURCE + '/measure_json.py').read_text()
    needle = "assert actual['tables'][name] == table, name"
    replacement = "assert json.loads(json.dumps(actual['tables'][name])) == table, name"
    assert old.count(needle) == 1 and corrected == old.replace(needle, replacement)
    public = 'docs/testing/ordinary_hint_growth_sources/'
    assert path(public + 'OrdinaryHintProbe.java').read_bytes() == path(SOURCE + '/OrdinaryHintProbe.java').read_bytes()
    for maintained, executed in [('model.py', 'model.py'), ('measure.py', 'measure_json.py')]:
        before = path(SOURCE + '/' + executed).read_text()
        assert before.count('parents[2]') == 1
        assert path(public + maintained).read_text() == before.replace('parents[2]', 'parents[3]')

    commands = first['commands'] + after['commands']
    assert len({tuple(command['argv']) for command in commands}) == 9
    for planned, command in zip(freeze['argv'], commands, strict=True):
        assert planned['argv'] == command['argv']
        assert command['status'] == 0 and command['stopped_for'] is None
        assert not command['capture_errors']
        assert command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == 16 * 1024 * 1024
        assert max(command['observed_stream_bytes']) <= 16 * 1024 * 1024
        check(command['stdout'])
        check(command['stderr'])
    assert len(first['compiled_classes']) == 2
    for item in first['compiled_classes']:
        check(item)

    command_index = {command['label']: command for command in commands}
    source_checks = {freeze['cases'][0]['name']: saved_first['facts'], **after['source_checks']}
    rows = roles = target_slots = index_entries = protected_pages = protected_bytes = 0
    sources = []
    for case in freeze['cases']:
        name = case['name']
        database = pin(case['output'])
        check(database)
        check(source_checks[name])
        saved = load(source_checks[name]['path'])
        getter = command_index[name + '-inspect']['stdout']['path']
        actual = model.snapshot(path(case['output']), path(getter), case['profile'], case['padding_rows'])
        assert normalized(actual) == saved['snapshot']
        original = load(case['original_facts']['path'])
        observations = measure.qualify(actual, path(case['output']).read_bytes(), original,
                                       path(case['original']['path']).read_bytes(), case['padding_rows'])
        assert normalized(observations) == saved['observations']
        rows += observations['rows_verified']
        roles += len(actual['maps'])
        target_slots += sum(actual['tables'][name]['count'] for name in model.TARGETS)
        index_entries += sum(index['count'] for table in actual['tables'].values()
                             for index in table['indexes'].values())
        protection = observations['original_page_preservation']
        assert protection['unique_pages'] == 36 and protection['comparison_bytes_checked'] == 147456
        protected_pages += protection['unique_pages']
        protected_bytes += protection['comparison_bytes_checked']
        sources.append({'name': name, 'database': database, 'pages': observations['eof'],
                        'rows': observations['rows_verified'], 'map_roles': len(actual['maps']),
                        'source_only_predictions': observations['target_observations']})
    assert (rows, roles, target_slots, index_entries, protected_pages, protected_bytes) == (
        12972, 112, 10800, 43204, 144, 589824,
    )
    return {
        'verified': True, 'fixture_processes': 0, 'actual_unique_fixture_commands': 9,
        'initial_successful_fixture_commands': 3, 'original_phase_completed': False,
        'saved_first_correction': True, 'continuation_successful_fixture_commands': 6,
        'saved_pin_occurrences': occurrences, 'unique_saved_pins': len(pins),
        'typed_rows_rechecked': rows, 'map_role_instances': roles,
        'raw_target_slots_rechecked': target_slots,
        'logical_index_entries_including_aliases': index_entries,
        'original_full_page_comparisons': protected_pages, 'original_full_bytes_compared': protected_bytes,
        'helper_byte_identity': True, 'maintained_model_changes': 'repository-root composition only',
        'sources': sources,
    }


def main():
    if not __debug__:
        raise RuntimeError('Saved verification requires enabled assertions')
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path)
    arguments = parser.parse_args()
    result = verify()
    encoded = json.dumps(result, indent=2) + '\n'
    if arguments.output is not None:
        with arguments.output.open('x') as output:
            output.write(encoded)
    print(encoded, end='')


if __name__ == '__main__':
    main()
