"""Verify saved encoded source observations without starting any child process."""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from encoded_index_split_sources import model

DEFAULT_FAILED = '.cache/ace16-encoded-index-source-first-20261027/report.json'
DEFAULT_REPORT = '.cache/ace16-encoded-index-source-corrected-first-20261027/report.json'
DEFAULT_FREEZE = '.cache/ace16-encoded-index-source-corrected-next-20261027/freeze.json'


def path(value):
    target = (ROOT / value).resolve()
    target.relative_to(ROOT)
    return target


def pin(value):
    target = path(value)
    raw = target.read_bytes()
    return {
        'path': str(target.relative_to(ROOT)),
        'bytes': len(raw),
        'sha256': hashlib.sha256(raw).hexdigest(),
    }


def load(value):
    return json.loads(path(value).read_text())


def function_bodies(value):
    syntax = ast.parse(path(value).read_text())
    return {
        node.name: ast.dump(node, include_attributes=False)
        for node in syntax.body
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    }


def verify(failed_path, report_path, freeze_path):
    failed = load(failed_path)
    report = load(report_path)
    freeze = load(freeze_path)
    assert failed['qualified_success'] is False
    assert len(failed['commands']) == 1
    assert failed['commands'][0]['status'] == 1
    assert not failed['originals'] and not failed['source_checks']
    assert len(report['commands']) == 5 and report['qualified_success']
    assert report['frozen_inputs_unchanged'] and report['originals_unchanged']
    assert not report['created_inventory_errors']
    assert report['freeze'] == pin(freeze_path)
    assert len(freeze['argv']) == 5
    assert [command['argv'] for command in report['commands']] == [item['argv'] for item in freeze['argv']]
    assert all(command['status'] == 0 and command['stopped_for'] is None for command in report['commands'])
    assert all(command['timeout_seconds'] == 30 and command['stream_limit_bytes'] == 16 * 1024 * 1024 for command in report['commands'])

    pins = {}
    required = freeze['inputs'] + report['created_files'] + [report['freeze']]
    required += failed['created_files'] + [failed['freeze']]
    for item in required:
        previous = pins.setdefault(item['path'], item)
        assert previous == item
        assert pin(item['path']) == item
    executed_helper = next(item for item in freeze['inputs'] if item['path'].endswith('source-corrected-next-20261027/EncodedSplitProbe.java'))
    assert path(executed_helper['path']).read_bytes() == path('docs/testing/encoded_index_split_sources/EncodedSplitProbe.java').read_bytes()
    original_model = next(item for item in freeze['inputs'] if item['path'].endswith('encoded-index-split-next-20261027/model.py'))
    assert function_bodies(original_model['path']) == function_bodies('docs/testing/encoded_index_split_sources/model.py')

    source_command = {item['label']: item for item in report['commands']}
    qualified = []
    rows = 0
    slots = 0
    stream_entries = 0
    for profile in ('V2016', 'V2019'):
        saved = report['source_checks'][profile]
        facts = load(saved['path'])
        database = next(item for item in report['originals'] if item['path'].endswith(profile + '-original.accdb'))
        assert pin(database['path']) == database
        command = source_command[profile + '-inspect']
        expected = model.snapshot(path(database['path']), path(command['stdout']['path']), profile)
        expected = json.loads(json.dumps(expected))
        assert facts == expected
        assert saved == pin(saved['path'])
        rows += sum(table['count'] for table in facts['tables'].values())
        slots += sum(table['count'] for name, table in facts['tables'].items() if name != 'LongSentinel')
        stream_entries += sum(index['count'] for table in facts['tables'].values() for index in table['indexes'].values())
        capacities = {
            name: {
                label: table['indexes'][label]['capacity']
                for label in ('Ascending', 'Descending')
            }
            for name, table in facts['tables'].items()
            if name != 'LongSentinel'
        }
        qualified.append({
            'profile': profile,
            'database': database,
            'rows': sum(table['count'] for table in facts['tables'].values()),
            'map_roles': len(facts['maps']),
            'capacity': capacities,
        })
    assert rows == 5402 and slots == 5400 and stream_entries == 21602
    return {
        'verified': True,
        'fixture_processes': 0,
        'unique_saved_pins': len(pins),
        'failed_compile_commands': 1,
        'corrected_source_commands': 5,
        'public_updates': 0,
        'native_baseline_calls': 0,
        'typed_rows_rechecked': rows,
        'selected_variable_slots_rechecked': slots,
        'logical_index_entries_rechecked_including_aliases': stream_entries,
        'profiles': qualified,
        'helper_byte_identity': True,
        'model_function_ast_identity': True,
    }


def main():
    if not __debug__:
        raise RuntimeError('Saved evidence verification requires enabled assertions')
    parser = argparse.ArgumentParser()
    parser.add_argument('--failed-report', default=DEFAULT_FAILED)
    parser.add_argument('--report', default=DEFAULT_REPORT)
    parser.add_argument('--freeze', default=DEFAULT_FREEZE)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.failed_report, arguments.report, arguments.freeze), indent=2))


if __name__ == '__main__':
    main()
