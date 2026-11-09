"""Saved-only original decoding; source wire is never reconstructed by an encoder."""
from shared_long_values.source import inventory as shared_inventory
from shared_long_values.source import parse as shared_parse
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / 'tests/access'))
from compressed_memo_observations import capture as long_wire
from indirect_lval_observations import capture as declarations
from ace16_insert_ignored_indexes import reference as index_wire
import delete_reference_maps as map_wire
from ace16_insert_long_value_map_growth import requests
from ace16_insert_external_long_values import census

PAGE = 4096


def number(data, offset, width=2):
    return long_wire.number(data, offset, width)


def label(value):
    return bytes.fromhex(value).decode('utf-16le', errors='strict')


def parse(text):
    """Bind this feature's source policy to shared independent mechanics."""
    return shared_parse(
        text,
        label=label,
    )


def expected_rows():
    return {
        1: (None, None, None), 2: (7, b'', ''),
        3: (7, requests.binary(65, 3), 'Ā' * 33),
        4: (8, requests.binary(4076, 4), 'Ā' * 2038),
        5: (None, requests.binary(4077, 5), 'Ā' * 2039),
        6: (9, requests.binary(65, 6), 'ABC'),
    }


def inventory(raw, owned, referenced):
    """Bind this feature's source policy to shared independent mechanics."""
    return shared_inventory(
        raw,
        owned,
        referenced,
        PAGE=PAGE,
        number=number,
    )


def map_cell(raw, claim, page):
    start, end = claim['row_start'], claim['row_end']
    if raw[start] == 0:
        relative = page - number(raw, start + 1, 4)
        assert 0 <= relative < (end - start - 5) * 8
        return start + 5 + relative // 8, 1 << (relative % 8)
    assert raw[start] == 1
    span = (PAGE - 4) * 8
    pointer_at = start + 1 + page // span * 4
    assert pointer_at + 4 <= end
    pointer = number(raw, pointer_at, 4)
    assert 0 < pointer < len(raw) // PAGE
    relative = page % span
    return pointer * PAGE + 4 + relative // 8, 1 << (relative % 8)



def represented_cell(raw, claim, page):
    start, end = claim['row_start'], claim['row_end']
    if raw[start] == 0:
        relative = page - number(raw, start + 1, 4)
        if not 0 <= relative < (end - start - 5) * 8:
            return None
    else:
        assert raw[start] == 1
        pointer_at = start + 1 + page // ((PAGE - 4) * 8) * 4
        if pointer_at + 4 > end or number(raw, pointer_at, 4) == 0:
            return None
    return map_cell(raw, claim, page)

def inspect(path, getter, mode, padding_rows):
    assert mode in ('raw', 'compressed') and padding_rows in (430, 700)
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= 8 * 1024 * 1024 and len(raw) % PAGE == 0
    tables, metadata, properties, absent = parse(Path(getter).read_text())
    assert set(('Items', 'Sentinel')) <= set(tables)
    decoded_maps = {}
    for role, description in metadata.items():
        members = description['all_members']
        assert members == sorted(set(members))
        description['file_members'] = [page for page in members if page < len(raw) // PAGE]
        decoded_maps[role] = map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
    table = tables['Items']
    assert table['count'] == len(table['rows']) == 6
    assert table['columns_count'] == table['max_columns'] == 4
    assert list(table['columns']) == ['ID', 'Group', 'Blob', 'Memo']
    declaration = declarations.declarations(raw, table)
    assert declaration['complete']
    owner = 'Table:' + str(table['definition'])
    assert not properties.get(owner)
    for position, (name, native) in enumerate([('ID', 4), ('Group', 4), ('Blob', 11), ('Memo', 12)]):
        column = table['columns'][name]
        declared = declaration['columns'][position]
        assert column['position'] == column['number'] == declared['number'] == position
        assert column['native_type'] == declared['native'] == native and not column['auto']
        assert declared['flags'] == (3 if position < 2 else 2)
        assert declared['extended'] == (1 if name == 'Memo' and mode == 'compressed' else 0)
        header = bytes.fromhex(declared['raw_hex'])
        assert number(header, 23) == (4 if position < 2 else 0)
        if position < 2:
            assert number(header, 21) == position * 4
        else:
            assert number(header, 7) == column['variable'] == position - 2
        wanted = [['Required', 1, 'true', 'java.lang.Boolean', 'true']] if name == 'ID' else []
        if name == 'Memo':
            wanted = [['AllowZeroLength', 1, 'true', 'java.lang.Boolean', 'true']]
        assert properties.get(owner + ':Column:' + str(position), []) == wanted
    assert number(raw, table['definition'] * PAGE + 16, 4) == 6
    long_values = {}
    for identity, (group, blob, memo) in expected_rows().items():
        values = table['values'][identity]
        assert values == dict(ID=identity, Group=group, Blob=blob, Memo=memo)
        row = table['rows'][identity]
        actual, path_frames = long_wire.final_frame(raw, row)
        assert (actual['page'], actual['slot']) == (row['target_page'], row['target_slot'])
        body = raw[actual['start']:actual['end']]
        assert body == row['raw'] and number(body, 0) == 4
        mask = len(body) - 1
        assert number(body, mask - 2) == 2 and body[mask] & 0xf0 == 0
        offsets = [number(body, mask - 4 - variable * 2) for variable in range(3)]
        assert offsets[0] == 10 and offsets == sorted(offsets)
        assert offsets[-1] <= mask - 8
        assert body[mask] & 1 and int.from_bytes(body[2:6], 'little', signed=True) == identity
        assert bool(body[mask] & 2) == (group is not None)
        if group is not None:
            assert int.from_bytes(body[6:10], 'little', signed=True) == group
        for name, expected in [('Blob', blob), ('Memo', memo)]:
            value = long_wire.long_value(raw, body, table['columns'][name])
            logical = value['logical']
            if isinstance(logical, dict):
                logical = bytes.fromhex(logical['binary_hex'])
            assert logical == expected
            long_values[(identity, name)] = value
    scopes = {}
    assert {pair['column'] for pair in declaration['pairs']} == {2, 3}
    for pair in declaration['pairs']:
        name = 'Blob' if pair['column'] == 2 else 'Memo'
        for suffix, field in [('Owned', 'owned'), ('Hint', 'candidate')]:
            role = owner + ':Column:' + str(pair['column']) + ':' + suffix
            item = decoded_maps[role]
            assert item['map_page'] << 8 | item['map_slot'] == pair[field]
        owned = decoded_maps[owner + ':Column:' + str(pair['column']) + ':Owned']['file_members']
        hints = decoded_maps[owner + ':Column:' + str(pair['column']) + ':Hint']['file_members']
        assert set(hints) <= set(owned)
        referenced = {chunk['reference'] for (identity, column), value in long_values.items()
                      if column == name for chunk in value['chunks']}
        scopes[name] = inventory(raw, owned, referenced)
    assert set(scopes['Blob']['pages']).isdisjoint(scopes['Memo']['pages'])
    indexes = table['indexes']
    assert list(indexes) == ['PrimaryID', 'PrimaryAlias', 'GroupOrder', 'GroupAlias']
    trees = {}
    physical_pages = set()
    for physical, names, column in [(0, ('PrimaryID', 'PrimaryAlias'), 'ID'),
                                    (1, ('GroupOrder', 'GroupAlias'), 'Group')]:
        selected = indexes[names[0]]
        alias = indexes[names[1]]
        assert selected['physical'] == alias['physical'] == physical
        assert selected['columns'] == alias['columns'] == [(column, True)]
        for field in ('root', 'entries', 'history', 'history_offset', 'flags', 'rows'):
            assert selected[field] == alias[field]
        assert selected['flags'] == (0x89 if physical == 0 else 0x80)
        assert selected['primary'] == (physical == 0) and not alias['primary']
        assert selected['entries'] == 6
        nodes, leaves, entries = index_wire.tree(raw, selected['root'], table['definition'])
        expected = sorted(index_wire.key(table['values'][identity][column],
                                         row['page'] << 8 | row['slot'])
                          for identity, row in table['rows'].items())
        assert entries == expected
        expected_ids = [int.from_bytes(key[-4:], 'big') for key in entries]
        assert [page << 8 | slot for _, page, slot in selected['rows']] == expected_ids
        for identity, page, slot in selected['rows']:
            assert (page, slot) == (table['rows'][identity]['page'], table['rows'][identity]['slot'])
        assert not physical_pages.intersection(nodes)
        physical_pages.update(nodes)
        role = owner + ':IndexData:' + str(physical)
        assert set(decoded_maps[role]['file_members']) == set(nodes)
        history_at = table['definition'] * PAGE + selected['history_offset']
        assert number(raw, history_at, 4) == selected['history']
        trees[physical] = {'nodes': nodes, 'leaves': leaves, 'entries': entries,
                           'history_at': history_at}
    data_pages = set(decoded_maps[owner + ':Owned']['file_members'])
    assert all(row['page'] in data_pages for row in table['rows'].values())
    assert physical_pages.isdisjoint(data_pages)
    long_pages = set(scopes['Blob']['pages']) | set(scopes['Memo']['pages'])
    assert long_pages.isdisjoint(physical_pages | data_pages)
    inventory_by_table = {name: declarations.declarations(raw, description)
                          for name, description in tables.items()}
    assert all(item['complete'] for item in inventory_by_table.values())
    map_claims = declarations.claims(raw, tables, inventory_by_table, decoded_maps,
                                    {name: description['indexes'] for name, description in tables.items()})
    selected_refs = [pair[field] for pair in declaration['pairs'] for field in ('owned', 'candidate')]
    assert len(selected_refs) == len(set(selected_refs)) == 4
    source_global = set(decoded_maps['GlobalFree']['file_members'])
    assert source_global.isdisjoint(data_pages | physical_pages | long_pages)
    sentinel = tables['Sentinel']
    assert sentinel['count'] == 1 and set(sentinel['rows']) == {91}
    assert sentinel['values'] == {91: {'ID': 91, 'Marker': 'untouched'}}
    catalog_census = census.prove(raw, tables, inventory_by_table, decoded_maps,
                                  map_claims, scopes, long_wire)
    padding = tables['Padding']
    assert padding['count'] == len(padding['rows']) == padding_rows
    assert set(padding['rows']) == set(range(1, padding_rows + 1))
    assert padding['columns_count'] == padding['max_columns'] == 8
    assert list(padding['columns']) == ['ID'] + ['Payload' + str(index) for index in range(7)]
    padding_owner = 'Table:' + str(padding['definition'])
    assert not properties.get(padding_owner) and not padding['indexes']
    padding_declaration = inventory_by_table['Padding']
    for position, (name, column) in enumerate(padding['columns'].items()):
        header = bytes.fromhex(padding_declaration['columns'][position]['raw_hex'])
        assert column['position'] == column['number'] == position
        assert not properties.get(padding_owner + ':Column:' + str(position))
        assert not column['auto'] and not column['compressed']
        assert header[0] == column['native_type'] == (4 if position == 0 else 10)
        assert header[15] == (3 if position == 0 else 2) and header[16] == 0
        assert number(header, 23) == column['length'] == (4 if position == 0 else 510)
        if position == 0:
            assert number(header, 21) == 0
        else:
            assert number(header, 7) == column['variable'] == position - 1
    padding_pages = decoded_maps[padding_owner + ':Owned']['file_members']
    assert len(padding_pages) == padding_rows
    for identity, row in padding['rows'].items():
        assert row['page'] in padding_pages
        frame, chain = long_wire.final_frame(raw, row)
        assert len(chain) == 1 and frame['word'] & 0xe000 == 0
        body = raw[frame['start']:frame['end']]
        assert body == row['raw'] and len(body) == 3595
        assert number(body, 0) == 8 and body[-1] == 255
        assert number(body, len(body) - 3) == 7
        assert int.from_bytes(body[2:6], 'little', signed=True) == identity
        offsets = [number(body, len(body) - 5 - variable * 2) for variable in range(8)]
        assert offsets == [6 + variable * 510 for variable in range(8)]
        values = {'ID': identity}
        for variable in range(7):
            payload = body[offsets[variable]:offsets[variable + 1]]
            assert long_wire.decode_stream(payload) == 'Ā' * 255
            values['Payload' + str(variable)] = 'Ā' * 255
        assert padding['values'][identity] == values
    future_pages = list(range(len(raw) // PAGE, len(raw) // PAGE + 16))
    growth = {}
    for name, column in [('Blob', 2), ('Memo', 3)]:
        role = owner + ':Column:' + str(column)
        for suffix in (':Owned', ':Hint'):
            claim = map_claims[role + suffix]
            assert raw[claim['row_start']] == 0 and claim['row_end'] - claim['row_start'] == 69
            assert number(raw, claim['row_start'] + 1, 4) == 0
        minimum = min(scopes[name]['pages'])
        aligned = minimum // 8 * 8
        final_bound = future_pages[0] + 7
        fits = final_bound < aligned + 512
        assert (padding_rows == 430) == fits
        assert future_pages[0] >= 527
        growth[name] = {'minimum': minimum, 'aligned_base': aligned,
                        'source_eof': future_pages[0], 'checked_final_bound': final_bound,
                        'hypothesis': 'rebase' if fits else 'promotion'}
    for role, claim in map_claims.items():
        for page in future_pages:
            offset = represented_cell(raw, claim, page)
            if role == 'GlobalFree':
                assert offset is not None and raw[offset[0]] & offset[1]
            elif offset is not None:
                assert not raw[offset[0]] & offset[1], (role, page)
    return {'raw': raw, 'tables': tables, 'maps': decoded_maps, 'properties': properties,
            'absent': absent, 'declaration': declaration, 'long_values': long_values,
            'scopes': scopes, 'trees': trees, 'map_claims': map_claims,
            'inventory': inventory_by_table, 'census': catalog_census,
            'certified_future_pages': future_pages, 'growth': growth}
