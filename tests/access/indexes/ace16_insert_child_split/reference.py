"""Closed public child-split observations, independent of engineering images."""
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_root_split import reference as root_reference
PAGE = 4096
EXTRA = [2147483647 - position * 65536 for position in range(225)]
ORIGINAL = list(range(-226, 0)) + list(range(1, 227))


def payload(variant, identity):
    if identity in EXTRA or identity == 227:
        if variant == 'fitted':
            return b''
        return bytes((position * 31 + identity) % 256 for position in range(84))
    ordinal = ORIGINAL.index(identity)
    return root_reference.payload(variant, ordinal, identity == 226)


def inspect(case, path, getter, maps, stage):
    assert stage in ('before', 'after')
    raw = path.read_bytes()
    assert 0 < len(raw) <= 64 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[:4] == bytes([0, 1, 0, 0])
    assert raw[4:20] == b'Standard ACE DB\x00'
    assert raw[20] == (5 if case['profile'] == 'V2016' else 6)
    assert raw[21:24] == bytes(3) and raw[62:66] == bytes.fromhex('fb8abc4e')
    table = root_reference.parsed(getter.read_text())
    identities = ORIGINAL + EXTRA + ([227] if stage == 'after' else [])
    assert len(identities) == len(set(identities)) == (678 if stage == 'after' else 677)
    assert set(table['rows']) == set(identities)
    layout = root_reference.schema(raw, table, len(identities))
    assert root_reference.number(raw, layout['history']) == len(identities)
    references = {}
    populations = {}
    for identity in identities:
        row = table['rows'][identity]
        assert 0 <= row['slot'] < 255
        reference = row['page'] << 8 | row['slot']
        assert reference not in references
        references[reference] = identity
        binary = payload(case['variant'], identity)
        wanted = root_reference.record(identity, binary)
        assert bytes.fromhex(table['raw_rows'][identity]) == wanted
        begin, end = root_reference.CODEC.frame(raw, row['page'], row['slot'])
        assert raw[begin:end] == wanted
        assert root_reference.number(raw, row['page'] * PAGE + 14 + row['slot'] * 2, 2) & ~0x1fff == 0
        assert root_reference.number(raw, row['page'] * PAGE + 4) == layout['definition']
        assert row['values'] == ['Integer:' + str(identity), 'NULL' if binary is None else 'Binary:' + binary.hex()]
        populations.setdefault(row['page'], []).append(row['slot'])
    for number, slots in populations.items():
        assert sorted(slots) == list(range(len(slots)))
        base = number * PAGE
        assert root_reference.number(raw, base + 12, 2) == len(slots)
        last = root_reference.number(raw, base + 14 + 2 * (len(slots) - 1), 2)
        assert root_reference.number(raw, base + 2, 2) == last - 14 - 2 * len(slots)
    nodes, leaves, entries = root_reference.INDEX.tree(raw, layout['root'], layout['definition'])
    expected = sorted(root_reference.CODEC.int_key(identity, reference) for reference, identity in references.items())
    assert entries == expected
    assert table['indexes']['PrimaryID']['order'] == sorted(identities)
    assert table['indexes']['PrimaryID']['owned_pages'] == len(nodes)
    storage = root_reference.maps(raw, maps.read_text(), layout, 227 if stage == 'after' else 1, table)
    eof = len(raw) // PAGE
    assert storage['owned']['members'] & set(range(eof)) == set(populations)
    assert storage['index_owned']['members'] == set(nodes)
    assert storage['hint']['members'] <= storage['owned']['members']
    assert not storage['global']['members'] & (set(populations) | set(nodes))
    root = nodes[layout['root']]
    if stage == 'before':
        assert len(nodes) == 3 and len(leaves) == 2
        assert root['height'] == 1 and len(root['entries']) == len(root['children']) == 1
        assert root['tail'] == leaves[1] and root['children'] == [leaves[0]]
        assert root_reference.INDEX.key(root['entries'][0]) == -1
        assert len(nodes[leaves[0]]['entries']) == 226
        target = nodes[leaves[1]]
        assert target['height'] == 0 and len(target['entries']) == 451
        assert target['prefix'] == 1 and target['free'] == 7
        assert root_reference.INDEX.best_size(target['entries'], False) == 3609
        for number in [eof, eof + 1]:
            assert number in storage['global']['members']
            for role in ['global', 'owned', 'hint', 'index_owned']:
                assert number in storage[role]['locations']
    return raw, table, layout, storage, nodes, leaves


def original_preserved(case, before):
    raw, table, _, _, _, _ = before
    original = root_reference.parsed((ROOT / case['original_getter']).read_text())
    assert original['count'] == len(original['rows']) == 452
    for identity in ORIGINAL:
        assert table['rows'][identity] == original['rows'][identity]
        assert table['raw_rows'][identity] == original['raw_rows'][identity]
    assert table['columns'] == original['columns']
    assert table['properties'] == original['properties']
    return {'verified': True, 'preserved_original_rows': 452, 'prepared_rows': 677,
            'prepared_sha256': hashlib.sha256(raw).hexdigest()}


def describe(state):
    return root_reference.describe(state)


def transition(case, before, after):
    original, old, layout, maps_before, nodes_before, old_leaves = before
    raw, table, actual_layout, maps_after, nodes, leaves = after
    assert layout == actual_layout
    root = nodes[layout['root']]
    assert root['height'] == 1 and len(root['entries']) == len(root['children']) == 2
    assert len(nodes) == 4 and len(leaves) == 3
    added_index = set(nodes) - set(nodes_before)
    assert len(added_index) == 1
    new_left = next(iter(added_index))
    assert leaves == [old_leaves[0], new_left, old_leaves[1]]
    assert root['children'] == leaves[:2] and root['tail'] == old_leaves[1]
    for identity, row in old['rows'].items():
        assert table['rows'][identity] == row
        assert table['raw_rows'][identity] == old['raw_rows'][identity]
    assert table['columns'] == old['columns'] and table['properties'] == old['properties']
    assert nodes[old_leaves[0]]['entries'] == nodes_before[old_leaves[0]]['entries']
    right_pairs = sorted(nodes_before[old_leaves[1]]['entries'] + [
        root_reference.CODEC.int_key(227, table['rows'][227]['page'] << 8 | table['rows'][227]['slot'])
    ])
    assert len(right_pairs) == 452
    assert root_reference.INDEX.best_size(right_pairs, False) == 3617
    assert nodes[new_left]['entries'] == right_pairs[:226]
    assert nodes[old_leaves[1]]['entries'] == right_pairs[226:]
    assert root['entries'] == [nodes_before[layout['root']]['entries'][0], right_pairs[225]]
    old_data = {row['page'] for row in old['rows'].values()}
    new_data = {row['page'] for row in table['rows'].values()} - old_data
    assert len(new_data) <= 1
    eof = len(original) // PAGE
    assert added_index | new_data == set(range(eof, len(raw) // PAGE))
    selected = table['rows'][227]
    selected_page = selected['page']
    expected_bytes = bytearray(original)
    changes = {}
    highest = max(maps_before['hint']['members'])
    wanted_hint = maps_before['hint']['members']
    if new_data:
        wanted_hint = (wanted_hint - {highest}) | new_data
    else:
        assert selected_page == highest
    desired = {'owned': maps_before['owned']['members'] | new_data,
               'index_owned': maps_before['index_owned']['members'] | added_index,
               'global': maps_before['global']['members'] - new_data - added_index,
               'hint': wanted_hint}
    for role, wanted in desired.items():
        before_map, after_map = maps_before[role], maps_after[role]
        assert before_map['kind'] == after_map['kind'] and before_map['range'] == after_map['range']
        assert after_map['members'] == wanted
        for number in before_map['members'] ^ wanted:
            offset, mask = before_map['locations'][number]
            value = changes.get(offset, expected_bytes[offset])
            changes[offset] = value | mask if number in wanted else value & ~mask
    for offset, value in changes.items():
        expected_bytes[offset] = value
    for role in ['table_count', 'history']:
        offset = layout[role]
        assert root_reference.number(original, offset) == 677 and root_reference.number(raw, offset) == 678
        expected_bytes[offset:offset + 4] = (678).to_bytes(4, 'little')
    # The peer receives only the mandatory next-sibling pointer change.
    peer = old_leaves[0]
    expected_bytes[peer * PAGE + 16:peer * PAGE + 20] = new_left.to_bytes(4, 'little')
    if selected_page < eof:
        before_page = original[selected_page * PAGE:(selected_page + 1) * PAGE]
        data = bytearray(before_page)
        count = root_reference.number(data, 12, 2)
        assert selected['slot'] == count < 255
        end = root_reference.number(data, 14 + 2 * (count - 1), 2)
        body = root_reference.record(227, payload(case['variant'], 227))
        start = end - len(body)
        assert start >= 14 + 2 * (count + 1)
        data[start:end] = body
        data[14 + 2 * count:16 + 2 * count] = start.to_bytes(2, 'little')
        data[12:14] = (count + 1).to_bytes(2, 'little')
        data[2:4] = (start - 14 - 2 * (count + 1)).to_bytes(2, 'little')
        assert raw[selected_page * PAGE:(selected_page + 1) * PAGE] == data
        expected_bytes[selected_page * PAGE:(selected_page + 1) * PAGE] = data
    # Public root/target compression and scratch padding are observed, not
    # borrowed as an engineering image oracle. Their decoded wires are exact.
    for page in [layout['root'], old_leaves[1]]:
        expected_bytes[page * PAGE:(page + 1) * PAGE] = raw[page * PAGE:(page + 1) * PAGE]
    assert raw[:len(original)] == expected_bytes
    return {'verified': True, 'old_rows_preserved': 677, 'new_rows': 1,
            'new_data_pages': sorted(new_data), 'new_index_pages': sorted(added_index),
            'suffix_roles': {str(page): 'DATA' if page in new_data else 'INDEX'
                             for page in range(eof, len(raw) // PAGE)},
            'map_byte_effects': [{'offset': offset, 'before': original[offset], 'after': value}
                                 for offset, value in sorted(changes.items())],
            'public_geometry': describe(after)['nodes'],
            'padding_policy': 'Observe public index active framing and raw scratch; never adapt engineering goldens.'}
