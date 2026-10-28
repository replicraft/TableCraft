"""Independent raw finite variable-key source and public-reference gates; no producer."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import facts
from ace16_insert_candidates import model as wire
import delete_reference_maps as maps_probe
from ace16_insert_variable_indexes import recipes

PAGE = 4096
CAPACITY = 3616
NAMES = ['PrimaryID', 'PrimaryAlias', 'GroupOrder', 'GroupAlias']


def n(raw, offset, width=4):
    value = raw[offset:offset + width]
    assert len(value) == width
    return int.from_bytes(value, 'little')


def components(case, primary=False):
    if primary:
        return [(0, False)]
    return list(zip(recipes.SHAPES[case['shape']],
                    [direction == 'D' for direction in case['directions']], strict=True))


def require_entry(entry, primary):
    # Secondary digest keys are deliberately not reversible. Their full wire is
    # bound below to independently decoded source fields and logical ROWs.
    assert 5 <= len(entry) <= 514
    if primary:
        assert len(entry) == 9 and entry[0] == 127
    assert int.from_bytes(entry[-4:], 'big') >> 8 != 0


def entry_order(entry):
    assert len(entry) >= 5
    return entry[:-4], int.from_bytes(entry[-4:], 'big')


def tree(raw, root, owner, case, primary=False):
    nodes = {}
    levels = {}
    leaves = []

    def visit(number, expected_height=None):
        assert number not in nodes and 0 < number < len(raw) // PAGE
        page = raw[number * PAGE:(number + 1) * PAGE]
        assert page[:2] in (bytes([3, 1]), bytes([4, 1]))
        assert n(page, 4) == owner
        height = page[26]
        assert height < 16 and (expected_height is None or height == expected_height)
        assert (page[0] == 4) == (height == 0)
        assert not page[27] & 1 and not page[479] & 0xfe
        ends = [end for end in range(1, CAPACITY + 1)
                if page[27 + end // 8] & (1 << (end % 8))]
        active = CAPACITY - n(page, 2, 2)
        assert active == (ends[-1] if ends else 0)
        prefix = n(page, 24, 2)
        entries = []
        children = []
        start = 0
        for end in ends:
            payload = page[480 + start:480 + end]
            if height:
                assert len(payload) >= 4
                children.append(int.from_bytes(payload[-4:], 'big'))
                payload = payload[:-4]
            complete = payload if not entries else entries[-1][:prefix] + payload
            require_entry(complete, primary)
            assert not entries or entry_order(entries[-1]) < entry_order(complete)
            entries.append(complete)
            start = end
        common = min(map(len, entries), default=0)
        for entry in entries[1:]:
            common = next((position for position, pair in enumerate(zip(entries[0][:common], entry))
                           if pair[0] != pair[1]), common)
        assert prefix <= common
        assert active == sum(map(len, entries)) + (4 * len(entries) if height else 0) - prefix * max(0, len(entries) - 1)
        tail = n(page, 20)
        nodes[number] = {'height': height, 'prefix': prefix, 'active': active,
                         'previous': n(page, 12), 'next': n(page, 16),
                         'tail': tail, 'entries': entries}
        levels.setdefault(height, []).append(number)
        if not height:
            assert tail == 0
            leaves.append(number)
            return entries
        if tail:
            children.append(tail)
        assert children
        result = []
        for position, child in enumerate(children):
            values = visit(child, height - 1)
            assert values
            if position < len(entries):
                assert entries[position] == values[-1]
            result.extend(values)
        assert result == sorted(result, key=entry_order) and len(result) == len(set(result))
        return result

    entries = visit(root)
    for sequence in levels.values():
        for position, number in enumerate(sequence):
            assert nodes[number]['previous'] == (sequence[position - 1] if position else 0)
            assert nodes[number]['next'] == (sequence[position + 1] if position + 1 < len(sequence) else 0)
    return nodes, leaves, entries


def layout_from(raw, table, case):
    base = table['definition'] * PAGE
    assert n(raw, base + 41, 2) == n(raw, base + 45, 2) == 3
    assert n(raw, base + 43, 2) == 1
    assert n(raw, base + 47) == 4 and n(raw, base + 51) == 2
    fields = []
    occupied = set()
    fixed = 0
    for position, name in enumerate(['ID', 'Group', 'Payload']):
        at = base + 87 + 25 * position
        variable = position == 2
        native = (9 if case['family'] == 'Binary' else 10) if variable else 4
        width = (255 if case['family'] == 'Binary' else 510) if variable else 4
        flags = 2 if variable else 3
        assert raw[at] == native and n(raw, at + 5, 2) == position
        assert raw[at + 15:at + 17] == bytes([flags, 0])
        assert n(raw, at + 23, 2) == width
        offset = n(raw, at + 21, 2)
        ordinal = n(raw, at + 7, 2)
        if variable:
            assert ordinal == 0
        else:
            span = set(range(offset, offset + width))
            assert not span & occupied
            occupied |= span
            fixed = max(fixed, offset + width)
        observed = table['columns'][name]
        assert observed['position'] == observed['number'] == position
        assert observed['native_type'] == native and observed['declared_bytes'] == width
        assert observed['compressed'] is False
        descriptor = None
        if variable and native == 10:
            descriptor = {'lcid': 1033, 'variant': 0, 'version': int(case['family'] == 'Modern')}
            assert observed['sort'] == descriptor
            assert raw[at + 11:at + 15] == bytes([9, 4, 0, descriptor['version']])
        fields.append({'name': name, 'number': position, 'offset': offset,
                       'declared_width': width, 'physical_width': 0 if variable else width,
                       'native_type': native, 'flags': flags, 'extended_flags': 0,
                       'variable_ordinal': ordinal, 'sort': descriptor})
    at = base + 87 + 75
    for name in ['ID', 'Group', 'Payload']:
        size = n(raw, at, 2)
        assert raw[at + 2:at + 2 + size].decode('utf-16le') == name
        at += size + 2
    return {'columns': 3, 'variables': 1, 'fixed_end': fixed + 2,
            'fields': fields, 'physical_start': at}


def decode_record(body, layout, case):
    assert n(body, 0, 2) == 3 and len(body) >= layout['fixed_end'] + 7
    assert n(body, len(body) - 3, 2) == 1
    start = n(body, len(body) - 5, 2)
    end = n(body, len(body) - 7, 2)
    assert start == layout['fixed_end'] and start <= end == len(body) - 7
    mask = body[-1]
    assert mask & 1
    values = []
    for field in layout['fields']:
        number = field['number']
        if not mask & (1 << number):
            values.append(None)  # Old NULL payloads/padding remain opaque.
        elif number == 2:
            value = body[start:end]
            if case['family'] != 'Binary':
                assert len(value) % 2 == 0
                value = value.decode('utf-16le', errors='strict').encode('ascii', errors='strict')
                assert all(byte in (97, 122) for byte in value)
            assert len(value) <= 255
            values.append(value)
        else:
            at = 2 + field['offset']
            value = body[at:at + 4]
            assert len(value) == 4
            values.append(int.from_bytes(value, 'little', signed=True))
    return tuple(values)


def getter_payload(tag, case):
    if tag == 'NULL':
        return None
    if case['family'] == 'Binary':
        assert tag.startswith('Binary:')
        return bytes.fromhex(tag[len('Binary:'):])
    assert tag.startswith('String16:')
    return bytes.fromhex(tag[len('String16:'):]).decode('utf-16le', errors='strict').encode('ascii', errors='strict')


def encoders(case, path):
    lines = path.read_text().splitlines()
    rows = recipes.source_rows(case)
    expected = (f"SOURCE\t{case['profile']}\t{case['family']}\t{case['shape']}\t{case['directions']}\t"
                f"{case['mode']}\t{case['policy']}\t{len(rows)}")
    assert lines[0] == expected
    operands = [('OLD', row) for row in rows] + [('NEW', row) for row in recipes.requests(case)]
    assert len(lines) == len(operands) + 1
    for line, (stage, row) in zip(lines[1:], operands, strict=True):
        assert line == f"ENCODE\t{stage}\t{row[0]}\t{recipes.encoded_key(row, case).hex()}"


def parse_getter(text):
    identities = set()
    for line in text.splitlines():
        parts = line.split('\t')
        if parts[0] in ('TABLE', 'COLUMN', 'ROW', 'RAWROW', 'INDEX'):
            identity = tuple(parts[:2 if parts[0] == 'TABLE' else 3])
            assert identity not in identities
            identities.add(identity)
        if parts[0] == 'COLUMN':
            assert parts[7] in ('true', 'false')
        if parts[0] == 'INDEX':
            assert all(value in ('true', 'false') for value in parts[7:11])
        if parts[0] == 'INDEX_COLUMN':
            assert parts[4] in ('true', 'false')
    return facts.parse(text)


def inspect(case, image, getter, maps, encoder_path=None, after=False):
    recipes.validate(case)
    raw = image.read_bytes()
    assert 0 < len(raw) <= 64 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[4:20] == b'Standard ACE DB\x00'
    assert raw[20] == (5 if case['profile'] == 'V2016' else 6)
    encoders(case, encoder_path if encoder_path is not None else ROOT / case['encoder'])
    tables = parse_getter(getter.read_text())
    assert list(tables) == ['Items']
    table = tables['Items']
    rows = recipes.source_rows(case) + (recipes.requests(case) if after else [])
    assert table['count'] == len(table['rows']) == len(table['raw_rows']) == len(rows)
    assert table['columns_count'] == table['max_columns'] == 3
    assert list(table['columns']) == ['ID', 'Group', 'Payload']
    properties = [['ID', 'Required', '1', 'true', 'Boolean:true']]
    if case['family'] != 'Binary':
        properties.append(['Payload', 'AllowZeroLength', '1', 'true', 'Boolean:true'])
    assert table['properties'] == properties
    assert list(table['indexes']) == NAMES
    layout = layout_from(raw, table, case)
    definition = table['definition']
    base = definition * PAGE
    assert n(raw, base + 16) == len(rows)
    physical = []
    for position in range(2):
        block = raw[layout['physical_start'] + 52 * position:layout['physical_start'] + 52 * (position + 1)]
        selected = components(case, position == 0)
        for slot, (column, descending) in enumerate(selected):
            assert n(block, 4 + 3 * slot, 2) == column
            assert block[6 + 3 * slot] == int(not descending)
        assert all(n(block, 4 + 3 * slot, 2) == 65535 for slot in range(len(selected), 10))
        flags = 0x89 if position == 0 else {
            'ordinary': 0x80, 'unique': 0x81, 'unique-ignored': 0x83,
        }[case['policy']]
        assert block[46] == flags
        physical.append({'root': n(block, 38), 'usage': n(block, 34),
                         'history': base + 67 + 12 * position})
    assert physical[0]['root'] != physical[1]['root']
    references = {}
    populations = {}
    for expected in rows:
        identity = expected[0]
        row = table['rows'][identity]
        reference = row['page'] * 256 + row['slot']
        assert reference not in references
        references[reference] = expected
        begin, end = wire.frame(raw, row['page'], row['slot'])
        body = raw[begin:end]
        assert bytes.fromhex(table['raw_rows'][identity]) == body
        assert decode_record(body, layout, case) == expected
        assert len(row['values']) == 3
        assert row['values'][0] == 'Integer:' + str(identity)
        assert row['values'][1] == ('NULL' if expected[1] is None else 'Integer:' + str(expected[1]))
        assert getter_payload(row['values'][2], case) == expected[2]
        page = row['page'] * PAGE
        assert raw[page:page + 2] == bytes([1, 1]) and n(raw, page + 4) == definition
        assert n(raw, page + 14 + 2 * row['slot'], 2) < PAGE
        populations.setdefault(row['page'], []).append(row['slot'])
    for page, slots in populations.items():
        assert sorted(slots) == list(range(len(slots)))
        assert n(raw, page * PAGE + 12, 2) == len(slots)
        last = n(raw, page * PAGE + 14 + 2 * (len(slots) - 1), 2)
        assert n(raw, page * PAGE + 2, 2) == last - 14 - 2 * len(slots)
    storage = {'owned': wire.map_record(raw, n(raw, base + 55)),
               'hint': wire.map_record(raw, n(raw, base + 59)),
               'global': wire.map_record(raw, 256)}
    trees = []
    for position, metadata in enumerate(physical):
        nodes, leaves, entries = tree(raw, metadata['root'], definition, case, position == 0)
        selected_rows = {reference: values for reference, values in references.items()
                         if position == 0 or recipes.eligible(values, case)}
        expected = sorted((recipes.encoded_key(values, case, position == 0) + reference.to_bytes(4, 'big')
                           for reference, values in selected_rows.items()), key=entry_order)
        assert entries == expected
        for name in NAMES[2 * position:2 * position + 2]:
            index = table['indexes'][name]
            assert index['physical'] == position and index['root'] == metadata['root']
            assert index['entries'] == len(expected) and index['owned_pages'] == len(nodes)
            assert index['primary'] == (name == 'PrimaryID')
            assert index['required'] == (position == 0)
            assert index['unique'] == (position == 0 or case['policy'] != 'ordinary')
            assert index['ignored_null'] == (position == 1 and case['policy'] == 'unique-ignored')
            selected = components(case, position == 0)
            assert index['columns'] == [[['ID', 'Group', 'Payload'][column], 'false' if descending else 'true']
                                        for column, descending in selected]
            assert index['order'] == [references[int.from_bytes(entry[-4:], 'big')][0] for entry in entries]
        role = 'index' + str(position)
        storage[role] = wire.map_record(raw, metadata['usage'])
        assert storage[role]['members'] == set(nodes)
        if not after:
            assert len(nodes) == len(leaves) == 1
            if position == 1 and case['mode'] == 'pressure':
                assert nodes[metadata['root']]['prefix'] == 0
                assert nodes[metadata['root']]['active'] == 3521
        if not after and position == 1 and case['mode'].startswith('digest'):
            assert len(entries) == 1 and len(entries[0]) == 514
            assert len(table['raw_rows'][rows[0][0]]) // 2 == layout['fixed_end'] + 510 + 7
        trees.append(nodes)
    assert not set(trees[0]) & set(trees[1])
    assert storage['hint']['members'] <= storage['owned']['members']
    assert {page for page in storage['owned']['members'] if page < len(raw) // PAGE} == set(populations)
    assert not (set(populations) | set(trees[0]) | set(trees[1])) & storage['global']['members']
    assert not (set(trees[0]) | set(trees[1])) & set(populations)
    ranges = [tuple(metadata['range']) for metadata in storage.values()]
    assert all(left_end <= right_start or right_end <= left_start
               for position, (left_start, left_end) in enumerate(ranges)
               for right_start, right_end in ranges[position + 1:])
    parsed_maps = maps_probe.parse(maps.read_text())
    assert set(parsed_maps['maps']) == {'JackcessGlobalFree', 'TableOwned', 'TableFreeSpace'}
    for name, role in [('JackcessGlobalFree', 'global'), ('TableOwned', 'owned'), ('TableFreeSpace', 'hint')]:
        checked = maps_probe.decode_map(raw, parsed_maps['maps'][name], PAGE, len(raw) // PAGE)
        assert set(checked['file_members']) == {page for page in storage[role]['members'] if page < len(raw) // PAGE}
    if not after:
        families = {recipes.encoded_key(row, case) for row in rows if recipes.eligible(row, case)}
        assert [n(raw, item['history']) for item in physical] == [len(rows), len(families)]
    return {'raw': raw, 'table': table, 'physical': physical, 'rows': references,
            'maps': storage, 'trees': trees, 'layout': layout, 'native_outcome': case['native_outcome']}


def describe(state):
    return {'source_gate_scope': 'raw schema/rows/keys; no native write admission',
            'expected_native_outcome': state['native_outcome'],
            'rows': state['table']['count'], 'physical': state['physical'], 'layout': state['layout'],
            'data_pages': sorted({reference >> 8 for reference in state['rows']}),
            'nodes': [sorted(tree) for tree in state['trees']],
            'histories': [n(state['raw'], item['history']) for item in state['physical']],
            'maps': {role: {'reference': item['reference'], 'range': list(item['range']),
                            'members': sorted(item['members'])} for role, item in state['maps'].items()}}


def transition(case, before, after):
    assert before['physical'] == after['physical'] and before['layout'] == after['layout']
    for reference, value in before['rows'].items():
        assert after['rows'][reference] == value
        identity = value[0]
        assert before['table']['rows'][identity] == after['table']['rows'][identity]
        assert before['table']['raw_rows'][identity] == after['table']['raw_rows'][identity]
    assert before['table']['columns'] == after['table']['columns']
    assert before['table']['properties'] == after['table']['properties']
    requested = recipes.requests(case)
    old_families = {recipes.encoded_key(row, case) for row in recipes.source_rows(case) if recipes.eligible(row, case)}
    new_families = {recipes.encoded_key(row, case) for row in requested if recipes.eligible(row, case)}
    deltas = [len(requested), len(new_families - old_families)]
    for metadata, delta in zip(before['physical'], deltas, strict=True):
        assert n(after['raw'], metadata['history']) == n(before['raw'], metadata['history']) + delta
    assert len(after['trees'][0]) == 1
    if case['mode'] == 'pressure':
        secondary = after['physical'][1]['root']
        assert after['trees'][1][secondary]['height'] == 1 and len(after['trees'][1]) == 3
    old_eof = len(before['raw']) // PAGE
    new_eof = len(after['raw']) // PAGE
    suffix = set(range(old_eof, new_eof))
    old_data = {reference >> 8 for reference in before['rows']}
    new_data = {reference >> 8 for reference in after['rows']} - old_data
    new_indexes = set().union(*[set(new) - set(old) for new, old in zip(after['trees'], before['trees'], strict=True)])
    assert suffix == new_data | new_indexes and not new_data & new_indexes
    assert after['maps']['owned']['members'] == before['maps']['owned']['members'] | new_data
    assert after['maps']['global']['members'] == before['maps']['global']['members'] - suffix
    for position in range(2):
        role = 'index' + str(position)
        assert after['maps'][role]['members'] == before['maps'][role]['members'] | set(after['trees'][position])
    hints = set(before['maps']['hint']['members'])
    page_state = {page: [n(before['raw'], page * PAGE + 2, 2),
                         n(before['raw'], page * PAGE + 12, 2)] for page in old_data}
    discovered_data = set()
    for values in requested:
        actual = after['table']['rows'][values[0]]
        begin, end = wire.frame(after['raw'], actual['page'], actual['slot'])
        needed = end - begin + 2
        selected = max(hints) if hints else None
        if selected is not None:
            free, slots = page_state[selected]
            if slots >= 255 or free < needed:
                hints.remove(selected)
                selected = None
        if selected is None:
            selected = actual['page']
            assert selected in new_data and selected not in discovered_data
            discovered_data.add(selected)
            page_state[selected] = [4082, 0]
            hints.add(selected)
        assert actual['page'] == selected and actual['slot'] == page_state[selected][1]
        page_state[selected][0] -= needed
        page_state[selected][1] += 1
    assert discovered_data == new_data
    assert after['maps']['hint']['members'] == hints
    for page, (free, slots) in page_state.items():
        assert n(after['raw'], page * PAGE + 2, 2) == free
        assert n(after['raw'], page * PAGE + 12, 2) == slots
    if case['mode'] == 'pressure':
        assert len(new_data) == 1 and len(new_indexes) == 2
        assert len(suffix) == 3  # Public allocation order is observed, not engineering policy.
    protected = bytearray(before['raw'])
    definition = before['table']['definition'] * PAGE
    protected[definition + 16:definition + 20] = after['raw'][definition + 16:definition + 20]
    for metadata in before['physical']:
        at = metadata['history']
        protected[at:at + 4] = after['raw'][at:at + 4]
    for tree_before in before['trees']:
        for page in tree_before:
            protected[page * PAGE:(page + 1) * PAGE] = after['raw'][page * PAGE:(page + 1) * PAGE]
    for reference in set(after['rows']) - set(before['rows']):
        page, slot = reference >> 8, reference & 255
        if page in old_data:
            begin, end = wire.frame(after['raw'], page, slot)
            protected[begin:end] = after['raw'][begin:end]
            at = page * PAGE + 14 + 2 * slot
            protected[at:at + 2] = after['raw'][at:at + 2]
            for offset in (2, 12):
                at = page * PAGE + offset
                protected[at:at + 2] = after['raw'][at:at + 2]
    for role, old in before['maps'].items():
        new = after['maps'][role]
        assert new['reference'] == old['reference'] and new['range'] == old['range'] and new['kind'] == old['kind']
        for member in old['members'] ^ new['members']:
            offset, mask = old['locations'][member]
            protected[offset] ^= mask
    assert bytes(protected) == after['raw'][:len(protected)]
    return {'inserted_rows': len(requested), 'counter_deltas': deltas,
            'source_rows_preserved': len(before['rows']), 'new_data': sorted(new_data),
            'new_indexes': sorted(new_indexes), 'suffix': sorted(suffix)}
