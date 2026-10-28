"""Public observations from original bytes and requests, never engineering images."""
import hashlib
from pathlib import Path

from ace16_insert_long_value_map_growth import source, requests

PAGE = 4096


def compare(before, path, getter, selected):
    raw = Path(path).read_bytes()
    original = before['raw']
    assert len(original) <= len(raw) <= 8 * 1024 * 1024 and len(raw) % PAGE == 0
    tables, descriptions, properties, absent = source.parse(Path(getter).read_text())
    assert set(tables) == set(before['tables']) and properties == before['properties']
    assert absent == before['absent']
    maps = {}
    for role, description in descriptions.items():
        description['file_members'] = [page for page in description['all_members'] if page < len(raw) // PAGE]
        maps[role] = source.map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
    assert set(maps) == set(before['maps'])
    table = tables['Items']
    old_table = before['tables']['Items']
    rows = requests.rows(selected)
    identities = [row[0] for row in rows]
    assert not set(identities).intersection(old_table['rows'])
    assert table['count'] == len(table['rows']) == 6 + len(rows)
    assert set(table['rows']) == set(old_table['rows']) | set(identities)
    for name, description in tables.items():
        old = before['tables'][name]
        for key in ('definition', 'columns_count', 'max_columns', 'columns'):
            assert description[key] == old[key]
        if name != 'Items':
            assert description == old
    assert table['values'].items() >= old_table['values'].items()
    for identity, row in old_table['rows'].items():
        assert table['rows'][identity] == row

    allowed = bytearray(len(original))

    def allow(start, end):
        assert 0 <= start <= end <= len(original)
        allowed[start:end] = bytes([255]) * (end - start)

    def expected_counter(offset, expected):
        assert source.number(raw, offset, 4) == expected
        allow(offset, offset + 4)

    expected_counter(table['definition'] * PAGE + 16, 6 + len(rows))
    owner = 'Table:' + str(table['definition'])
    new_locations = []
    long_values = {}
    for identity, group, blob, memo in rows:
        assert table['values'][identity] == dict(ID=identity, Group=group, Blob=blob, Memo=memo)
        row = table['rows'][identity]
        frame, chain = source.long_wire.final_frame(raw, row)
        assert len(chain) == 1 and frame['word'] & 0xe000 == 0
        assert (frame['page'], frame['slot']) == (row['target_page'], row['target_slot'])
        body = raw[frame['start']:frame['end']]
        assert body == row['raw'] and source.number(body, 0) == 4
        mask = len(body) - 1
        assert source.number(body, mask - 2) == 2 and body[mask] & 0xf0 == 0
        assert source.number(body, mask - 4) == 10
        assert int.from_bytes(body[2:6], 'little', signed=True) == identity and body[mask] & 1
        assert bool(body[mask] & 2) == (group is not None)
        if group is not None:
            assert int.from_bytes(body[6:10], 'little', signed=True) == group
        new_locations.append(frame)
        for name, value in [('Blob', blob), ('Memo', memo)]:
            decoded = source.long_wire.long_value(raw, body, table['columns'][name])
            logical = decoded['logical']
            if isinstance(logical, dict):
                logical = bytes.fromhex(logical['binary_hex'])
            assert logical == value
            long_values[(identity, name)] = decoded

    def append_regions(frames, role, lval):
        pages = {frame['page'] for frame in frames}
        old_pages = set(before['maps'][role]['file_members'])
        assert set(maps[role]['file_members']) == old_pages | pages
        for frame in frames:
            base = frame['page'] * PAGE
            page = raw[base:base + PAGE]
            assert page[:2] == b'\x01\x01'
            assert page[4:8] == (b'LVAL' if lval else table['definition'].to_bytes(4, 'little'))
            count = source.number(page, 12)
            assert 0 <= frame['slot'] < count <= 255
            end = PAGE if not count else source.number(page, 14 + (count - 1) * 2) & 0x1fff
            assert 14 + count * 2 <= end <= PAGE
            assert source.number(page, 2) == end - 14 - count * 2
            if base < len(original):
                allow(base + 2, base + 4)
                allow(base + 12, base + 14)
                allow(base + 14 + frame['slot'] * 2, base + 16 + frame['slot'] * 2)
                allow(frame['start'], frame['end'])
        return pages - old_pages

    new_data = append_regions(new_locations, owner + ':Owned', False)
    assert set(maps[owner + ':Hint']['file_members']) <= set(maps[owner + ':Owned']['file_members'])
    new_long = {}
    scopes = {}
    new_bitmaps = {}
    map_transitions = {}
    for name in ('Blob', 'Memo'):
        column = 2 if name == 'Blob' else 3
        chunks = [chunk for (identity, member), value in long_values.items()
                  if member == name for chunk in value['chunks']]
        role = owner + ':Column:' + str(column)
        new_long[name] = append_regions(chunks, role + ':Owned', True)
        all_references = {chunk['reference'] for chunk in chunks}
        assert len(all_references) == len(chunks)
        assert set(maps[role + ':Hint']['file_members']) <= set(maps[role + ':Owned']['file_members'])
        all_references |= {chunk['reference'] for (identity, member), value in before['long_values'].items()
                           if member == name for chunk in value['chunks']}
        scopes[name] = source.inventory(raw, maps[role + ':Owned']['file_members'], all_references)
        owned = maps[role + ':Owned']
        hint = maps[role + ':Hint']
        for value in (owned, hint):
            assert value['outside_file_set_bit_count'] == 0
            assert value['row_end'] - value['row_start'] == 69
        bodies = {chunk['page'] for chunk in chunks}
        assert bodies and min(bodies) >= len(original) // PAGE
        minimum = min(owned['file_members'])
        aligned = minimum // 8 * 8
        should_fit = max(owned['file_members']) < aligned + 512
        assert owned['payload_type'] == (0 if should_fit else 1)
        owned_start = owned['row_start']
        if should_fit:
            assert source.number(raw, owned_start + 1, 4) == aligned
            assert owned['bitmap_pages'] == []
        else:
            assert len(owned['bitmap_pages']) == 1
            bitmap = owned['bitmap_pages'][0]
            assert bitmap == min(bodies) + 1
            assert source.number(raw, owned_start + 1, 4) == bitmap
            assert raw[owned_start + 5:owned['row_end']] == bytes(64)
            assert bitmap not in owned['file_members'] and bitmap not in hint['file_members']
            assert bitmap * PAGE >= len(original)
        new_bitmaps[name] = set(owned['bitmap_pages'])
        assert hint['payload_type'] == 0
        assert source.number(raw, hint['row_start'] + 1, 4) == min(bodies) // 8 * 8
        assert hint['file_members'] == [max(bodies)]
        map_transitions[name] = {
            'owned_kind': owned['payload_type'],
            'owned_base': source.number(raw, owned_start + 1, 4) if should_fit else None,
            'owned_members': owned['file_members'], 'bitmap_pages': owned['bitmap_pages'],
            'candidate_base': source.number(raw, hint['row_start'] + 1, 4),
            'candidate_members': hint['file_members'],
        }
        for scope_row in before['scopes'][name]['live']:
            assert raw[scope_row['start']:scope_row['end']] == original[scope_row['start']:scope_row['end']]

    index_scratch = {}
    for physical, column, names in [(0, 'ID', ('PrimaryID', 'PrimaryAlias')),
                                     (1, 'Group', ('GroupOrder', 'GroupAlias'))]:
        index = table['indexes'][names[0]]
        old_index = old_table['indexes'][names[0]]
        for name in names:
            actual = table['indexes'][name]
            old = old_table['indexes'][name]
            for key in ('physical', 'root', 'history_offset', 'flags', 'primary', 'columns'):
                assert actual[key] == old[key]
            assert actual['entries'] == 6 + len(rows)
        root = index['root']
        nodes, leaves, entries = source.index_wire.tree(raw, root, table['definition'])
        assert len(nodes) == len(leaves) == 1
        expected = list(before['trees'][physical]['entries'])
        expected.extend(source.index_wire.key(table['values'][identity][column],
                                               row['page'] << 8 | row['slot'])
                        for identity, row in table['rows'].items() if identity in identities)
        assert entries == sorted(expected)
        expected_rows = [int.from_bytes(entry[-4:], 'big') for entry in entries]
        for name in names:
            assert [page << 8 | slot for identity, page, slot in table['indexes'][name]['rows']] == expected_rows
        old_values = {old_table['values'][identity][column] for identity in old_table['rows']}
        added_values = {table['values'][identity][column] for identity in identities} - old_values
        history = old_index['history'] + len(added_values)
        assert all(table['indexes'][name]['history'] == history for name in names)
        expected_counter(before['trees'][physical]['history_at'], history)
        start = root * PAGE
        # The public serializer owns its selected root's verified framing and
        # unused scratch; this is an observation, not an engineering byte policy.
        allow(start, start + PAGE)
        active_end = start + 480 + nodes[root]['active']
        index_scratch[str(physical)] = sum(left != right for left, right in
                                          zip(original[active_end:start + PAGE], raw[active_end:start + PAGE]))

    assert not new_data, 'The qualified selected DATA page fits this one-row request'
    metadata_pages = new_bitmaps['Blob'] | new_bitmaps['Memo']
    suffix_pages = set(range(len(original) // PAGE, len(raw) // PAGE))
    assert suffix_pages == new_long['Blob'] | new_long['Memo'] | metadata_pages
    assert new_long['Blob'].isdisjoint(new_long['Memo'] | metadata_pages)
    assert new_long['Memo'].isdisjoint(metadata_pages)
    assert new_bitmaps['Blob'].isdisjoint(new_bitmaps['Memo'])
    column_roles = {owner + ':Column:' + str(column) + ':' + suffix
                    for column in (2, 3) for suffix in ('Owned', 'Hint')}
    for role, actual in maps.items():
        old = before['maps'][role]
        for key in ('map_page', 'map_slot', 'row_start', 'row_end'):
            assert actual[key] == old[key]
        if role == 'GlobalFree':
            assert set(actual['file_members']) == set(old['file_members'])
        elif role not in column_roles:
            assert actual['payload_type'] == old['payload_type']
            assert actual['bitmap_pages'] == old['bitmap_pages']
            assert actual['file_members'] == old['file_members']
        if role in column_roles:
            allow(actual['row_start'], actual['row_end'])
    for page in suffix_pages:
        offset, bit = source.map_cell(original, before['map_claims']['GlobalFree'], page)
        allowed[offset] |= bit
    for position, (old, actual) in enumerate(zip(original, raw)):
        assert (old ^ actual) & ~allowed[position] == 0, ('Protected source prefix', position, old, actual)
    declarations = {name: source.declarations.declarations(raw, value) for name, value in tables.items()}
    assert all(value['complete'] for value in declarations.values())
    claims = source.declarations.claims(raw, tables, declarations, maps,
                                        {name: value['indexes'] for name, value in tables.items()})
    census = source.census.prove(raw, tables, declarations, maps, claims, scopes, source.long_wire)
    return {
        'case': selected, 'rows': len(rows), 'source_bytes': len(original), 'after_bytes': len(raw),
        'after_sha256': hashlib.sha256(raw).hexdigest(),
        'new_data_pages': sorted(new_data),
        'new_long_pages': {name: sorted(pages) for name, pages in new_long.items()},
        'owned_bitmap_pages': {name: sorted(pages) for name, pages in new_bitmaps.items()},
        'map_transitions': map_transitions,
        'new_rows': [{'identity': identity, 'page': table['rows'][identity]['page'],
                      'slot': table['rows'][identity]['slot'],
                      'bytes': len(table['rows'][identity]['raw'])} for identity in identities],
        'long_values': {str(identity) + ':' + name: {
            key: value[key] for key in ('kind', 'encoded_bytes', 'descriptor_hex')}
            for (identity, name), value in long_values.items()},
        'index_unused_scratch_changed_bytes': index_scratch,
        'historical_counts': {name: index['history'] for name, index in table['indexes'].items()},
        'catalog_long_tables': census['long_tables'],
    }
