"""Public saved observations; placement is never engineering-model authority."""
import copy
import hashlib
from pathlib import Path

import intent

source = intent.source
PAGE = 4096
USERS = ('Items', 'UniqueItems', 'Sentinel')


def state(raw, getter):
    tables, descriptions, properties, absent, counters = source.previous.parse(getter)
    maps = {}
    for role, item in descriptions.items():
        assert item['all_members'] == sorted(set(item['all_members']))
        item['file_members'] = [number for number in item['all_members'] if number < len(raw) // PAGE]
        maps[role] = source.native.map_wire.decode_map(raw, item, PAGE, len(raw) // PAGE)
    declarations = {name: source.native.declarations.declarations(raw, table)
                    for name, table in tables.items()}
    assert all(item['complete'] for item in declarations.values())
    claims = source.native.declarations.claims(
        raw, tables, declarations, maps,
        {name: table['indexes'] for name, table in tables.items()})
    return dict(tables=tables, maps=maps, properties=properties, absent=absent,
                counters=counters, declarations=declarations, claims=claims)


def tree(raw, root, owner, columns, expected_count):
    nodes = {}
    levels = {}
    leaf_entries = []

    def order(entry):
        return entry[:-4], entry[-4:]

    def visit(number, expected_height=None):
        assert number not in nodes and 0 < number < len(raw) // PAGE
        block = raw[number * PAGE:(number + 1) * PAGE]
        assert block[0] in (3, 4) and block[1] == 1
        assert source.native.number(block, 4, 4) == owner
        height = block[26]
        assert height < 16 and (expected_height is None or height == expected_height)
        assert (block[0] == 4) == (height == 0)
        assert not block[27] & 1 and not block[479] & 0xfe
        prefix = source.native.number(block, 24)
        assert prefix <= 5 * len(columns) + 4
        ends = [position for position in range(1, 3617)
                if block[27 + position // 8] & (1 << (position % 8))]
        active = 3616 - source.native.number(block, 2)
        assert (ends[-1] if ends else 0) == active
        entries = []
        children = []
        start = 0
        for end in ends:
            assert start < end <= 3616
            payload = block[480 + start:480 + end]
            if height:
                assert len(payload) >= 4
                children.append(int.from_bytes(payload[-4:], 'big'))
                payload = payload[:-4]
            complete = payload if not entries else entries[-1][:prefix] + payload
            assert not entries or prefix <= len(entries[-1])
            source.previous.decode_entry(complete, columns)
            assert not entries or order(entries[-1]) < order(complete)
            entries.append(complete)
            start = end
        tail = source.native.number(block, 20, 4)
        nodes[number] = dict(height=height, prefix=prefix, active=active,
                             previous=source.native.number(block, 12, 4),
                             next=source.native.number(block, 16, 4), tail=tail)
        levels.setdefault(height, []).append(number)
        if not height:
            assert not tail
            leaf_entries.extend(entries)
            assert len(leaf_entries) <= expected_count
            return entries
        if tail:
            children.append(tail)
        assert children and len(children) == len(entries) + int(bool(tail))
        flattened = []
        for position, child in enumerate(children):
            values = visit(child, height - 1)
            assert values
            if position < len(entries):
                assert entries[position] == values[-1]
            flattened.extend(values)
        assert flattened == sorted(flattened, key=order)
        return flattened

    entries = visit(root)
    assert len(entries) == expected_count
    for sequence in levels.values():
        for position, number in enumerate(sequence):
            assert nodes[number]['previous'] == (sequence[position - 1] if position else 0)
            assert nodes[number]['next'] == (sequence[position + 1] if position + 1 < len(sequence) else 0)
    return nodes, entries


def compare(case, getter, action_stdout, action_stderr):
    before_path = source.ROOT / case['source']
    before_getter = source.ROOT / case['before_getter']
    output_path = source.ROOT / case['output']
    source.inspect(before_path, before_getter, case['profile'], case['direction'])
    before = before_path.read_bytes()
    after = output_path.read_bytes()
    assert len(before) <= len(after) <= 8 * 1024 * 1024 and len(after) % PAGE == 0
    first = state(before, before_getter.read_text())
    final = state(after, Path(getter).read_text())
    assert first['tables'].keys() == final['tables'].keys()
    for name in ('properties', 'absent', 'counters', 'claims'):
        assert first[name] == final[name], name
    assert first['maps'].keys() == final['maps'].keys()
    original = first['tables'][case['table']]
    observed = intent.derive(original, case)
    intent.check_streams(Path(action_stdout).read_bytes(), Path(action_stderr).read_bytes(),
                         case, observed)
    allowed = bytearray(len(before))

    def allow(start, end):
        assert 0 <= start <= end <= len(before)
        allowed[start:end] = bytes([1]) * (end - start)

    # The entire selected definition is exact except explicitly predicted histories.
    definition, pages = source.native.declarations.definition(before, original['definition'])
    assert pages == [original['definition']]
    expected_definition = bytearray(definition)
    for index_name in ('LeftOrder', 'PairOrder'):
        offset = original['indexes'][index_name]['history_offset']
        expected_definition[offset:offset + 4] = observed['histories'][index_name].to_bytes(4, 'little')
        allow(original['definition'] * PAGE + offset, original['definition'] * PAGE + offset + 4)
    actual_definition, actual_pages = source.native.declarations.definition(after, original['definition'])
    assert actual_pages == pages and actual_definition == expected_definition
    expected_declarations = copy.deepcopy(first['declarations'])
    expected_declarations[case['table']]['definition_sha256'] = hashlib.sha256(expected_definition).hexdigest()
    assert final['declarations'] == expected_declarations

    all_physical = {}
    selected_physical = {}
    null_payload_observations = []
    assigned = dict(intent.updates(case))
    for name, old_table in first['tables'].items():
        table = final['tables'][name]
        for field in ('definition', 'count', 'columns_count', 'max_columns', 'columns'):
            assert table[field] == old_table[field]
        assert table['rows'].keys() == old_table['rows'].keys()
        values = observed['rows'] if name == case['table'] else old_table['values']
        assert table['values'] == values
        if name in USERS:
            live = source.records.live_records(after, table, final['maps'])
            by_root = {row['page'] << 8 | row['slot']: identity for identity, row in table['rows'].items()}
            assert set(by_root) == {root for root, _ in live}
            for root, body in live:
                identity = by_root[root]
                old = old_table['rows'][identity]
                current = table['rows'][identity]
                for field in ('page', 'slot', 'target_page', 'target_slot'):
                    assert old[field] == current[field]
                assert body == current['raw'] and len(body) == len(old['raw'])
                frame, chain = source.native.long_wire.final_frame(after, current)
                assert len(chain) == 1 and not frame['word'] & 0xe000
                expected_body = bytearray(old['raw'])
                if name == case['table'] and identity in assigned:
                    for column, value in assigned[identity].items():
                        position = table['columns'][column]['position']
                        offset = 2 + position * 4
                        if value is None:
                            expected_body[-1] &= ~(1 << position)
                            expected_body[offset:offset + 4] = body[offset:offset + 4]
                            null_payload_observations.append(dict(identity=identity, column=column,
                                before=old['raw'][offset:offset + 4].hex(),
                                after=body[offset:offset + 4].hex()))
                        else:
                            expected_body[-1] |= 1 << position
                            expected_body[offset:offset + 4] = value.to_bytes(4, 'little', signed=True)
                        allow(frame['start'] + offset, frame['start'] + offset + 4)
                    allow(frame['end'] - 1, frame['end'])
                assert body == expected_body
                decoded = {item['name']: source.records.scalar_value(body, item)
                           for item in final['declarations'][name]['columns']}
                assert decoded == values[identity]
        else:
            assert old_table['rows'] == table['rows'] == {}

        assert table['indexes'].keys() == old_table['indexes'].keys()
        for index_name, old_index in old_table['indexes'].items():
            index = table['indexes'][index_name]
            target = name == case['table']
            for field in old_index:
                if field not in ('history', 'entries', 'rows'):
                    assert index[field] == old_index[field]
            if name not in USERS:
                assert index == old_index
                continue
            columns = index['columns']
            eligible = []
            for identity, row in values.items():
                key = intent.key(row, columns, bool(index['flags'] & 2))
                if key is not None:
                    location = table['rows'][identity]
                    root = location['page'] << 8 | location['slot']
                    eligible.append((key, root, identity))
            eligible.sort(key=lambda item: (item[0], item[1]))
            nodes, entries = tree(after, index['root'], table['definition'], columns, len(eligible))
            assert entries == [key + root.to_bytes(4, 'big') for key, root, _ in eligible]
            assert index['entries'] == len(eligible)
            assert index['rows'] == [(identity, root >> 8, root & 255) for _, root, identity in eligible]
            family = 'PairOrder' if index_name.startswith('Pair') else 'LeftOrder'
            assert index['history'] == (observed['histories'][family] if target else old_index['history'])
            physical = (name, index['physical'])
            if physical in all_physical:
                assert all_physical[physical] == set(nodes)
            else:
                all_physical[physical] = set(nodes)
                if target:
                    selected_physical[physical] = nodes
            if target:
                assert index['root'] * PAGE < len(before)
                allow(index['root'] * PAGE, (index['root'] + 1) * PAGE)

    owner = 'Table:' + str(original['definition'])
    changed_map_roles = {'GlobalFree'} | {owner + ':IndexData:' + str(number) for number in (0, 1)}
    old_eof, new_eof = len(before) // PAGE, len(after) // PAGE
    reached = set()
    for (name, number), members in all_physical.items():
        role = 'Table:' + str(first['tables'][name]['definition']) + ':IndexData:' + str(number)
        assert set(final['maps'][role]['file_members']) == members
        assert reached.isdisjoint(members)
        reached.update(members)
    new_index = set()
    for (name, physical), nodes in selected_physical.items():
        old_root = original['indexes']['PairOrder' if physical else 'LeftOrder']['root']
        assert old_root in nodes
        assert all(number == old_root or number >= old_eof for number in nodes)
        new_index.update(number for number in nodes if number >= old_eof)
        role = owner + ':IndexData:' + str(physical)
        old_map, new_map = first['maps'][role], final['maps'][role]
        start, end = old_map['row_start'], old_map['row_end']
        assert (new_map['row_start'], new_map['row_end']) == (start, end)
        expected = bytearray(before[start:end])
        for number in nodes:
            if number != old_root:
                cell = source.future(before, old_map, number)
                assert cell is not None and not cell['set']
                assert start <= cell['offset'] < end
                expected[cell['offset'] - start] |= cell['mask']
        assert after[start:end] == expected
        allow(start, end)
    free = set(final['maps']['GlobalFree']['file_members'])
    suffix = set(range(old_eof, new_eof))
    assert suffix == new_index | (suffix & free)
    assert new_index.isdisjoint(free)
    old_map, new_map = first['maps']['GlobalFree'], final['maps']['GlobalFree']
    start, end = old_map['row_start'], old_map['row_end']
    assert (new_map['row_start'], new_map['row_end']) == (start, end)
    expected = bytearray(before[start:end])
    for number in new_index:
        cell = source.future(before, old_map, number)
        assert cell is not None and cell['set'] and start <= cell['offset'] < end
        expected[cell['offset'] - start] &= ~cell['mask']
    assert after[start:end] == expected
    allow(start, end)
    for role in first['maps']:
        if role not in changed_map_roles:
            # Unset represented cells acquire decode-only locations as EOF grows.
            # Their source row/bitmap bytes remain covered by the prefix proof.
            original_map = {field: value for field, value in first['maps'][role].items()
                            if field != 'bit_locations'}
            actual_map = {field: value for field, value in final['maps'][role].items()
                          if field != 'bit_locations'}
            assert original_map == actual_map
    selected_roles = [role for role in final['claims'] if role.startswith(owner + ':')]
    source.ordinary.require_exclusive(final['claims'], selected_roles)
    source.records.catalog_long_values(after, final['tables'], final['declarations'],
                                       final['maps'], final['claims'])
    assert all(old == new or allowed[offset]
               for offset, (old, new) in enumerate(zip(before, after)))
    return dict(id=case['id'],source_sha256=hashlib.sha256(before).hexdigest(),
                output_sha256=hashlib.sha256(after).hexdigest(), rows=500,
                updates=case['updates'], warnings=len(observed['warnings']),
                histories=observed['histories'], old_eof=old_eof, new_eof=new_eof,
                new_index_pages=sorted(new_index), new_free_pages=sorted(suffix & free),
                physical_pages={str(number): sorted(nodes) for (_, number), nodes in selected_physical.items()},
                inactive_null_payloads=null_payload_observations,
                prefix_differing_bytes=sum(a != b for a, b in zip(before, after)))
