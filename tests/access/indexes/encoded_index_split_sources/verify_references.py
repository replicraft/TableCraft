"""Recheck saved public replacements without any child process or database write."""
import argparse
import json

from encoded_index_split_sources import action_model, reference_proof
from encoded_index_split_sources.verify import function_bodies, load, path, pin

DEFAULT_FREEZE = '.cache/ace16-encoded-index-reference-next-20261027/freeze.json'
DEFAULT_REPORT = '.cache/ace16-encoded-index-reference-first-20261027/report.json'


def verify(report_path, freeze_path):
    report = load(report_path)
    freeze = load(freeze_path)
    assert report['qualified_success'] and report['completed']
    assert report['failure'] is None and not report['inventory_errors']
    assert report['inputs_unchanged']
    assert report['freeze'] == pin(freeze_path)
    assert len(report['commands']) == len(freeze['argv']) == 12
    assert len(report['cases']) == len(freeze['cases']) == 6
    assert [command['argv'] for command in report['commands']] == [item['argv'] for item in freeze['argv']]
    for command in report['commands']:
        assert command['status'] == 0 and command['stopped_for'] is None
        assert not command['capture_errors']
        assert command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == 16 * 1024 * 1024
        assert all(size <= 16 * 1024 * 1024 for size in command['observed_stream_bytes'])

    required = freeze['inputs'] + [report['freeze']]
    saved = {}
    for item in required + report['created_files']:
        assert saved.setdefault(item['path'], item) == item
        assert pin(item['path']) == item
    integrity = report['input_checks']
    assert integrity['unchanged'] and len(integrity['checks']) == len(required)
    for expected, check in zip(required, integrity['checks'], strict=True):
        assert check['path'] == expected['path'] and check['unchanged']
        assert check['actual'] == expected

    model_source = next(item for item in freeze['inputs'] if item['path'].endswith('encoded-index-reference-next-20261027/action_model.py'))
    proof_source = next(item for item in freeze['inputs'] if item['path'].endswith('encoded-index-reference-next-20261027/proof.py'))
    assert function_bodies(model_source['path']) == function_bodies('docs/testing/encoded_index_split_sources/action_model.py')
    assert function_bodies(proof_source['path']) == function_bodies('docs/testing/encoded_index_split_sources/reference_proof.py')

    total_rows = 0
    total_entries = 0
    prefix_bytes = 0
    added_pages = 0
    outcomes = []
    for position, (case, actual) in enumerate(zip(freeze['cases'], report['cases'], strict=True)):
        assert actual['id'] == case['id'] and actual['source'] == case['source']
        assert actual['output'] == case['output']
        assert actual['completed'] and actual['output_pin'] == pin(case['output'])
        update, getter = report['commands'][2 * position:2 * position + 2]
        assert update['argv'][-4:] == ['reference', case['source']['path'], case['output'], case['table']]
        assert getter['argv'][-3:] == ['inspect', case['output'], case['table']]
        before = action_model.snapshot(path(case['source']['path']), path(case['source_getter']['path']), case['profile'])
        accepted = load(case['source_gates']['path'])
        assert json.loads(json.dumps(before)) == accepted
        after = action_model.snapshot(path(case['output']), path(getter['stdout']['path']), case['profile'], case['table'])
        certification = reference_proof.compare(before, after, case['table'])
        assert actual['facts'] == pin(actual['facts']['path'])
        assert load(actual['facts']['path']) == json.loads(json.dumps({'snapshot': after, 'proof': certification}))
        rows = sum(table['count'] for table in after['tables'].values())
        entries = sum(index['count'] for table in after['tables'].values() for index in table['indexes'].values())
        total_rows += rows
        total_entries += entries
        prefix_bytes += before['bytes']
        added_pages += len(certification['added_index_pages'])
        outcomes.append({'id': case['id'], 'output': actual['output_pin'], 'proof': certification})
    assert total_rows == 16206 and total_entries == 64806 and added_pages == 36
    assert prefix_bytes == 3563520
    return {
        'verified': True,
        'fixture_processes': 0,
        'saved_pins': len(saved),
        'public_updates': 6,
        'readonly_getters': 6,
        'new_index_pages_observed': added_pages,
        'after_typed_row_comparisons': total_rows,
        'after_logical_index_entry_comparisons_including_aliases': total_entries,
        'original_prefix_bytes_rechecked_across_copies': prefix_bytes,
        'maintained_model_and_proof_function_ast_identity': True,
        'outcomes': outcomes,
    }


def main():
    if not __debug__:
        raise RuntimeError('Saved evidence checks require enabled assertions')
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', default=DEFAULT_REPORT)
    parser.add_argument('--freeze', default=DEFAULT_FREEZE)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.report, arguments.freeze), indent=2))


if __name__ == '__main__':
    main()
