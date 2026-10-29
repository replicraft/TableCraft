"""Native admitted-row transition proof, from original SOURCE and assignments."""
import copy
import hashlib
import json
from pathlib import Path

from ace16_fixed_ignore_null_updates import source

PAGE = 4096
MAX_WARNING_RECORDS = 3
MAX_WARNING_LINE_BYTES = 2048
LOGGER = 'com.healthmarketscience.jackcess.Index'


def key(row, columns):
    values = tuple(row[name] for name, ascending in columns)
    if all(value is None for value in values):
        return None
    return b''.join(source.component(row[name], ascending) for name, ascending in columns)


def ordered_intent(before_table, action):
    rows = copy.deepcopy(before_table['values'])
    index = before_table['indexes']['ValueOrder']
    columns = index['columns']
    history = index['history']
    warnings = []
    steps = []
    refusal = None
    for identity, changes in action['updates']:
        original = rows[identity]
        old_key = key(original, columns)
        updated = original | changes
        new_key = key(updated, columns)
        family_elsewhere = {key(row, columns) for other, row in rows.items()
                            if other != identity and key(row, columns) is not None}
        any_null = any(updated[name] is None for name, ascending in columns)
        # This is the pinned public writer ordering, not TableCraft's Required law.
        if new_key is not None and any_null and index['flags'] & 8:
            refusal = 'required-partial-null'
        elif new_key is not None and index['flags'] & 1 and new_key in family_elsewhere:
            refusal = 'unique-represented-duplicate'
        if refusal is not None:
            assert len(action['updates']) == 1
            steps.append(dict(identity=identity, outcome=refusal))
            break
        if new_key is None:
            root = before_table['rows'][identity]
            warnings.append(dict(identity=identity, page=root['page'], slot=root['slot'],
                                 old_key=None if old_key is None else old_key.hex()))
        elif old_key is None and new_key not in family_elsewhere:
            history += 1
        rows[identity] = updated
        steps.append(dict(identity=identity, history=history,
                          old_key=None if old_key is None else old_key.hex(),
                          new_key=None if new_key is None else new_key.hex()))
    expected_refusal = action['public_outcome'] == 'ConstraintViolationException'
    assert expected_refusal == (refusal is not None)
    assert len(warnings) == action['expected_warning_records'] <= MAX_WARNING_RECORDS
    return dict(rows=rows, history=history, warnings=warnings, steps=steps, refusal=refusal)


def entry_text(warning):
    if warning['old_key'] is None:
        return 'null'
    payload = bytes.fromhex(warning['old_key'])
    assert 0 < len(payload) <= 10
    text = ''
    for position, byte in enumerate(payload):
        if position:
            text += '  ' if position % 4 == 0 else ' '
        text += f'{byte:02X}'
    return ('Entry[rowId=' + str(warning['page']) + ':' + str(warning['slot'])
            + ',bytes=(' + str(len(payload)) + ') ' + text + ']')


def check_action_streams(stdout, stderr, action, output, intent):
    expected = ('ACTION\t' + action['id'] + '\t' + action['public_outcome'] + '\n').encode()
    assert stdout == expected
    assert len(stderr) <= 16777216
    lines = stderr.splitlines(keepends=True)
    assert len(lines) == len(intent['warnings'])
    for line, warning in zip(lines, intent['warnings'], strict=True):
        assert len(line) <= MAX_WARNING_LINE_BYTES
        message = ('WARNING\t' + LOGGER + '\tAdded duplicate index entry ' + entry_text(warning)
                   + ' (Db=' + Path(output).name + ';Table=' + action['table']
                   + ';Index=[ValueOrder, ValueAlias])\n')
        assert line == message.encode(), 'Unexpected preserved action warning'


def state(raw, getter):
    tables, descriptions, properties, absent, counters = source.parse(getter)
    maps = {}
    for role, item in descriptions.items():
        assert item['all_members'] == sorted(set(item['all_members']))
        item['file_members'] = [number for number in item['all_members'] if number < len(raw) // PAGE]
        maps[role] = source.native.map_wire.decode_map(raw, item, PAGE, len(raw) // PAGE)
    declarations = {name: source.native.declarations.declarations(raw, table)
                    for name, table in tables.items()}
    assert all(item['complete'] for item in declarations.values())
    claims = source.native.declarations.claims(raw, tables, declarations, maps,
                                              {name: table['indexes'] for name, table in tables.items()})
    return dict(tables=tables, maps=maps, properties=properties, absent=absent,
                counters=counters, declarations=declarations, claims=claims)


def compare(before_path, before_getter, output, getter, profile, action, action_stdout, action_stderr):
    # The full original gate is independent of any AFTER observation.
    source.inspect(before_path, before_getter, profile)
    before = Path(before_path).read_bytes()
    after = Path(output).read_bytes()
    assert len(after) == len(before)
    first = state(before, Path(before_getter).read_text())
    final = state(after, Path(getter).read_text())
    assert first.keys() == final.keys()
    for name in ('maps', 'properties', 'absent', 'counters', 'claims'):
        assert first[name] == final[name], name
    assert first['tables'].keys() == final['tables'].keys()
    selected = first['tables'][action['table']]
    intent = ordered_intent(selected, action)
    assert first['declarations'].keys() == final['declarations'].keys()
    for table_name, original_declaration in first['declarations'].items():
        current_declaration = final['declarations'][table_name]
        if table_name != action['table']:
            assert current_declaration == original_declaration
            continue
        # The declaration service's digest covers the complete table definition,
        # including the historical counter. Prove that entire definition against
        # the exact predicted counter before exempting this redundant digest.
        definition = selected['definition']
        original_definition, original_pages = source.native.declarations.definition(before, definition)
        current_definition, current_pages = source.native.declarations.definition(after, definition)
        assert current_pages == original_pages == [definition]
        predicted_definition = bytearray(original_definition)
        counter = selected['indexes']['ValueOrder']['history_offset']
        predicted_definition[counter:counter + 4] = intent['history'].to_bytes(4, 'little')
        assert current_definition == bytes(predicted_definition)
        assert original_declaration.keys() == current_declaration.keys()
        assert all(original_declaration[field] == current_declaration[field]
                   for field in original_declaration if field != 'definition_sha256')
    assert action['engineering_policy'] == 'supported' and intent['refusal'] is None
    changed = {identity for identity, changes in action['updates']
               if any(selected['values'][identity][name] != value for name, value in changes.items())}
    expected_stdout = f'candidate_changed_rows={len(changed)}\n'.encode()
    assert Path(action_stdout).read_bytes() == expected_stdout
    notice = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
    batch_notice = b'Engineering candidate: delete actions, data-macro semantics and native Access qualification pending.\n'
    assignment_count = sum(len(changes) for identity, changes in action['updates'])
    expected_stderr = notice if assignment_count == 1 else batch_notice
    assert Path(action_stderr).read_bytes() == expected_stderr
    allowed = set()
    changed_columns = {}
    if intent['refusal'] is None:
        for identity, changes in action['updates']:
            changed_columns.setdefault(identity, set()).update(changes)
    for name, original in first['tables'].items():
        actual = final['tables'][name]
        assert original.keys() == actual.keys()
        for field in ('definition', 'count', 'columns_count', 'max_columns', 'columns'):
            assert actual[field] == original[field]
        assert actual['rows'].keys() == original['rows'].keys()
        expected_values = intent['rows'] if name == action['table'] else original['values']
        assert actual['values'] == expected_values
        assert actual['indexes'].keys() == original['indexes'].keys()
        for index_name, original_index in original['indexes'].items():
            actual_index = actual['indexes'][index_name]
            target = name == action['table'] and index_name in ('ValueOrder', 'ValueAlias')
            for field in original_index:
                if field not in ('entries', 'history', 'rows'):
                    assert actual_index[field] == original_index[field]
            history = intent['history'] if target else original_index['history']
            assert actual_index['history'] == history
            # Every user stream is checked against complete final logical ROW associations.
            if name in json.loads((source.HERE / 'plan.json').read_text())['tables']:
                columns = original_index['columns']
                eligible = []
                for identity, row in expected_values.items():
                    wire = key(row, columns)
                    if wire is None and actual_index['flags'] & 2:
                        continue
                    assert wire is not None
                    reported = actual['rows'][identity]
                    root = reported['page'] << 8 | reported['slot']
                    eligible.append((wire, root, identity))
                eligible.sort()
                entries, decoded = source.leaf(after, actual_index['root'], actual['definition'], columns)
                assert entries == [wire + root.to_bytes(4, 'big') for wire, root, identity in eligible]
                assert actual_index['entries'] == len(eligible)
                assert actual_index['rows'] == [(identity, root >> 8, root & 255)
                                               for wire, root, identity in eligible]
            else:
                assert actual_index == original_index
            if target:
                begin = original_index['root'] * PAGE
                allowed.update(range(begin, begin + PAGE))
                definition, definition_pages = source.native.declarations.definition(before, original['definition'])
                assert definition_pages == [original['definition']]
                offset = original['definition'] * PAGE + original_index['history_offset']
                allowed.update(range(offset, offset + 4))
                assert int.from_bytes(after[offset:offset + 4], 'little') == history
        if name not in json.loads((source.HERE / 'plan.json').read_text())['tables'] and name != 'Sentinel':
            # System table user rows are not materialized by the inert inspector.
            assert original['rows'] == actual['rows'] == {}
            continue
        live = source.records.live_records(after, actual, final['maps'])
        by_root = {item['page'] << 8 | item['slot']: identity for identity, item in actual['rows'].items()}
        assert len(live) == len(by_root)
        for root, body in live:
            identity = by_root[root]
            old = original['rows'][identity]
            current = actual['rows'][identity]
            assert all(old[field] == current[field] for field in ('page', 'slot', 'target_page', 'target_slot'))
            assert body == current['raw'] and len(body) == len(old['raw'])
            values = {item['name']: source.records.scalar_value(body, item)
                      for item in final['declarations'][name]['columns']}
            assert values == expected_values[identity]
            changes = changed_columns.get(identity, set()) if name == action['table'] else set()
            if not changes:
                assert body == old['raw']
                continue
            assert source.native.number(body, 0) == 4 and len(body) == 19
            expected_mask = sum(1 << column['number'] for column in final['declarations'][name]['columns']
                                if expected_values[identity][column['name']] is not None)
            assert body[-1] == expected_mask
            page = root >> 8
            slot = root & 255
            begin = page * PAGE + (source.native.number(after, page * PAGE + 14 + 2 * slot) & 0x1fff)
            permitted = {18}
            for column in final['declarations'][name]['columns']:
                if column['name'] in changes:
                    offset = 2 + source.native.number(bytes.fromhex(column['raw_hex']), 21)
                    permitted.update(range(offset, offset + 4))
            assert all(left == right for offset, (left, right) in enumerate(zip(old['raw'], body, strict=True))
                       if offset not in permitted)
            allowed.update(begin + offset for offset in permitted)
    differences = [offset for offset, (left, right) in enumerate(zip(before, after, strict=True)) if left != right]
    assert all(offset in allowed for offset in differences), 'Unrelated source bytes changed'
    return dict(profile=profile, action=action['id'], public_outcome=action['public_outcome'],
                engineering_policy=action['engineering_policy'], refusal=intent['refusal'],
                original_history=selected['indexes']['ValueOrder']['history'], final_history=intent['history'],
                ordered_steps=intent['steps'], warning_records=0,
                differing_bytes=len(differences), output_sha256=hashlib.sha256(after).hexdigest())
