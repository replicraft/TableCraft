"""Closed public multi-child INSERT source and transition checks, never goldens."""
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_child_split import reference as child_reference
from ace16_insert_root_split import reference as root_reference

PAGE = 4096
BASE = child_reference.ORIGINAL + child_reference.EXTRA + [227]
NEGATIVE = [-2147483648 + position * 65536 for position in range(225)]
MIDPOINT = [2147483647 - position * 65536 - 32768 for position in range(225)]
PREPARED = BASE + NEGATIVE + MIDPOINT
REQUESTS = [-227, 228]


def payload(variant, identity):
    if identity in NEGATIVE or identity in MIDPOINT or identity in REQUESTS:
        if variant == 'fitted':
            return b''
        return bytes((position * 31 + identity) % 256 for position in range(84))
    return child_reference.payload(variant, identity)


def inspect(case, path, getter, maps, stage):
    assert stage in ('before', 'after')
    raw = path.read_bytes()
    assert 0 < len(raw) <= 64 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[:4] == bytes([0, 1, 0, 0]) and raw[4:20] == b'Standard ACE DB\x00'
    assert raw[20] == (5 if case['profile'] == 'V2016' else 6)
    assert raw[21:24] == bytes(3) and raw[62:66] == bytes.fromhex('fb8abc4e')
    table = root_reference.parsed(getter.read_text())
    identities = PREPARED + (REQUESTS if stage == 'after' else [])
    assert len(identities) == len(set(identities)) == (1130 if stage == 'after' else 1128)
    assert set(table['rows']) == set(identities)
    layout = root_reference.schema(raw, table, len(identities))
    assert root_reference.number(raw, layout['history']) == len(identities)
    references = {}
    populations = {}
    for identity in identities:
        row = table['rows'][identity]
        assert 0 <= row['slot'] < 255
        row_reference = row['page'] << 8 | row['slot']
        assert row_reference not in references
        references[row_reference] = identity
        binary = payload(case['variant'], identity)
        wanted = root_reference.record(identity, binary)
        assert bytes.fromhex(table['raw_rows'][identity]) == wanted
        begin, end = root_reference.CODEC.frame(raw, row['page'], row['slot'])
        assert raw[begin:end] == wanted
        assert root_reference.number(raw, row['page'] * PAGE + 14 + row['slot'] * 2, 2) & ~0x1fff == 0
        assert root_reference.number(raw, row['page'] * PAGE + 4) == layout['definition']
        assert row['values'] == ['Integer:' + str(identity),
                                 'NULL' if binary is None else 'Binary:' + binary.hex()]
        populations.setdefault(row['page'], []).append(row['slot'])
    for page, slots in populations.items():
        assert sorted(slots) == list(range(len(slots)))
        base = page * PAGE
        assert root_reference.number(raw, base + 12, 2) == len(slots)
        last = root_reference.number(raw, base + 14 + 2 * (len(slots) - 1), 2)
        assert root_reference.number(raw, base + 2, 2) == last - 14 - 2 * len(slots)
    nodes, leaves, entries = root_reference.INDEX.tree(raw, layout['root'], layout['definition'])
    expected = sorted(root_reference.CODEC.int_key(identity, row_reference)
                      for row_reference, identity in references.items())
    assert entries == expected
    assert table['indexes']['PrimaryID']['order'] == sorted(identities)
    assert table['indexes']['PrimaryID']['owned_pages'] == len(nodes)
    storage = root_reference.maps(raw, maps.read_text(), layout,
                                  228 if stage == 'after' else 1, table)
    eof = len(raw) // PAGE
    assert storage['owned']['members'] & set(range(eof)) == set(populations)
    assert storage['index_owned']['members'] == set(nodes)
    assert storage['hint']['members'] <= storage['owned']['members']
    assert not storage['global']['members'] & (set(populations) | set(nodes))
    root = nodes[layout['root']]
    if stage == 'before':
        assert len(nodes) == 4 and len(leaves) == 3
        assert root['height'] == 1 and len(root['entries']) == len(root['children']) == 2
        assert root['tail'] == leaves[2] and root['children'] == leaves[:2]
        assert [root_reference.INDEX.key(entry) for entry in root['entries']] == [-1, 226]
        assert [len(nodes[leaf]['entries']) for leaf in leaves] == [451, 226, 451]
        for leaf in [leaves[0], leaves[2]]:
            target = nodes[leaf]
            assert target['height'] == 0 and target['prefix'] == 1 and target['free'] == 7
            assert root_reference.INDEX.best_size(target['entries'], False) == 3609
        for page in range(eof, eof + 4):
            assert page in storage['global']['members']
            for role in ['global', 'owned', 'hint', 'index_owned']:
                assert page in storage[role]['locations']
    return raw, table, layout, storage, nodes, leaves


def original_preserved(case, before):
    raw, table, layout, storage, nodes, leaves = before
    old_raw = (ROOT / case['original']).read_bytes()
    old_table = root_reference.parsed((ROOT / case['original_getter']).read_text())
    assert old_table['count'] == 678
    for identity in BASE:
        assert table['rows'][identity] == old_table['rows'][identity]
        assert table['raw_rows'][identity] == old_table['raw_rows'][identity]
    assert table['columns'] == old_table['columns']
    assert table['properties'] == old_table['properties']
    old_nodes, old_leaves, _ = root_reference.INDEX.tree(old_raw, layout['root'], layout['definition'])
    assert leaves == old_leaves
    assert raw[leaves[1] * PAGE:(leaves[1] + 1) * PAGE] == old_raw[leaves[1] * PAGE:(leaves[1] + 1) * PAGE]
    assert nodes[leaves[1]]['entries'] == old_nodes[leaves[1]]['entries']
    return {'verified': True, 'preserved_original_rows': 678, 'prepared_rows': 1128,
            'middle_leaf_unchanged': leaves[1],
            'prepared_sha256': hashlib.sha256(raw).hexdigest()}


def describe(state):
    return root_reference.describe(state)


def transition(case, before, after):
    original, old, layout, maps_before, nodes_before, old_leaves = before
    raw, table, actual_layout, maps_after, nodes, leaves = after
    assert layout == actual_layout
    root = nodes[layout['root']]
    assert root['height'] == 1 and len(root['entries']) == len(root['children']) == 4
    assert len(nodes) == 6 and len(leaves) == 5
    assert leaves[1] == old_leaves[0] and leaves[2] == old_leaves[1] and leaves[4] == old_leaves[2]
    added_index = set(nodes) - set(nodes_before)
    assert added_index == {leaves[0], leaves[3]}
    assert root['children'] == leaves[:4] and root['tail'] == old_leaves[2]
    for identity, row in old['rows'].items():
        assert table['rows'][identity] == row
        assert table['raw_rows'][identity] == old['raw_rows'][identity]
    assert table['columns'] == old['columns'] and table['properties'] == old['properties']
    assert nodes[old_leaves[1]]['entries'] == nodes_before[old_leaves[1]]['entries']
    for old_leaf, fresh, identity in [(old_leaves[0], leaves[0], -227),
                                       (old_leaves[2], leaves[3], 228)]:
        row = table['rows'][identity]
        pairs = sorted(nodes_before[old_leaf]['entries'] + [
            root_reference.CODEC.int_key(identity, row['page'] << 8 | row['slot'])
        ])
        assert len(pairs) == 452
        assert root_reference.INDEX.best_size(pairs, False) == 3617
        assert nodes[fresh]['entries'] == pairs[:226]
        assert nodes[old_leaf]['entries'] == pairs[226:]
    assert all(len(nodes[leaf]['entries']) == 226 for leaf in leaves)
    old_data = {row['page'] for row in old['rows'].values()}
    new_data = {row['page'] for row in table['rows'].values()} - old_data
    assert len(new_data) <= 2
    eof = len(original) // PAGE
    assert added_index | new_data == set(range(eof, len(raw) // PAGE))
    expected_bytes = bytearray(original)
    data_pages = {}
    hints = set(maps_before['hint']['members'])
    created_data = set()
    for identity in REQUESTS:
        row = table['rows'][identity]
        body = root_reference.record(identity, payload(case['variant'], identity))
        highest = max(hints)
        if highest in data_pages:
            current = data_pages[highest]
        else:
            current = bytearray(original[highest * PAGE:(highest + 1) * PAGE])
        count = root_reference.number(current, 12, 2)
        free = root_reference.number(current, 2, 2)
        if count >= 255 or free < len(body) + 2:
            hints.remove(highest)
            assert row['page'] in new_data - created_data and row['slot'] == 0
            created_data.add(row['page'])
            hints.add(row['page'])
            # The established fresh DATA profile has a zero-initialized frame.
            # Its complete body/header is checked independently; public INDEX
            # scratch remains observed rather than an engineering image input.
            current = bytearray(PAGE)
            current[:2] = bytes([1, 1])
            current[4:8] = layout['definition'].to_bytes(4, 'little')
            count = 0
        else:
            assert row['page'] == highest and row['slot'] == count
        end = PAGE if count == 0 else root_reference.number(current, 14 + 2 * (count - 1), 2)
        start = end - len(body)
        assert start >= 14 + 2 * (count + 1)
        current[start:end] = body
        current[14 + 2 * count:16 + 2 * count] = start.to_bytes(2, 'little')
        current[12:14] = (count + 1).to_bytes(2, 'little')
        current[2:4] = (start - 14 - 2 * (count + 1)).to_bytes(2, 'little')
        data_pages[row['page']] = current
    assert created_data == new_data
    for page, data in data_pages.items():
        assert raw[page * PAGE:(page + 1) * PAGE] == data
        if page < eof:
            expected_bytes[page * PAGE:(page + 1) * PAGE] = data
    desired = {'owned': maps_before['owned']['members'] | new_data,
               'index_owned': maps_before['index_owned']['members'] | added_index,
               'global': maps_before['global']['members'] - new_data - added_index,
               'hint': hints}
    changes = {}
    for role, wanted in desired.items():
        before_map, after_map = maps_before[role], maps_after[role]
        assert before_map['kind'] == after_map['kind'] and before_map['range'] == after_map['range']
        assert after_map['members'] == wanted
        for page in before_map['members'] ^ wanted:
            offset, mask = before_map['locations'][page]
            value = changes.get(offset, expected_bytes[offset])
            changes[offset] = value | mask if page in wanted else value & ~mask
    for offset, value in changes.items():
        expected_bytes[offset] = value
    for role in ['table_count', 'history']:
        offset = layout[role]
        assert root_reference.number(original, offset) == 1128 and root_reference.number(raw, offset) == 1130
        expected_bytes[offset:offset + 4] = (1130).to_bytes(4, 'little')
    middle = old_leaves[1]
    expected_bytes[middle * PAGE + 16:middle * PAGE + 20] = leaves[3].to_bytes(4, 'little')
    for page in [layout['root'], old_leaves[0], old_leaves[2]]:
        expected_bytes[page * PAGE:(page + 1) * PAGE] = raw[page * PAGE:(page + 1) * PAGE]
    assert raw[:len(original)] == expected_bytes
    return {'verified': True, 'old_rows_preserved': 1128, 'new_rows': 2,
            'new_data_pages': sorted(new_data), 'new_index_pages': sorted(added_index),
            'suffix_roles': {str(page): 'DATA' if page in new_data else 'INDEX'
                             for page in range(eof, len(raw) // PAGE)},
            'map_byte_effects': [{'offset': offset, 'before': original[offset], 'after': value}
                                 for offset, value in sorted(changes.items())],
            'public_geometry': describe(after)['nodes'],
            'padding_policy': 'Public active framing and scratch observed; no engineering golden adaptation.'}
