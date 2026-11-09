"""Actual source admission and reference framing; never candidate image policy."""
from pathlib import Path
import hashlib
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_candidates import model as CODEC
from parent_index_split_originals import wire as INDEX
from slot_preview_originals import facts
import delete_reference_maps as MAPS

PAGE = 4096
KEYS = list(range(-226, 0)) + list(range(1, 226))


def number(raw, offset, width=4):
    value = raw[offset:offset + width]
    assert len(value) == width
    return int.from_bytes(value, 'little')


def payload(variant, ordinal, inserted=False):
    if variant == 'fitted':
        return b'' if inserted or ordinal % 2 else None
    return bytes((position * 31 + (226 if inserted else ordinal)) % 256
                 for position in range(84))


def record(identity, binary):
    body = b'\x02\x00' + identity.to_bytes(4, 'little', signed=True)
    body += b'' if binary is None else binary
    return (body + len(body).to_bytes(2, 'little') + b'\x06\x00\x01\x00'
            + bytes([1 if binary is None else 3]))


def parsed(text):
    seen = set()
    for line in text.splitlines():
        fields = line.split('\t')
        if fields[0] in ('TABLE', 'COLUMN', 'ROW', 'RAWROW', 'INDEX'):
            width = 2 if fields[0] == 'TABLE' else 3
            identity = tuple(fields[:width])
            assert identity not in seen, ('Duplicate getter item', identity)
            seen.add(identity)
        if fields[0] == 'COLUMN':
            assert fields[7] in ('true', 'false')
        if fields[0] == 'INDEX':
            assert all(value in ('true', 'false') for value in fields[7:11])
        if fields[0] == 'INDEX_COLUMN':
            assert fields[-1] in ('true', 'false')
    tables = facts.parse(text)
    assert list(tables) == ['BinaryStorage']
    return tables['BinaryStorage']


def schema(raw, table, count):
    assert table['count'] == len(table['rows']) == len(table['raw_rows']) == count
    assert table['columns_count'] == table['max_columns'] == 2
    assert list(table['columns']) == ['ID', 'Bytes']
    assert table['properties'] == [['ID', 'Required', '1', 'true', 'Boolean:true']]
    assert list(table['indexes']) == ['PrimaryID']
    index = table['indexes']['PrimaryID']
    assert index['physical'] == 0 and index['entries'] == count
    assert index['primary'] and index['unique'] and index['required']
    assert not index['ignored_null'] and index['columns'] == [['ID', 'true']]
    base = table['definition'] * PAGE
    assert number(raw, base + 16) == count
    assert number(raw, base + 41, 2) == number(raw, base + 45, 2) == 2
    assert number(raw, base + 43, 2) == 1
    assert number(raw, base + 47) == number(raw, base + 51) == 1
    fields = base + 75
    for position, (kind, width, flags) in enumerate([(4, 4, 3), (9, 255, 2)]):
        at = fields + position * 25
        assert raw[at] == kind and number(raw, at + 5, 2) == position
        assert raw[at + 15] == flags and raw[at + 16] == 0
        assert number(raw, at + 23, 2) == width
        if position == 0:
            assert number(raw, at + 21, 2) == 0
        else:
            assert number(raw, at + 7, 2) == 0
    at = fields + 50
    for name in ['ID', 'Bytes']:
        length = number(raw, at, 2)
        assert raw[at + 2:at + 2 + length].decode('utf-16le') == name
        at += 2 + length
    assert raw[at + 46] == 0x89
    assert number(raw, at + 4, 2) == 0 and raw[at + 6] == 1
    assert all(number(raw, at + 4 + slot * 3, 2) == 0xffff for slot in range(1, 10))
    assert number(raw, at + 38) == index['root']
    return {'definition': table['definition'], 'root': index['root'],
            'owned': number(raw, base + 55), 'hint': number(raw, base + 59),
            'index_owned': number(raw, at + 34), 'global': 1 << 8,
            'table_count': base + 16, 'history': base + 67}


def maps(raw, text, layout, selected_id, table):
    lines = text.splitlines()
    location = [line.split('\t')[1:] for line in lines if line.startswith('LOCATION\t')]
    assert len(location) == 1
    row = table['rows'][selected_id]
    assert list(map(int, location[0])) == [row['page'], row['slot']] * 2
    data = MAPS.parse('\n'.join(line for line in lines if not line.startswith('LOCATION\t')))
    assert data['page_bytes'] == PAGE and data['page_count'] == len(raw) // PAGE
    assert set(data['maps']) == {'JackcessGlobalFree', 'TableOwned', 'TableFreeSpace'}
    result = {}
    for label, key in [('JackcessGlobalFree', 'global'), ('TableOwned', 'owned'),
                       ('TableFreeSpace', 'hint')]:
        decoded = MAPS.decode_map(raw, data['maps'][label], PAGE, len(raw) // PAGE)
        own = CODEC.map_record(raw, layout[key])
        assert decoded['row_start'] == own['range'][0]
        assert set(decoded['file_members']) == {n for n in own['members'] if n < len(raw) // PAGE}
        result[key] = own
    result['index_owned'] = CODEC.map_record(raw, layout['index_owned'])
    return result


def inspect(case, path, getter_path, maps_path, after=False):
    raw = path.read_bytes()
    assert 0 < len(raw) <= 64 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[:4] == bytes([0, 1, 0, 0]) and raw[4:20] == b'Standard ACE DB\x00'
    assert raw[20] == (5 if case['profile'] == 'V2016' else 6)
    assert raw[21:24] == bytes(3) and raw[62:66] == bytes.fromhex('fb8abc4e')
    table = parsed(getter_path.read_text())
    keys = KEYS + ([226] if after else [])
    layout = schema(raw, table, len(keys))
    assert set(table['rows']) == set(keys)
    references = {}
    populations = {}
    for ordinal, identity in enumerate(keys):
        row = table['rows'][identity]
        assert 0 <= row['slot'] < 255
        reference = (row['page'] << 8) | row['slot']
        assert reference not in references
        references[reference] = identity
        binary = payload(case['variant'], ordinal, identity == 226)
        wanted = record(identity, binary)
        assert bytes.fromhex(table['raw_rows'][identity]) == wanted
        begin, end = CODEC.frame(raw, row['page'], row['slot'])
        assert raw[begin:end] == wanted
        assert number(raw, row['page'] * PAGE + 4) == layout['definition']
        assert row['values'] == ['Integer:' + str(identity),
                                 'NULL' if binary is None else 'Binary:' + binary.hex()]
        populations.setdefault(row['page'], []).append(row['slot'])
    for page, slots in populations.items():
        base = page * PAGE
        assert sorted(slots) == list(range(len(slots)))
        assert number(raw, base + 12, 2) == len(slots)
        last = number(raw, base + 14 + 2 * (len(slots) - 1), 2)
        assert last & ~0x1fff == 0
        assert number(raw, base + 2, 2) == last - (14 + 2 * len(slots))
    nodes, leaves, entries = INDEX.tree(raw, layout['root'], layout['definition'])
    expected = sorted(CODEC.int_key(identity, reference) for reference, identity in references.items())
    assert entries == expected
    assert table['indexes']['PrimaryID']['order'] == sorted(keys)
    assert len(nodes) == table['indexes']['PrimaryID']['owned_pages']
    storage = maps(raw, maps_path.read_text(), layout, 226 if after else 1, table)
    eof = len(raw) // PAGE
    assert {n for n in storage['owned']['members'] if n < eof} == set(populations)
    assert storage['index_owned']['members'] == set(nodes)
    assert storage['hint']['members'] <= storage['owned']['members']
    assert not storage['global']['members'] & (set(populations) | set(nodes))
    if not after:
        root = nodes[layout['root']]
        assert len(nodes) == len(leaves) == 1
        assert root['height'] == 0 and root['prefix'] == 1 and root['free'] == 7
        assert (root['previous'], root['next'], root['tail']) == (0, 0, 0)
        assert sorted(map(len, populations.values())) == sorted(case['expected_source']['data_populations'])
        assert number(raw, layout['history']) == 451
        if case['variant'] == 'full84':
            assert all(number(raw, page * PAGE + 2, 2) == 23 for page in populations)
        else:
            highest = max(storage['hint']['members'])
            assert highest in populations and len(populations[highest]) == 196
            assert number(raw, highest * PAGE + 2, 2) >= 15
        for number_ in range(eof, eof + (3 if case['variant'] == 'full84' else 2)):
            assert number_ in storage['global']['locations'] and number_ in storage['global']['members']
            assert number_ in storage['index_owned']['locations']
            if case['variant'] == 'full84':
                assert number_ in storage['owned']['locations'] and number_ in storage['hint']['locations']
    return raw, table, layout, storage, nodes, leaves


def describe(state):
    raw, table, layout, storage, nodes, leaves = state
    return {'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
            'rows': table['count'], 'layout': layout,
            'data_pages': sorted({row['page'] for row in table['rows'].values()}),
            'nodes': {str(n): {k: ([x.hex() for x in v] if k == 'entries' else v)
                             for k, v in node.items()} for n, node in nodes.items()},
            'leaves': leaves,
            'maps': {key: {'reference': value['reference'], 'range': list(value['range']),
                           'kind': value['kind'], 'members': sorted(value['members'])}
                     for key, value in storage.items()}}


def transition(case, before, after):
    original, old, layout, maps_before, nodes_before, _ = before
    raw, table, actual_layout, maps_after, nodes, leaves = after
    assert actual_layout == layout
    assert nodes[layout['root']]['height'] == 1 and len(leaves) == 2
    assert len(nodes) == 3 and layout['root'] in nodes
    for identity, row in old['rows'].items():
        assert table['rows'][identity] == row
        assert table['raw_rows'][identity] == old['raw_rows'][identity]
    assert table['columns'] == old['columns'] and table['properties'] == old['properties']
    assert number(raw, layout['history']) == number(original, layout['history']) + 1 == 452
    new_row = table['rows'][226]
    old_data = {row['page'] for row in old['rows'].values()}
    new_data = {row['page'] for row in table['rows'].values()} - old_data
    new_indexes = set(nodes) - set(nodes_before)
    eof = len(original) // PAGE
    expected_data = 1 if case['variant'] == 'full84' else 0
    assert len(new_data) == expected_data and len(new_indexes) == 2
    assert len(raw) - len(original) == (expected_data + 2) * PAGE
    assert new_data | new_indexes == set(range(eof, len(raw) // PAGE))
    if not expected_data:
        assert new_row['page'] == max(maps_before['hint']['members'])
    expected_bytes = bytearray(original)
    changes = {}
    desired = {'owned': maps_before['owned']['members'] | new_data,
               'index_owned': maps_before['index_owned']['members'] | new_indexes,
               'global': maps_before['global']['members'] - new_data - new_indexes}
    if expected_data:
        desired['hint'] = (maps_before['hint']['members']
                           - {max(maps_before['hint']['members'])}) | new_data
    else:
        desired['hint'] = maps_before['hint']['members']
    for key, wanted in desired.items():
        before_map, after_map = maps_before[key], maps_after[key]
        assert before_map['range'] == after_map['range'] and before_map['kind'] == after_map['kind']
        assert after_map['members'] == wanted
        for number_ in before_map['members'] ^ wanted:
            offset, mask = before_map['locations'][number_]
            value = changes.get(offset, expected_bytes[offset])
            changes[offset] = value | mask if number_ in wanted else value & ~mask
    for offset, value in changes.items():
        expected_bytes[offset] = value
    for key in ['table_count', 'history']:
        at = layout[key]
        expected_bytes[at:at + 4] = (452).to_bytes(4, 'little')
    allowed_pages = {layout['root']}
    if new_row['page'] < eof:
        allowed_pages.add(new_row['page'])
    for page in allowed_pages:
        expected_bytes[page * PAGE:(page + 1) * PAGE] = raw[page * PAGE:(page + 1) * PAGE]
    assert raw[:len(original)] == expected_bytes, 'Unexpected old-byte rewrite outside explicit active pages/map cells/counters'
    return {'verified': True, 'old_rows_preserved': 451, 'new_reference': new_row['page'] << 8 | new_row['slot'],
            'data_pages_added': sorted(new_data), 'index_pages_added': sorted(new_indexes),
            'suffix_roles': {str(n): 'DATA' if n in new_data else 'INDEX'
                             for n in range(eof, len(raw) // PAGE)},
            'map_byte_effects': [{'offset': n, 'before': original[n], 'after': v}
                                 for n, v in sorted(changes.items())],
            'protected_original_bytes': len(original) - len(allowed_pages) * PAGE - 8 - len(changes),
            'public_root_and_leaf_framing': describe(after)['nodes'],
            'padding_policy': 'Observed raw pages only; no candidate padding/prefix golden inferred'}
