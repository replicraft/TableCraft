"""Raw scalar source/reference gates; no native producer or candidate authority."""
from datetime import datetime, timedelta
import math
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import facts
from ace16_insert_candidates import model as wire
import delete_reference_maps as maps_probe

PAGE = 4096
CAPACITY = 3616
NAMES = ['PrimaryID', 'PrimaryAlias', 'GroupOrder', 'GroupAlias']
KINDS = {'Boolean': (1, 0), 'Single': (6, 4), 'Double': (7, 8), 'DateTime': (8, 8)}
EPOCH = datetime(1899, 12, 30)


def n(raw, at, width=4):
    value = raw[at:at + width]
    assert len(value) == width
    return int.from_bytes(value, 'little')


def double_bits(value):
    return int.from_bytes(struct.pack('<d', value), 'little')


def source_rows(case):
    family = case['family']
    assert family in KINDS and case['direction'] in ('ASC', 'DESC')
    if family == 'Boolean':
        return [(1, False, b''), (2, True, b''), (3, False, b'')]
    count = 400 if family == 'Single' else 277
    rows = []
    for position in range(count):
        if family == 'Single':
            value = 0x3f000000 + position
        elif family == 'Double':
            value = 0x3fe0000000000000 + position
        else:
            value = double_bits(36526.5 + position)
        rows.append((position + 1, value, b''))
    return rows + [(count + 1, None, b'')]


def requests(case):
    return {
        'Boolean': [(4, False, b''), (5, True, b'')],
        'Single': [(402, 0x80000000, b''), (403, 0, b'')],
        'Double': [(279, 0x8000000000000000, b'')],
        'DateTime': [(279, 0, b'')],
    }[case['family']]


def key(value, case, primary=False):
    if primary:
        assert type(value) is int and -(2 ** 31) <= value < 2 ** 31
        return b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
    family = case['family']
    if family == 'Boolean':
        assert type(value) is bool
        result = bytes([127, 0 if value else 255])
    elif value is None:
        result = b'\x00'
    else:
        width = KINDS[family][1]
        assert type(value) is int and 0 <= value < 2 ** (8 * width)
        sign = 1 << (8 * width - 1)
        ordered = value ^ sign if not value & sign else value ^ ((1 << (8 * width)) - 1)
        result = b'\x7f' + ordered.to_bytes(width, 'big')
    return bytes(byte ^ 255 for byte in result) if case['direction'] == 'DESC' else result


def tree(raw, root, owner, case=None, primary=False):
    assert case is not None
    family = 'Int32' if primary else case['family']
    width = 4 if primary else KINDS[family][1]
    full_width = 9 if primary else 6 if family == 'Boolean' else width + 5
    nodes = {}
    levels = {}
    leaves = []

    def visit(number, wanted_height=None):
        assert number not in nodes and 0 < number < len(raw) // PAGE
        page = raw[number * PAGE:(number + 1) * PAGE]
        assert page[0] in (3, 4) and page[1] == 1 and n(page, 4) == owner
        height = page[26]
        assert height < 16 and (wanted_height is None or height == wanted_height)
        assert (page[0] == 4) == (height == 0)
        assert not page[27] & 1 and not page[479] & 0xfe
        ends = [end for end in range(1, CAPACITY + 1) if page[27 + end // 8] & (1 << (end % 8))]
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
            assert complete and complete[0] in (0, 127, 128, 255)
            is_null = complete[0] in (0, 255)
            assert len(complete) == (5 if is_null else full_width)
            assert not primary or not is_null
            assert family != 'Boolean' or not is_null
            assert not entries or entries[-1] < complete
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
                         'previous': n(page, 12), 'next': n(page, 16), 'tail': tail,
                         'entries': entries}
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
        assert result == sorted(result) and len(result) == len(set(result))
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
    assert n(raw, base + 43, 2) == 1 and n(raw, base + 47) == 4 and n(raw, base + 51) == 2
    descriptors = []
    kinds = [4, KINDS[case['family']][0], 9]
    widths = [4, KINDS[case['family']][1], 255]
    occupied = set()
    fixed = 0
    start = base + 87
    for position, name in enumerate(['ID', 'Group', 'Bytes']):
        at = start + 25 * position
        native = raw[at]
        declared = n(raw, at + 23, 2)
        flags, extended = raw[at + 15:at + 17]
        assert native == kinds[position] and n(raw, at + 5, 2) == position
        assert flags == (2 if position == 2 else 3) and extended == 0
        assert declared == (1 if native == 1 else widths[position])
        offset = n(raw, at + 21, 2)
        variable = n(raw, at + 7, 2)
        physical = 0 if native == 1 else widths[position]
        if position == 2:
            assert variable == 0
            physical = 0
        elif physical:
            span = set(range(offset, offset + physical))
            assert not span & occupied
            occupied |= span
            fixed = max(fixed, offset + physical)
        descriptors.append({'name': name, 'number': position, 'offset': offset,
                            'declared_width': declared, 'physical_width': physical,
                            'variable_ordinal': variable, 'native_type': native,
                            'flags': flags, 'extended_flags': extended})
    for field in descriptors[:2]:
        assert field['native_type'] != 1 or field['offset'] <= fixed
    assert occupied == set(range(fixed))
    at = start + 75
    for name in ['ID', 'Group', 'Bytes']:
        size = n(raw, at, 2)
        assert raw[at + 2:at + 2 + size].decode('utf-16le') == name
        at += size + 2
    return {'fields': descriptors, 'fixed_end': 2 + fixed, 'columns': 3,
            'variables': 1, 'physical_start': at}


def decode_record(body, layout, case):
    assert n(body, 0, 2) == 3
    assert n(body, len(body) - 3, 2) == 1
    assert n(body, len(body) - 5, 2) == layout['fixed_end']
    end = n(body, len(body) - 7, 2)
    assert end == len(body) - 7 == layout['fixed_end']
    mask = body[-1]
    assert mask & 1 and mask & 4 and not mask & 0xf8
    identity_field, group_field, _ = layout['fields']
    identity = int.from_bytes(body[2 + identity_field['offset']:6 + identity_field['offset']], 'little', signed=True)
    if case['family'] == 'Boolean':
        value = bool(mask & 2)
    elif not mask & 2:
        value = None  # Existing NULL fixed bytes remain opaque and preserved.
    else:
        at = 2 + group_field['offset']
        value = n(body, at, group_field['physical_width'])
    return identity, value, b''


def getter_group(tag, case):
    family = case['family']
    if tag == 'NULL':
        assert family != 'Boolean'
        return None
    if family == 'Boolean':
        assert tag in ('Boolean:true', 'Boolean:false')
        return tag == 'Boolean:true'
    if family in ('Single', 'Double'):
        prefix = 'Float:' if family == 'Single' else 'Double:'
        assert tag.startswith(prefix)
        value = float(tag[len(prefix):])
        assert math.isfinite(value)
        fmt = '<f' if family == 'Single' else '<d'
        return int.from_bytes(struct.pack(fmt, value), 'little')
    assert tag.startswith('LocalDateTime:')
    value = datetime.fromisoformat(tag[len('LocalDateTime:'):])
    assert value.tzinfo is None
    delta = value - EPOCH
    assert delta.days >= 0 and delta.microseconds == 0
    return double_bits(delta.days + delta.seconds / 86400)


def encoders(case, path):
    lines = path.read_text().splitlines()
    wanted = f"SOURCE\t{case['profile']}\t{case['family']}\t{case['direction']}\t{len(source_rows(case))}"
    assert lines[0] == wanted
    expected = [('OLD', row) for row in source_rows(case)] + [('NEW', row) for row in requests(case)]
    assert len(lines) == len(expected) + 1
    for line, (stage, row) in zip(lines[1:], expected, strict=True):
        identity, value, _ = row
        assert line == f'ENCODE\t{stage}\t{identity}\t{key(value, case).hex()}'


def inspect(case, image, getter, maps, encoder_path=None, after=False):
    assert case['profile'] in ('V2016', 'V2019')
    raw = image.read_bytes()
    assert 0 < len(raw) <= 64 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[4:20] == b'Standard ACE DB\x00' and raw[20] == (5 if case['profile'] == 'V2016' else 6)
    encoders(case, encoder_path if encoder_path is not None else ROOT / case['encoder'])
    text = getter.read_text()
    identities = set()
    for line in text.splitlines():
        parts = line.split('\t')
        if parts[0] in ('TABLE', 'COLUMN', 'ROW', 'RAWROW', 'INDEX'):
            identity = tuple(parts[:2 if parts[0] == 'TABLE' else 3])
            assert identity not in identities
            identities.add(identity)
        if parts[0] == 'INDEX':
            assert all(value in ('true', 'false') for value in parts[7:11])
    tables = facts.parse(text)
    assert list(tables) == ['Items']
    table = tables['Items']
    rows = source_rows(case) + (requests(case) if after else [])
    assert table['count'] == len(table['rows']) == len(table['raw_rows']) == len(rows)
    assert table['columns_count'] == table['max_columns'] == 3
    assert list(table['columns']) == ['ID', 'Group', 'Bytes']
    assert table['properties'] == [['ID', 'Required', '1', 'true', 'Boolean:true']]
    assert list(table['indexes']) == NAMES
    layout = layout_from(raw, table, case)
    base = table['definition'] * PAGE
    assert n(raw, base + 16) == len(rows)
    physical = []
    for position in range(2):
        block = raw[layout['physical_start'] + 52 * position:layout['physical_start'] + 52 * (position + 1)]
        assert n(block, 4, 2) == position
        assert block[6] == (1 if position == 0 or case['direction'] == 'ASC' else 0)
        assert all(n(block, 4 + 3 * slot, 2) == 65535 for slot in range(1, 10))
        assert block[46] == (0x89 if position == 0 else 0x80)
        physical.append({'root': n(block, 38), 'usage': n(block, 34), 'history': base + 67 + 12 * position})
    assert physical[0]['root'] != physical[1]['root']
    references = {}
    populations = {}
    for identity, value, binary in rows:
        row = table['rows'][identity]
        reference = row['page'] * 256 + row['slot']
        assert reference not in references
        references[reference] = (identity, value, binary)
        begin, end = wire.frame(raw, row['page'], row['slot'])
        body = raw[begin:end]
        assert body == bytes.fromhex(table['raw_rows'][identity])
        assert decode_record(body, layout, case) == (identity, value, binary)
        assert row['values'][0] == 'Integer:' + str(identity)
        assert getter_group(row['values'][1], case) == value
        assert row['values'][2] == 'Binary:'
        assert raw[row['page'] * PAGE:row['page'] * PAGE + 2] == bytes([1, 1])
        assert n(raw, row['page'] * PAGE + 4) == table['definition']
        assert n(raw, row['page'] * PAGE + 14 + 2 * row['slot'], 2) < PAGE
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
        nodes, leaves, entries = tree(raw, metadata['root'], table['definition'], case, position == 0)
        expected = sorted(key(values[position], case, position == 0) + reference.to_bytes(4, 'big')
                          for reference, values in references.items())
        assert entries == expected
        for name in NAMES[2 * position:2 * position + 2]:
            index = table['indexes'][name]
            assert index['physical'] == position and index['root'] == metadata['root']
            assert index['entries'] == len(rows) and index['owned_pages'] == len(nodes)
            assert index['unique'] == index['required'] == (position == 0)
            assert not index['ignored_null'] and index['primary'] == (name == 'PrimaryID')
            assert index['columns'] == [[('ID' if position == 0 else 'Group'),
                                          'true' if position == 0 or case['direction'] == 'ASC' else 'false']]
            assert index['order'] == [references[int.from_bytes(entry[-4:], 'big')][0] for entry in entries]
        role = 'index' + str(position)
        storage[role] = wire.map_record(raw, metadata['usage'])
        assert storage[role]['members'] == set(nodes)
        if not after:
            assert len(nodes) == len(leaves) == 1
            if position == 1 and case['family'] != 'Boolean':
                assert nodes[metadata['root']]['prefix'] == 0
                assert nodes[metadata['root']]['active'] == (3605 if case['family'] == 'Single' else 3606)
        trees.append(nodes)
    assert not set(trees[0]) & set(trees[1])
    assert storage['index0']['range'] != storage['index1']['range']
    assert storage['hint']['members'] <= storage['owned']['members']
    assert {page for page in storage['owned']['members'] if page < len(raw) // PAGE} == set(populations)
    assert not (set(populations) | set(trees[0]) | set(trees[1])) & storage['global']['members']
    parsed = maps_probe.parse(maps.read_text())
    assert set(parsed['maps']) == {'JackcessGlobalFree', 'TableOwned', 'TableFreeSpace'}
    for name, role in [('JackcessGlobalFree', 'global'), ('TableOwned', 'owned'), ('TableFreeSpace', 'hint')]:
        checked = maps_probe.decode_map(raw, parsed['maps'][name], PAGE, len(raw) // PAGE)
        assert set(checked['file_members']) == {page for page in storage[role]['members'] if page < len(raw) // PAGE}
    if not after:
        families = {key(row[1], case) for row in rows}
        assert [n(raw, item['history']) for item in physical] == [len(rows), len(families)]
    return {'raw': raw, 'table': table, 'physical': physical, 'rows': references,
            'maps': storage, 'trees': trees, 'layout': layout}


def describe(state):
    return {'rows': state['table']['count'], 'physical': state['physical'],
            'layout': state['layout'], 'data_pages': sorted({reference >> 8 for reference in state['rows']}),
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
    requested = requests(case)
    old_families = {key(row[1], case) for row in source_rows(case)}
    new_families = {key(row[1], case) for row in requested}
    deltas = [len(requested), len(new_families - old_families)]
    for metadata, delta in zip(before['physical'], deltas, strict=True):
        assert n(after['raw'], metadata['history']) == n(before['raw'], metadata['history']) + delta
    assert len(after['trees'][0]) == 1
    if case['family'] != 'Boolean':
        secondary = after['physical'][1]['root']
        assert after['trees'][1][secondary]['height'] == 1 and len(after['trees'][1]) == 3
    old_eof = len(before['raw']) // PAGE
    new_eof = len(after['raw']) // PAGE
    suffix = set(range(old_eof, new_eof))
    new_data = {reference >> 8 for reference in after['rows']} - {reference >> 8 for reference in before['rows']}
    new_indexes = set().union(*[set(new) - set(old) for new, old in zip(after['trees'], before['trees'], strict=True)])
    assert suffix == new_data | new_indexes and not new_data & new_indexes
    assert after['maps']['owned']['members'] == before['maps']['owned']['members'] | new_data
    assert after['maps']['global']['members'] == before['maps']['global']['members'] - suffix
    for position in range(2):
        role = 'index' + str(position)
        assert after['maps'][role]['members'] == before['maps'][role]['members'] | set(after['trees'][position])
    record_size = after['layout']['fixed_end'] + 7
    hints = {page for page in before['maps']['hint']['members'] | new_data
             if n(after['raw'], page * PAGE + 2, 2) >= record_size + 2}
    assert after['maps']['hint']['members'] == hints
    protected = bytearray(before['raw'])
    definition = before['table']['definition'] * PAGE
    protected[definition + 16:definition + 20] = after['raw'][definition + 16:definition + 20]
    for metadata in before['physical']:
        at = metadata['history']
        protected[at:at + 4] = after['raw'][at:at + 4]
    for tree_before in before['trees']:
        for page in tree_before:
            protected[page * PAGE:(page + 1) * PAGE] = after['raw'][page * PAGE:(page + 1) * PAGE]
    old_data = {reference >> 8 for reference in before['rows']}
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
