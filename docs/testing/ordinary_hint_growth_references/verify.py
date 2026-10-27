"""Recheck saved reference25 evidence without database, getter or native calls."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ordinary_hint_growth_references import action_model, reference_proof

PRIVATE = '.cache/ace16-ordinary-hint-reference-next-20261027'
WORK = '.cache/ace16-ordinary-hint-reference-first-20261027'
PUBLIC = 'docs/testing/ordinary_hint_growth_references'
FREEZE_SHA = 'be7063ee7d2ea0a2aaa1448a77f6aa0a483d8c5feb8d716b0843fe5c63a67c94'
UPDATES_SHA = '9feb967e798c7595e00846e9fee691ffdd2abf0da8f7853ee25b93cd5bc1b4ab'
GETTERS_SHA = 'b1a5a1e021d0677bfdaa7b7f6b1a44217fda0f4064c1dec90587b253fd7234e9'


def path(value):
    target = (ROOT / value).resolve()
    target.relative_to(ROOT)
    return target


def pin(value):
    target = path(value)
    data = target.read_bytes()
    return {
        'path': target.relative_to(ROOT).as_posix(),
        'bytes': len(data),
        'sha256': hashlib.sha256(data).hexdigest(),
    }


def load(value):
    return json.loads(path(value).read_text())


def normalized(value):
    return json.loads(json.dumps(value))


def maintained_identity():
    for name in ('OrdinaryHintReferenceProbe.java', 'map_proof.py'):
        assert path(PUBLIC + '/' + name).read_bytes() == path(PRIVATE + '/' + name).read_bytes()
    for name in ('action_model.py', 'reference_proof.py'):
        executed = path(PRIVATE + '/' + name).read_text()
        assert executed.count('parents[2]') == 1
        maintained = executed.replace('parents[2]', 'parents[3]')
        if name == 'reference_proof.py':
            assert maintained.count('import map_proof\n') == 1
            maintained = maintained.replace(
                'import map_proof\n',
                'from ordinary_hint_growth_references import map_proof\n',
            )
        assert path(PUBLIC + '/' + name).read_text() == maintained


def verify():
    freeze_path = PRIVATE + '/freeze.json'
    updates_path = WORK + '/updates/report.json'
    getters_path = WORK + '/inspect/report.json'
    for value, digest in (
        (freeze_path, FREEZE_SHA), (updates_path, UPDATES_SHA), (getters_path, GETTERS_SHA),
    ):
        assert pin(value)['sha256'] == digest
    maintained_identity()
    freeze = load(freeze_path)
    proposal = load(freeze['proposal']['path'])
    updates = load(updates_path)
    getters = load(getters_path)
    assert len(freeze['inputs']) == 115
    assert updates['qualified_success'] and getters['qualified_success']
    assert updates['completed'] and getters['completed']
    assert updates['failure'] is getters['failure'] is None
    assert updates['freeze'] == getters['freeze'] == pin(freeze_path)
    assert getters['updates_report'] == pin(updates_path)
    assert len(updates['commands']) == 13 and len(getters['commands']) == 12
    assert updates['public_update_attempts'] == 12
    assert updates['read_only_getter_attempts'] == 0
    assert getters['public_update_attempts'] == 0
    assert getters['read_only_getter_attempts'] == 12
    assert updates['native_calls'] == getters['native_calls'] == 0
    assert updates['generation_calls'] == getters['generation_calls'] == 0
    assert not updates['inventory_errors'] and not getters['inventory_errors']
    assert updates['input_postcheck']['unchanged'] and updates['output_postcheck']['unchanged']
    assert getters['input_postcheck']['unchanged']
    commands = updates['commands'] + getters['commands']
    assert len({tuple(command['argv']) for command in commands}) == 25
    assert [command['argv'] for command in commands] == [command['argv'] for command in proposal['argv']]
    for command in commands:
        assert command['status'] == 0 and command['stopped_for'] is None
        assert command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == 16777216
        assert all(size <= 16777216 for size in command['observed_stream_bytes'])
    assert len(updates['compiled_classes']) == 2
    assert {Path(item['path']).name for item in updates['compiled_classes']} == {
        'OrdinaryHintReferenceProbe.class', 'BoundedFileChannel.class',
    }

    checked = {}
    occurrences = 0

    def visit(value):
        nonlocal occurrences
        if isinstance(value, dict):
            if {'path', 'bytes', 'sha256'} <= value.keys():
                item = {key: value[key] for key in ('path', 'bytes', 'sha256')}
                occurrences += 1
                previous = checked.setdefault(item['path'], item)
                assert previous == item
                if previous is item:
                    assert pin(item['path']) == item
                return
            for child in value.values():
                visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    for value in (freeze, updates, getters):
        visit(value)
    for value in (
        freeze_path, updates_path, getters_path,
        PRIVATE + '/phase-a-postcheck.json', PRIVATE + '/phase-b-postcheck.json',
        PRIVATE + '/phase-b-binding.json', PRIVATE + '/observations.json',
    ):
        visit(pin(value))
    for value, count in (('phase-a-postcheck.json', 131), ('phase-b-postcheck.json', 168)):
        postcheck = load(PRIVATE + '/' + value)
        assert postcheck['all_unchanged'] and postcheck['fixture_processes'] == 0
        assert len(postcheck['checks']) == count
        for item in postcheck['checks']:
            assert item['actual'] == item['expected']
        visit(postcheck)

    assert len(proposal['cases']) == len(updates['cases']) == len(getters['cases']) == 12
    assert [case['id'] for case in proposal['cases']] == [case['id'] for case in getters['cases']]
    rows = padding_rows = roles = index_entries = original_bytes = 0
    data_pages = index_pages = bitmap_pages = counter_blocks = 0
    cases = []
    for planned, receipt in zip(proposal['cases'], getters['cases'], strict=True):
        assert receipt['completed']
        saved = load(receipt['facts']['path'])
        command = next(command for command in getters['commands'] if command['label'] == planned['id'] + '-inspect')
        before = action_model.snapshot(
            path(planned['source']), path(planned['source_getter']),
            planned['profile'], planned['padding_rows'],
        )
        transported = normalized(before)
        transported['file'] = str(Path(transported['file']).resolve())
        assert transported == load(planned['source_facts'])['snapshot']
        after = action_model.snapshot(
            path(planned['output']), path(command['stdout']['path']),
            planned['profile'], planned['padding_rows'], changed=planned['table'],
        )
        assert normalized(after) == saved['snapshot']
        proof = reference_proof.compare(before, after, planned['table'])
        assert normalized(proof) == saved['proof']
        assert before['tables']['PaddingHint'] == after['tables']['PaddingHint']
        rows += sum(table['count'] for table in after['tables'].values())
        padding_rows += planned['padding_rows']
        roles += len(after['maps'])
        index_entries += sum(index['count'] for table in after['tables'].values() for index in table['indexes'].values())
        original_bytes += before['bytes']
        data_pages += 1
        index_pages += len(proof['added_index_pages'])
        bitmap_pages += len(proof['fresh_map_bitmap_pages'])
        counter_blocks += len(proof['physical_counter_blocks'])
        cases.append({
            'id': planned['id'],
            'source_pages': before['bytes'] // 4096,
            'after_pages': after['bytes'] // 4096,
            'suffix_pages': (after['bytes'] - before['bytes']) // 4096,
            'output': pin(planned['output']),
            'facts': receipt['facts'],
            'hint_transition': proof['map_transitions'][planned['table'] + ':Hint'],
            'suffix': proof['suffix_purposes_in_actual_order'],
        })
    assert (rows, padding_rows, roles, index_entries) == (38916, 6504, 336, 129612)
    assert (original_bytes, data_pages, index_pages, bitmap_pages, counter_blocks) == (
        34603008, 12, 72, 16, 120,
    )
    return {
        'verified': True,
        'fixture_processes': 0,
        'actual_fixture_commands': 25,
        'public_updates': 12,
        'readonly_getters': 12,
        'javac_commands': 1,
        'saved_pin_occurrences': occurrences,
        'unique_saved_pins': len(checked),
        'typed_after_rows_rechecked': rows,
        'padding_rows_preserved': padding_rows,
        'map_role_instances': roles,
        'logical_index_entries_including_aliases': index_entries,
        'original_prefix_bytes_rechecked': original_bytes,
        'new_DATA_pages': data_pages,
        'new_index_leaves': index_pages,
        'new_map_bitmap_pages': bitmap_pages,
        'unchanged_physical_counter_blocks': counter_blocks,
        'maintained_derivatives': 'ROOT/import composition only; helper/map proof byte-identical',
        'cases': cases,
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
