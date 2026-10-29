"""Saved SOURCE gates and all twenty prefrozen predictions, without construction."""
import json
from pathlib import Path

import intent
import reference_bounded as bounded

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / '.cache/ace16-nullable-split-batch-source-next-20261029'
EXPECTED = ROOT / '.cache/ace16-nullable-split-batch-expected-next-20261029'
OUTPUT = ROOT / '.cache/ace16-nullable-split-batch-expected-first-20261029'


def normalized(value):
    return json.loads(json.dumps(value))


def require_source():
    gates_path = SOURCE / 'source-gates-first.json'
    gates = json.loads(gates_path.read_text())
    assert gates['source_qualification'] is True
    assert gates['source_gates'] == 8 and gates['sentinel_gates'] == 4
    assert gates['actual_child_commands'] == gates['constructor_images'] == 0
    for item in gates['pins']:
        bounded.require_pin(item)
    assert len(gates['facts']) == 4
    for saved in gates['facts']:
        bounded.require_pin(saved['source'])
        bounded.require_pin(saved['getter'])
        actual = intent.source.inspect(ROOT / saved['source']['path'],
                                       ROOT / saved['getter']['path'],
                                       saved['profile'], saved['direction'])
        assert normalized(actual) == saved['facts']
    return gates


def require_expected(cases):
    # These files do not exist until the separately frozen constructor succeeds.
    freeze_path = OUTPUT / 'freeze.json'
    report_path = OUTPUT / 'report.json'
    freeze = json.loads(freeze_path.read_text())
    report = json.loads(report_path.read_text())
    constructor_pin = bounded.pin(EXPECTED / 'freeze.json')
    assert freeze['completed'] is True and freeze['derivations'] == 20
    assert report['completed'] is True and report['model_derivations'] == 20
    assert report['source_gates_before_images'] is True
    assert report['actual_child_commands'] == 0
    assert report['input_freeze'] == constructor_pin
    assert freeze['inputs'] == report['inputs'] + [constructor_pin]
    assert bounded.pin(report_path) in freeze['outputs']
    for item in freeze['inputs'] + freeze['outputs']:
        bounded.require_pin(item)
    recipes = json.loads((EXPECTED / 'requests.json').read_text())
    assert recipes['case_count'] == len(recipes['cases']) == 20
    assert recipes['row_updates'] == 8008
    assert len(report['results']) == len(cases) == 20
    for case, request, result in zip(cases, recipes['cases'], report['results'], strict=True):
        for field in ('id', 'source_id', 'profile', 'direction', 'table', 'recipe'):
            assert case[field] == request[field]
        assert request['updates'] == normalized(intent.updates(case))
        assert result['case'] == case['id']
        assert result['expected'] in freeze['outputs']
        assert result['expected']['path'] == str(
            (OUTPUT / (case['id'] + '.accdb')).relative_to(ROOT))
    return freeze, report


def require_plan(plan):
    assert plan['source_gate_receipt'] == str((SOURCE / 'source-gates-first.json').relative_to(ROOT))
    assert plan['expected_input_freeze'] == str((EXPECTED / 'freeze.json').relative_to(ROOT))
    assert plan['expected_output_freeze'] == str((OUTPUT / 'freeze.json').relative_to(ROOT))
    assert plan['expected_report'] == str((OUTPUT / 'report.json').relative_to(ROOT))
    gates = require_source()
    expected, _ = require_expected(plan['cases'])
    assert plan['phase'] == 'nullable-split-batch-public-reference40'
    assert plan['reference_children'] == len(plan['commands']) == 40
    assert plan['transitions'] == len(plan['cases']) == 20
    assert plan['ordered_public_update_calls'] == 8008
    assert plan['source_gates'] == 8 and plan['sentinel_gates'] == 4
    assert plan['limits'] == dict(file_bytes=8388608, stdout_bytes=16777216,
                                  stderr_bytes=16777216, heap_bytes=536870912, child_seconds=30)
    originals = {saved['id']: saved for saved in gates['facts']}
    order = [('Items', 'Full'), ('Items', 'Clear'),
             ('UniqueItems', 'Full'), ('UniqueItems', 'Clear'), ('Items', 'DuplicateFamily')]
    source_order = [layout + '-' + direction for layout in ('V2016', 'V2019')
                    for direction in ('ASC', 'DESC')]
    work = '.cache/ace16-nullable-split-batch-reference-first-20261029'
    source_classes = '.cache/ace16-nullable-split-batch-source-first-20261029/classes'
    action_classes = '.cache/ace16-nullable-split-batch-action-compile-first-20261029/classes'
    jar = '.cache/tablecraft-fixtures/jackcess-5.0.4.jar'
    prefix = ['java', '-Xmx512m', '-Duser.timezone=UTC', '-Duser.language=en',
              '-Duser.country=US',
              '-Djava.util.logging.SimpleFormatter.format=%4$s\t%3$s\t%5$s%6$s%n', '-cp']
    commands = []
    for position, case in enumerate(plan['cases']):
        saved = originals[source_order[position // 5]]
        assert (case['table'], case['recipe']) == order[position % 5]
        assert case['source_id'] == saved['id']
        assert case['profile'] == saved['profile'] and case['direction'] == saved['direction']
        assert case['source'] == saved['source']['path']
        assert case['before_getter'] == saved['getter']['path']
        assert case['id'] == saved['id'] + '-' + case['table'] + '-' + case['recipe']
        assert case['output'] == work + '/' + case['id'] + '-reference.accdb'
        assert case['updates'] == (2 if case['recipe'] == 'DuplicateFamily' else 500)
        assert case['warning_records'] == (200 if case['recipe'] == 'Clear' else 0)
        action = prefix + [jar + ':' + source_classes + ':' + action_classes,
                           'NullableSplitBatchAction', 'action', case['source'], case['output'],
                           case['profile'], case['direction'], case['table'], case['recipe']]
        inspect = prefix + [jar + ':' + source_classes, 'NullableSplitBatchFixture',
                            'inspect', case['output'], case['profile']]
        commands.extend([dict(id=case['id'] + '-action', argv=action),
                         dict(id=case['id'] + '-inspect', argv=inspect)])
    assert plan['commands'] == commands
    return gates, expected
