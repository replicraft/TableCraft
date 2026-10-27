"""Verify saved combined-pressure sources; never starts a child process."""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from variable_encoded_split_sources import model

DEFAULT_REPORT = '.cache/ace16-variable-encoded-source-first-20261027/report.json'
DEFAULT_FREEZE = '.cache/ace16-variable-encoded-source-next-20261027/freeze.json'


def path(value):
    target = (ROOT / value).resolve()
    target.relative_to(ROOT)
    return target


def pin(value):
    target = path(value)
    data = target.read_bytes()
    return {
        'path': str(target.relative_to(ROOT)),
        'bytes': len(data),
        'sha256': hashlib.sha256(data).hexdigest(),
    }


def load(value):
    return json.loads(path(value).read_text())


def functions(value):
    syntax = ast.parse(path(value).read_text())
    return {
        node.name: ast.dump(node, include_attributes=False)
        for node in syntax.body
        if isinstance(node, ast.FunctionDef)
    }


def verify(report_path, freeze_path):
    report = load(report_path)
    freeze = load(freeze_path)
    assert report['freeze'] == pin(freeze_path)
    assert report['qualified_success'] and report['completed']
    assert report['failure'] is None and not report['created_inventory_errors']
    assert report['frozen_inputs_unchanged'] and report['originals_unchanged']
    assert len(report['commands']) == len(freeze['argv']) == 5
    assert [item['phase'] for item in freeze['argv']] == ['compile', 'create', 'inspect', 'create', 'inspect']
    for expected, command in zip(freeze['argv'], report['commands'], strict=True):
        assert expected['argv'] == command['argv']
        assert command['status'] == 0 and command['stopped_for'] is None
        assert command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == 16 * 1024 * 1024
    assert report['input_verification']['unchanged']
    assert len(report['input_verification']['checks']) == len(freeze['inputs']) + 1
    assert all(item['matches'] for item in report['input_verification']['checks'])
    assert report['original_verification']['unchanged']
    pins = {}

    def check(item):
        previous = pins.setdefault(item['path'], item)
        assert previous == item
        assert pin(item['path']) == item

    for item in freeze['inputs'] + report['created_files'] + [report['freeze']]:
        check(item)
    directory = Path(freeze_path).parent
    archive = directory / 'before-residue-prose-correction'
    for item in load(archive / 'source-pins.json'):
        check(item)
    initial = load(archive / 'freeze.json')
    assert initial['argv'] == freeze['argv']
    changes = []
    for before, after in zip(initial['inputs'], freeze['inputs'], strict=True):
        assert before['path'] == after['path']
        if before != after:
            assert before['path'] == str(directory / 'DESIGN.md')
            preserved = pin(archive / 'DESIGN.md')
            assert (preserved['bytes'], preserved['sha256']) == (before['bytes'], before['sha256'])
            changes.append(before['path'])
    assert changes == [str(directory / 'DESIGN.md')]
    helper = directory / 'VariableEncodedProbe.java'
    assert path(helper).read_bytes() == path('docs/testing/variable_encoded_split_sources/VariableEncodedProbe.java').read_bytes()
    assert functions(directory / 'model.py') == functions('docs/testing/variable_encoded_split_sources/model.py')
    commands = {item['label']: item for item in report['commands']}
    profiles = []
    rows = slots = entries = data_pages = 0
    for profile in ('V2016', 'V2019'):
        database = next(item for item in report['originals'] if item['path'].endswith(profile + '-original.accdb'))
        check(database)
        facts = report['source_checks'][profile]
        check(facts)
        saved = load(facts['path'])
        getter = commands[profile + '-inspect']['stdout']['path']
        actual = model.snapshot(path(database['path']), path(getter), profile)
        assert saved == json.loads(json.dumps(actual))
        rows += sum(table['count'] for table in actual['tables'].values())
        entries += sum(index['count'] for table in actual['tables'].values() for index in table['indexes'].values())
        selected = {}
        for name in model.TARGETS:
            table = actual['tables'][name]
            geometry = table['data_geometry']
            slots += table['count']
            data_pages += len(geometry['pages'])
            selected[name] = {
                'geometry': geometry,
                'hint': actual['maps'][name + ':Hint']['file_members'],
                'capacity': {
                    label: table['indexes'][label]['capacity']
                    for label in ('Ascending', 'Descending')
                },
            }
        profiles.append({
            'profile': profile,
            'database': database,
            'rows': sum(table['count'] for table in actual['tables'].values()),
            'map_roles': len(actual['maps']),
            'targets': selected,
        })
    assert (rows, slots, entries, data_pages) == (5402, 5400, 21602, 60)
    return {
        'verified': True,
        'fixture_processes': 0,
        'source_commands': 5,
        'public_updates': 0,
        'native_calls_in_source_phase': 0,
        'unique_saved_pins': len(pins),
        'pre_execution_correction': (
            'Only private residue-policy prose changed; original draft archived, '
            'no failed fixture attempt.'
        ),
        'typed_rows_rechecked': rows,
        'raw_target_slots_rechecked': slots,
        'logical_index_entries_including_aliases': entries,
        'target_data_pages': data_pages,
        'helper_byte_identity': True,
        'model_function_ast_identity': True,
        'profiles': profiles,
    }


def main():
    if not __debug__:
        raise RuntimeError('Saved verification requires enabled assertions')
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', default=DEFAULT_REPORT)
    parser.add_argument('--freeze', default=DEFAULT_FREEZE)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.report, arguments.freeze), indent=2))


if __name__ == '__main__':
    main()
