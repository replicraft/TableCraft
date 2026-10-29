"""Saved raw empty ownership, complete catalog roles and inert getter binding."""
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_global_bootstrap import source as native

PAGE = 4096
POINTER_BYTES = 4
REFERENCE_BYTES = 69


def bind_definition(raw, table, claims, index_names, variables):
    data, pages = native.declarations.definition(raw, table['definition'])
    owner = 'Table:' + str(table['definition'])
    assert native.number(data, 16, 4) == table['count']
    assert native.number(data, 41) == native.number(data, 45) == table['columns_count']
    assert native.number(data, 43) == variables
    assert native.number(data, 55, 4) == claims[owner + ':Owned']['reference']
    assert native.number(data, 59, 4) == claims[owner + ':Hint']['reference']
    physical_count = 1 if index_names else 0
    assert native.number(data, 51, 4) == physical_count
    assert native.number(data, 47, 4) == len(index_names)
    at = 63 + physical_count * 12 + table['columns_count'] * 25
    for name in table['columns']:
        length = native.number(data, at)
        at += 2
        assert data[at:at + length].decode('utf-16le') == name
        at += length
    if index_names:
        index = table['indexes']['PrimaryID']
        block = data[at:at + 52]
        assert len(block) == 52 and native.number(block, 4) == 0 and block[6] == 1
        assert all(native.number(block, 4 + position * 3) == 0xffff for position in range(1, 10))
        assert native.number(block, 34, 4) == claims[owner + ':IndexData:0']['reference']
        assert native.number(block, 38, 4) == index['root']
        assert block[46] == index['flags'] == 0x89
        assert index['history_offset'] == 67 and native.number(data, 67, 4) == 0
    logical = at + physical_count * 52
    names = logical + len(index_names) * 28
    for number, name in enumerate(index_names):
        block = data[logical + number * 28:logical + (number + 1) * 28]
        assert native.number(block, 4, 4) == number
        assert native.number(block, 8, 4) == 0
        assert block[23] == (1 if name == 'PrimaryID' else 0)
        length = native.number(data, names)
        names += 2
        assert data[names:names + length].decode('utf-16le') == name
        names += length
    assert native.number(data, names) == 0xffff
    return pages


def scalar_rows(raw, table, maps):
    owner = 'Table:' + str(table['definition'])
    pages = maps[owner + ':Owned']['file_members']
    hints = maps[owner + ':Hint']['file_members']
    assert set(hints) <= set(pages)
    references = set()
    for number in pages:
        page = raw[number * PAGE:(number + 1) * PAGE]
        assert page[:2] == b'\x01\x01'
        assert native.number(page, 4, 4) == table['definition']
        count = native.number(page, 12)
        assert count <= 255
        end = PAGE
        for slot in range(count):
            word = native.number(page, 14 + slot * 2)
            start = word & 0x1fff
            assert word & 0x2000 == 0
            assert 14 + count * 2 <= start <= end
            if not word & 0x8000:
                assert start < end
                references.add(number << 8 | slot)
            end = start
        assert native.number(page, 2) == end - 14 - count * 2
    assert references == {row['page'] << 8 | row['slot'] for row in table['rows'].values()}
    assert len(references) == table['count']
    for row in table['rows'].values():
        frame, chain = native.long_wire.final_frame(raw, row)
        assert (frame['page'], frame['slot']) == (row['target_page'], row['target_slot'])
        assert all(part['page'] in pages for part in chain)
        for part in chain:
            assert native.number(raw, part['page'] * PAGE + 4, 4) == table['definition']
        assert raw[frame['start']:frame['end']] == row['raw']


def require_exclusive(claims, selected):
    for role in selected:
        claim = claims[role]
        for other, foreign in claims.items():
            if role == other:
                continue
            assert (claim['row_start'], claim['row_end']) != (foreign['row_start'], foreign['row_end']), (role, other)
            assert set(claim['bitmaps']).isdisjoint(foreign['bitmaps']), (role, other)


def inspect(path, getter):
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= 8 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[20] == 5 and raw[4:19] == b'Standard ACE DB'
    tables, descriptions, properties, absent = native.parse(Path(getter).read_text())
    assert {'Items', 'Sentinel'} <= set(tables)
    maps = {}
    for role, description in descriptions.items():
        members = description['all_members']
        assert members == sorted(set(members))
        description['file_members'] = [page for page in members if page < len(raw) // PAGE]
        maps[role] = native.map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
    declarations = {name: native.declarations.declarations(raw, table)
                    for name, table in tables.items()}
    assert all(declaration['complete'] for declaration in declarations.values())
    claims = native.declarations.claims(raw, tables, declarations, maps,
                                       {name: table['indexes'] for name, table in tables.items()})
    table = tables['Items']
    owner = 'Table:' + str(table['definition'])
    assert table['columns_count'] == table['max_columns'] == 2
    assert table['count'] == 0 and table['rows'] == {} and table['values'] == {}
    assert list(table['columns']) == ['ID', 'Bytes']
    assert list(table['indexes']) == ['PrimaryID', 'PrimaryAlias']
    assert not declarations['Items']['pairs'] and not properties.get(owner)
    bind_definition(raw, table, claims, ['PrimaryID', 'PrimaryAlias'], 1)
    for position, (name, kind, width, flags) in enumerate([
        ('ID', 4, 4, 3), ('Bytes', 9, 255, 2),
    ]):
        column = table['columns'][name]
        declared = declarations['Items']['columns'][position]
        block = bytes.fromhex(declared['raw_hex'])
        assert column['position'] == column['number'] == declared['number'] == position
        assert column['native_type'] == declared['native'] == kind
        assert column['length'] == native.number(block, 23) == width
        assert not column['auto'] and not column['compressed']
        assert declared['flags'] == flags and declared['extended'] == 0
        assert native.number(block, 21 if position == 0 else 7) == 0
        wanted = [['Required', 1, 'true', 'java.lang.Boolean', 'true']] if position == 0 else []
        assert properties.get(owner + ':Column:' + str(position), []) == wanted
    scalar_rows(raw, table, maps)
    owned, hints = maps[owner + ':Owned'], maps[owner + ':Hint']
    assert owned['file_members'] == hints['file_members'] == []
    assert owned['outside_file_set_bit_count'] == hints['outside_file_set_bit_count'] == 0
    assert owned['payload_type'] == 1 and hints['payload_type'] == 0
    assert owned['row_end'] - owned['row_start'] == REFERENCE_BYTES
    assert hints['row_end'] - hints['row_start'] == REFERENCE_BYTES
    owned_bytes = raw[owned['row_start']:owned['row_end']]
    assert owned_bytes[0] == 1 and native.number(owned_bytes, 1, POINTER_BYTES) == 0
    assert native.number(raw, owned['map_page'] * PAGE + 4, 4) == 0
    assert native.number(raw, hints['map_page'] * PAGE + 4, 4) == 0
    assert native.number(raw, hints['row_start'] + 1, 4) == 0
    assert raw[hints['row_start'] + 5:hints['row_end']] == bytes(64)
    selected, alias = (table['indexes'][name] for name in ('PrimaryID', 'PrimaryAlias'))
    assert selected['physical'] == alias['physical'] == 0
    assert selected['columns'] == alias['columns'] == [('ID', True)]
    assert selected['primary'] and not alias['primary']
    for field in ('root', 'entries', 'history', 'history_offset', 'flags', 'rows'):
        assert selected[field] == alias[field]
    assert selected['entries'] == selected['history'] == 0 and selected['rows'] == []
    nodes, leaves, entries = native.index_wire.tree(raw, selected['root'], table['definition'])
    assert leaves == [selected['root']] and list(nodes) == [selected['root']] and entries == []
    assert nodes[selected['root']]['prefix'] == nodes[selected['root']]['active'] == 0
    assert maps[owner + ':IndexData:0']['file_members'] == [selected['root']]
    sentinel = tables['Sentinel']
    sentinel_owner = 'Table:' + str(sentinel['definition'])
    assert sentinel['count'] == 1 and set(sentinel['rows']) == {91}
    assert sentinel['values'] == {91: {'ID': 91, 'Marker': 12345}}
    assert sentinel['columns_count'] == sentinel['max_columns'] == 2 and not sentinel['indexes']
    assert list(sentinel['columns']) == ['ID', 'Marker']
    assert not properties.get(sentinel_owner) and not declarations['Sentinel']['pairs']
    bind_definition(raw, sentinel, claims, [], 0)
    for position, name in enumerate(('ID', 'Marker')):
        column = sentinel['columns'][name]
        block = bytes.fromhex(declarations['Sentinel']['columns'][position]['raw_hex'])
        assert column['position'] == column['number'] == position
        assert column['native_type'] == 4 and column['length'] == 4
        assert not column['auto'] and not column['compressed']
        assert block[0] == 4 and block[15] == 3 and block[16] == 0
        assert native.number(block, 5) == position and native.number(block, 23) == 4
        assert native.number(block, 21) == position * 4
        assert not properties.get(sentinel_owner + ':Column:' + str(position))
    scalar_rows(raw, sentinel, maps)
    expected_row = b'\x02\0' + (91).to_bytes(4, 'little') + (12345).to_bytes(4, 'little') + b'\x03'
    assert sentinel['rows'][91]['raw'] == expected_row
    census = native.census.prove(raw, tables, declarations, maps, claims, {}, native.long_wire)
    free = set(maps['GlobalFree']['file_members'])
    assert free.isdisjoint(map(int, census['protected_pages']))
    assert free.isdisjoint(map(int, census['owned_claims']))
    assert free.isdisjoint(map(int, census['live_claims']))
    assert claims['GlobalFree']['reference'] == 256
    assert native.number(raw, claims['GlobalFree']['container'] * PAGE + 4, 4) == 1
    for role in (owner + ':Owned', owner + ':Hint', owner + ':IndexData:0'):
        assert native.number(raw, claims[role]['container'] * PAGE + 4, 4) == 0
    for role, description in maps.items():
        if role != owner + ':IndexData:0' and (role.endswith(':Owned') or ':IndexData:' in role):
            assert selected['root'] not in description['file_members'], role
    mutable = ['GlobalFree', owner + ':Owned', owner + ':Hint', owner + ':IndexData:0']
    require_exclusive(claims, mutable)
    future = list(range(len(raw) // PAGE, len(raw) // PAGE + 8))
    assert future[-1] < 512
    for role, claim in claims.items():
        for number in future:
            cell = native.represented_cell(raw, claim, number)
            if role == 'GlobalFree':
                assert cell is not None and raw[cell[0]] & cell[1]
            elif cell is not None:
                assert not raw[cell[0]] & cell[1], (role, number)
    return {'raw': raw, 'tables': tables, 'table': table, 'maps': maps,
            'properties': properties, 'absent': absent, 'declarations': declarations,
            'map_claims': claims, 'trees': {0: {'nodes': nodes, 'leaves': leaves, 'entries': entries,
                                             'history_at': table['definition'] * PAGE + 67}},
            'census': census, 'certified_future_pages': future,
            'source_owned_record': owned_bytes.hex()}
