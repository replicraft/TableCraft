"""Pure ignored-NULL source/reference qualification; no candidate authority."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import facts
from ace16_insert_candidates import model as wire
import delete_reference_maps as maps_probe

PAGE = 4096
NAMES = ['PrimaryID', 'PrimaryAlias', 'GroupOrder', 'GroupAlias']


def n(raw, at, width=4):
    data = raw[at:at + width]
    assert len(data) == width
    return int.from_bytes(data, 'little')


def source_rows(variant):
    if variant in ('mixed', 'empty', 'empty-first-nonnull'):
        empty = variant != 'mixed'
        return [(1, None if empty else 10, None),
                (2, None if empty else 10, b''), (3, None, bytes([128, 0, 255]))]
    if variant == 'all-null-node':
        return [(identity, identity - 251, b'') for identity in range(1, 501)]
    assert variant == 'both-root-pressure'
    return [(identity, None if identity == -200 else identity, b'')
            for identity in list(range(-200, 0)) + list(range(1, 201))]


def requests(variant):
    if variant == 'mixed':
        return [(4, 10, b''), (5, 20, b''), (6, 20, b''), (7, None, b'')]
    if variant == 'empty':
        return [(4, None, b''), (5, None, b'')]
    if variant == 'empty-first-nonnull':
        return [(4, 30, b''), (5, None, b'')]
    if variant == 'all-null-node':
        return [(501, None, b''), (502, None, b'')]
    assert variant == 'both-root-pressure'
    return [(identity, identity, b'') for identity in range(201, 255)] + [
        (255, None, b''), (256, None, b'')]


def record(row):
    identity, group, binary = row
    body = b'\x03\x00' + identity.to_bytes(4, 'little', signed=True)
    body += (group or 0).to_bytes(4, 'little', signed=True)
    body += b'' if binary is None else binary
    return body + len(body).to_bytes(2, 'little') + b'\x0a\x00\x01\x00' + bytes([1 | (group is not None) * 2 | (binary is not None) * 4])


def key(group, reference):
    payload = b'\x00' if group is None else b'\x7f' + ((group & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
    return payload + reference.to_bytes(4, 'big')


def tree(raw, root, owner):
    nodes = {}
    levels = {}
    leaves = []

    def visit(number, expected_height=None):
        assert number not in nodes and 0 < number < len(raw) // PAGE
        page = raw[number * PAGE:(number + 1) * PAGE]
        assert page[0] in (3, 4) and page[1] == 1 and n(page, 4) == owner
        height = page[26]
        assert height < 16 and (expected_height is None or height == expected_height)
        assert (page[0] == 4) == (height == 0)
        prefix = n(page, 24, 2)
        ends = [position for position in range(1, 3617) if page[27 + position // 8] & (1 << (position % 8))]
        assert not page[27] & 1 and not page[479] & 0xfe
        active = 3616 - n(page, 2, 2)
        assert (ends[-1] if ends else 0) == active
        previous = b''
        entries = []
        children = []
        start = 0
        for end in ends:
            payload = page[480 + start:480 + end]
            if height:
                children.append(int.from_bytes(payload[-4:], 'big'))
                payload = payload[:-4]
            complete = payload if not entries else previous[:prefix] + payload
            assert complete[0] in (0, 127) and len(complete) == (5 if complete[0] == 0 else 9)
            assert not entries or previous < complete
            entries.append(complete)
            previous = complete
            start = end
        tail = n(page, 20)
        nodes[number] = {'height': height, 'prefix': prefix, 'active': active,
                         'previous': n(page, 12), 'next': n(page, 16), 'tail': tail,
                         'entries': entries}
        levels.setdefault(height, []).append(number)
        if not height:
            assert not tail
            leaves.append(number)
            return entries
        if tail:
            children.append(tail)
        assert children
        all_entries = []
        for position, child in enumerate(children):
            values = visit(child, height - 1)
            assert values
            if position < len(entries):
                assert entries[position] == values[-1]
            all_entries.extend(values)
        assert all_entries == sorted(all_entries) and len(set(all_entries)) == len(all_entries)
        return all_entries

    entries = visit(root)
    for sequence in levels.values():
        for position, number in enumerate(sequence):
            assert nodes[number]['previous'] == (sequence[position - 1] if position else 0)
            assert nodes[number]['next'] == (sequence[position + 1] if position + 1 < len(sequence) else 0)
    return nodes, leaves, entries


def inspect(case, image, getter, maps, after=False):
    raw = image.read_bytes()
    assert 0 < len(raw) <= 64 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[4:20] == b'Standard ACE DB\x00' and raw[20] == (5 if case['profile'] == 'V2016' else 6)
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
    rows = source_rows(case['variant']) + (requests(case['variant']) if after else [])
    assert table['count'] == len(table['rows']) == len(table['raw_rows']) == len(rows)
    assert table['columns_count'] == table['max_columns'] == 3
    assert list(table['columns']) == ['ID', 'Group', 'Score']
    assert table['properties'] == [['ID', 'Required', '1', 'true', 'Boolean:true']]
    assert list(table['indexes']) == NAMES
    definition = table['definition']
    base = definition * PAGE
    assert n(raw, base + 16) == len(rows) and n(raw, base + 47) == 4 and n(raw, base + 51) == 2
    assert n(raw, base + 41, 2) == n(raw, base + 45, 2) == 3 and n(raw, base + 43, 2) == 1
    fields = base + 87
    for position, (kind, width, offset, flags) in enumerate([(4, 4, 0, 3), (4, 4, 4, 3), (9, 255, 0, 2)]):
        at = fields + 25 * position
        assert raw[at] == kind and n(raw, at + 5, 2) == position
        assert raw[at + 15:at + 17] == bytes([flags, 0]) and n(raw, at + 23, 2) == width
        assert n(raw, at + (7 if position == 2 else 21), 2) == (0 if position == 2 else offset)
    at = fields + 75
    for name in ['ID', 'Group', 'Score']:
        width = n(raw, at, 2)
        assert raw[at + 2:at + 2 + width].decode('utf-16le') == name
        at += width + 2
    physical = []
    for index in range(2):
        block = raw[at + 52 * index:at + 52 * (index + 1)]
        assert n(block, 4, 2) == index and block[6] == 1
        assert all(n(block, 4 + 3 * slot, 2) == 65535 for slot in range(1, 10))
        assert block[46] == (0x89 if index == 0 else 0x82)
        physical.append({'root': n(block, 38), 'usage': n(block, 34), 'history': base + 67 + 12 * index})
    assert physical[0]['root'] != physical[1]['root']
    references = {}
    populations = {}
    for identity, group, binary in rows:
        row = table['rows'][identity]
        reference = row['page'] * 256 + row['slot']
        assert reference not in references
        references[reference] = (identity, group, binary)
        begin, end = wire.frame(raw, row['page'], row['slot'])
        body = raw[begin:end]
        wanted = bytearray(record((identity, group, binary)))
        if group is None:
            wanted[6:10] = body[6:10]  # NULL has no typed fixed payload authority.
        assert body == bytes.fromhex(table['raw_rows'][identity]) == wanted
        assert n(raw, row['page'] * PAGE + 4) == definition
        assert row['values'] == ['Integer:' + str(identity), 'NULL' if group is None else 'Integer:' + str(group), 'NULL' if binary is None else 'Binary:' + binary.hex()]
        populations.setdefault(row['page'], []).append(row['slot'])
    for page, slots in populations.items():
        assert sorted(slots) == list(range(len(slots)))
        assert n(raw, page * PAGE + 12, 2) == len(slots)
        last = n(raw, page * PAGE + 14 + 2 * (len(slots) - 1), 2)
        assert n(raw, page * PAGE + 2, 2) == last - 14 - 2 * len(slots)
    storage = {'owned': wire.map_record(raw, n(raw, base + 55)), 'hint': wire.map_record(raw, n(raw, base + 59)), 'global': wire.map_record(raw, 256)}
    trees = []
    for index, metadata in enumerate(physical):
        nodes, leaves, entries = tree(raw, metadata['root'], definition)
        expected = sorted(key(values[index], reference) for reference, values in references.items()
                          if index == 0 or values[index] is not None)
        assert entries == expected
        for logical in NAMES[index * 2:index * 2 + 2]:
            observed = table['indexes'][logical]
            assert observed['physical'] == index and observed['root'] == metadata['root']
            assert observed['entries'] == len(expected) and observed['owned_pages'] == len(nodes)
            assert observed['unique'] == observed['required'] == (index == 0) and observed['ignored_null'] == (index == 1)
            assert observed['primary'] == (logical == 'PrimaryID')
            assert observed['columns'] == [[('ID' if index == 0 else 'Group'), 'true']]
            assert observed['order'] == [references[int.from_bytes(entry[-4:], 'big')][0] for entry in entries]
        storage['index' + str(index)] = wire.map_record(raw, metadata['usage'])
        assert storage['index' + str(index)]['members'] == set(nodes)
        if not after:
            if case['variant'] == 'all-null-node' and index == 1:
                assert nodes[metadata['root']]['height'] == 1 and len(leaves) >= 2
            else:
                assert len(nodes) == len(leaves) == 1
            if not entries:
                assert nodes[metadata['root']]['prefix'] == 0
            for node in nodes.values():
                complete = node['entries']
                common = len(complete[0]) if complete else 0
                for entry in complete[1:]:
                    common = next((position for position, pair in enumerate(zip(complete[0][:common], entry))
                                   if pair[0] != pair[1]), common)
                assert node['prefix'] <= common
                physical_width = sum(map(len, complete)) + (4 * len(complete) if node['height'] else 0)
                assert node['active'] == physical_width - node['prefix'] * max(0, len(complete) - 1)
            if case['variant'] == 'both-root-pressure':
                full_common = min((next((position for position, pair in enumerate(zip(entries[0], entry))
                                        if pair[0] != pair[1]), len(entry)) for entry in entries[1:]), default=0)
                assert sum(map(len, entries)) - full_common * (len(entries) - 1) <= 3616
        trees.append(nodes)
    assert not set(trees[0]) & set(trees[1])
    assert storage['index0']['range'] != storage['index1']['range']
    assert storage['hint']['members'] <= storage['owned']['members']
    assert {page for page in storage['owned']['members'] if page < len(raw) // PAGE} == set(populations)
    assert not (set(populations) | set(trees[0]) | set(trees[1])) & storage['global']['members']
    parsed_maps = maps_probe.parse(maps.read_text())
    assert set(parsed_maps['maps']) == {'JackcessGlobalFree', 'TableOwned', 'TableFreeSpace'}
    for name, role in [('JackcessGlobalFree', 'global'), ('TableOwned', 'owned'), ('TableFreeSpace', 'hint')]:
        checked = maps_probe.decode_map(raw, parsed_maps['maps'][name], PAGE, len(raw) // PAGE)
        assert set(checked['file_members']) == {page for page in storage[role]['members'] if page < len(raw) // PAGE}
    if not after:
        assert [n(raw, item['history']) for item in physical] == [len(rows), len({row[1] for row in rows if row[1] is not None})]
    return {'raw': raw, 'table': table, 'physical': physical, 'rows': references, 'maps': storage, 'trees': trees}


def describe(state):
    return {'rows': state['table']['count'], 'physical': state['physical'], 'data_pages': sorted({reference >> 8 for reference in state['rows']}), 'nodes': [sorted(tree) for tree in state['trees']], 'histories': [n(state['raw'], item['history']) for item in state['physical']], 'maps': {role: {'reference': item['reference'], 'range': list(item['range']), 'members': sorted(item['members'])} for role, item in state['maps'].items()}}


def transition(case, before, after):
    assert before['physical'] == after['physical']
    for reference, value in before['rows'].items():
        assert after['rows'][reference] == value
        identity = value[0]
        assert before['table']['rows'][identity] == after['table']['rows'][identity]
        assert before['table']['raw_rows'][identity] == after['table']['raw_rows'][identity]
    assert before['table']['columns'] == after['table']['columns']
    assert before['table']['properties'] == after['table']['properties']
    delta = len(requests(case['variant']))
    old_families = {row[1] for row in source_rows(case['variant']) if row[1] is not None}
    new_families = {row[1] for row in requests(case['variant']) if row[1] is not None}
    counter_deltas = [delta, len(new_families - old_families)]
    if not new_families:
        for page in before['trees'][1]:
            assert after['raw'][page * PAGE:(page + 1) * PAGE] == before['raw'][page * PAGE:(page + 1) * PAGE]
        assert after['maps']['index1'] == before['maps']['index1']
    for item, change in zip(before['physical'], counter_deltas, strict=True):
        assert n(after['raw'], item['history']) == n(before['raw'], item['history']) + change
    if case['variant'] == 'both-root-pressure':
        assert all(tree[item['root']]['height'] == 1 and len(tree) == 3 for tree, item in zip(after['trees'], after['physical'], strict=True))
    old_end = len(before['raw']) // PAGE
    new_end = len(after['raw']) // PAGE
    suffix = set(range(old_end, new_end))
    new_data = {reference >> 8 for reference in after['rows']} - {reference >> 8 for reference in before['rows']}
    new_indexes = set().union(*[set(tree) - set(old) for tree, old in zip(after['trees'], before['trees'], strict=True)])
    assert suffix == new_data | new_indexes and not new_data & new_indexes
    assert after['maps']['owned']['members'] == before['maps']['owned']['members'] | new_data
    for index in range(2):
        role = 'index' + str(index)
        assert after['maps'][role]['members'] == before['maps'][role]['members'] | set(after['trees'][index])
    assert after['maps']['global']['members'] == before['maps']['global']['members'] - suffix
    for page in new_data:
        assert after['raw'][page * PAGE:page * PAGE + 2] == bytes([1, 1])
    for page in new_indexes:
        assert after['raw'][page * PAGE] in (3, 4)
    expected_hint = {page for page in before['maps']['hint']['members'] if n(after['raw'], page * PAGE + 2, 2) >= 19}
    expected_hint |= {page for page in new_data if n(after['raw'], page * PAGE + 2, 2) >= 19}
    assert after['maps']['hint']['members'] == expected_hint
    # Public index scratch is diagnostic, but every other original byte is
    # protected outside exact new DATA frames, counters and derived map cells.
    protected = bytearray(before['raw'])
    definition = before['table']['definition'] * PAGE
    protected[definition + 16:definition + 20] = after['raw'][definition + 16:definition + 20]
    for item in before['physical']:
        at = item['history']
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
    for role, old_map in before['maps'].items():
        new_map = after['maps'][role]
        assert new_map['reference'] == old_map['reference'] and new_map['range'] == old_map['range']
        assert new_map['kind'] == old_map['kind']
        for member in old_map['members'] ^ new_map['members']:
            offset, mask = old_map['locations'][member]
            protected[offset] ^= mask
    assert bytes(protected) == after['raw'][:len(protected)], 'Protected original bytes changed'
    return {'inserted_rows': delta, 'counter_deltas': counter_deltas, 'source_rows_preserved': len(before['rows']), 'new_data': sorted(new_data), 'new_indexes': sorted(new_indexes), 'suffix': sorted(suffix)}
