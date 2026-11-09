"""Saved original declarations and raw civil timestamps, never re-encoded rows."""
import datetime
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_global_bootstrap import source as native

PAGE = 4096
DATE_BYTES = 42
MAX_DAY = 3652058


def timestamp(data):
    assert len(data) == DATE_BYTES
    assert data[19] == 58 and data[39:] == b':7\0'
    assert data[:19].isdigit() and data[20:39].isdigit()
    days, seconds, fraction = int(data[:19]), int(data[20:32]), int(data[32:39])
    assert 0 <= days <= MAX_DAY and 0 <= seconds < 86400
    date = datetime.date(1, 1, 1) + datetime.timedelta(days=days)
    return (f'{date.year:04}-{date.month:02}-{date.day:02}T'
            f'{seconds // 3600:02}:{seconds % 3600 // 60:02}:{seconds % 60:02}.{fraction:07}')


def parse(getter):
    lines = getter.read_text().splitlines()
    # The existing owner parses unchanged catalog/map/index/ROW records. Values
    # are interpreted here because this helper adds an explicit civil carrier.
    tables, maps, properties, absent = native.parse('\n'.join(
        line for line in lines if not line.startswith('VALUE\t')))
    for line in lines:
        fields = line.split('\t')
        if fields[0] != 'VALUE':
            continue
        assert len(fields) == 7
        name, identity, column = native.label(fields[1]), int(fields[2]), native.label(fields[3])
        carrier, kind, encoded = fields[4:]
        values = tables[name]['values'][identity]
        assert column not in values
        if kind == 'NULL':
            assert carrier == 'null' and encoded == 'NULL'
            value = None
        elif kind == 'INT32':
            assert carrier == 'java.lang.Integer'
            value = int(encoded)
            assert -(2 ** 31) <= value < 2 ** 31
        else:
            assert kind == 'EXTENDED_DATETIME' and carrier == 'java.time.LocalDateTime'
            assert len(encoded) == 27 and encoded.isascii()
            assert encoded[4] == encoded[7] == '-'
            assert encoded[10] == 'T' and encoded[13] == encoded[16] == ':' and encoded[19] == '.'
            assert all(encoded[position].isdigit() for position in range(27)
                       if position not in (4, 7, 10, 13, 16, 19))
            datetime.datetime.strptime(encoded[:19], '%Y-%m-%dT%H:%M:%S')
            value = encoded
        values[column] = value
    return tables, maps, properties, absent


def ordinary_rows(raw, table, maps):
    owner = 'Table:' + str(table['definition'])
    pages = maps[owner + ':Owned']['file_members']
    hints = maps[owner + ':Hint']['file_members']
    assert set(hints) <= set(pages)
    roots = set()
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
                roots.add(number << 8 | slot)
            end = start
        assert native.number(page, 2) == end - 14 - count * 2
    assert roots == {row['page'] << 8 | row['slot'] for row in table['rows'].values()}
    assert len(roots) == table['count']
    for row in table['rows'].values():
        frame, chain = native.long_wire.final_frame(raw, row)
        assert (frame['page'], frame['slot']) == (row['target_page'], row['target_slot'])
        assert all(part['page'] in pages for part in chain)
        for part in chain:
            base = part['page'] * PAGE
            assert native.number(raw, base + 4, 4) == table['definition']
        assert raw[frame['start']:frame['end']] == row['raw']


def bind_definition(raw, table, claims, index_names):
    data, pages = native.declarations.definition(raw, table['definition'])
    owner = 'Table:' + str(table['definition'])
    assert native.number(data, 16, 4) == table['count']
    assert native.number(data, 41) == native.number(data, 45) == table['columns_count']
    assert native.number(data, 43) == 0
    assert native.number(data, 55, 4) == claims[owner + ':Owned']['reference']
    assert native.number(data, 59, 4) == claims[owner + ':Hint']['reference']
    physical_count = 2 if index_names else 0
    assert native.number(data, 51, 4) == physical_count
    assert native.number(data, 47, 4) == len(index_names)
    at = 63 + physical_count * 12 + table['columns_count'] * 25
    for name in table['columns']:
        length = native.number(data, at)
        at += 2
        assert data[at:at + length].decode('utf-16le') == name
        at += length
    for physical, name in enumerate(('PrimaryID', 'GroupOrder') if index_names else ()):
        block = data[at + physical * 52:at + (physical + 1) * 52]
        index = table['indexes'][name]
        assert len(block) == 52
        assert native.number(block, 4) == physical and block[6] == 1
        assert all(native.number(block, 4 + position * 3) == 0xffff for position in range(1, 10))
        assert native.number(block, 34, 4) == claims[owner + ':IndexData:' + str(physical)]['reference']
        assert native.number(block, 38, 4) == index['root']
        assert block[46] == index['flags']
        assert index['history_offset'] == 67 + physical * 12
    logical = at + physical_count * 52
    names = logical + len(index_names) * 28
    for number, name in enumerate(index_names):
        block = data[logical + number * 28:logical + (number + 1) * 28]
        assert native.number(block, 4, 4) == number
        assert native.number(block, 8, 4) == number // 2
        assert block[23] == (1 if name == 'PrimaryID' else 0)
        length = native.number(data, names)
        names += 2
        assert data[names:names + length].decode('utf-16le') == name
        names += length
    assert native.number(data, names) == 0xffff


def inspect(path, getter):
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= 8 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[20] == 6 and raw[4:19] == b'Standard ACE DB'
    tables, descriptions, properties, absent = parse(Path(getter))
    assert {'Items', 'Sentinel'} <= set(tables)
    maps = {}
    for role, description in descriptions.items():
        members = description['all_members']
        assert members == sorted(set(members))
        description['file_members'] = [page for page in members if page < len(raw) // PAGE]
        maps[role] = native.map_wire.decode_map(raw, description, PAGE, len(raw) // PAGE)
    declarations = {name: native.declarations.declarations(raw, table)
                    for name, table in tables.items()}
    assert all(item['complete'] for item in declarations.values())
    claims = native.declarations.claims(raw, tables, declarations, maps,
                                       {name: table['indexes'] for name, table in tables.items()})
    table = tables['Items']
    definition = declarations['Items']
    owner = 'Table:' + str(table['definition'])
    bind_definition(raw, table, claims, ['PrimaryID', 'PrimaryAlias', 'GroupOrder', 'GroupAlias'])
    assert table['columns_count'] == table['max_columns'] == 3
    assert table['count'] == len(table['rows']) == 4
    assert list(table['columns']) == ['ID', 'Group', 'Stamp']
    assert native.number(raw, table['definition'] * PAGE + 43) == 0
    assert not definition['pairs'] and not properties.get(owner)
    for position, (name, kind, length, offset) in enumerate([
        ('ID', 4, 4, 0), ('Group', 4, 4, 4), ('Stamp', 20, 42, 8),
    ]):
        column = table['columns'][name]
        declared = definition['columns'][position]
        header = bytes.fromhex(declared['raw_hex'])
        assert column['position'] == column['number'] == declared['number'] == position
        assert column['native_type'] == declared['native'] == kind
        assert not column['auto'] and not column['compressed']
        assert column['length'] == native.number(header, 23) == length
        assert native.number(header, 21) == offset
        assert declared['flags'] == 3 and declared['extended'] == 0
        wanted = [['Required', 1, 'true', 'java.lang.Boolean', 'true']] if position == 0 else []
        assert properties.get(owner + ':Column:' + str(position), []) == wanted
    plan = json.loads((Path(__file__).parent / 'plan.json').read_text())
    expected = {identity: {'ID': identity, 'Group': group, 'Stamp': stamp}
                for identity, group, stamp in plan['origins'][0]['old_rows']}
    assert table['values'] == expected and set(table['rows']) == set(expected)
    ordinary_rows(raw, table, maps)
    for identity, row in table['rows'].items():
        body = row['raw']
        assert len(body) == 53 and native.number(body, 0) == 3
        assert body[-1] & 0xf8 == 0 and body[-1] & 1
        assert int.from_bytes(body[2:6], 'little', signed=True) == identity
        for name, mask, start, end in [('Group', 2, 6, 10), ('Stamp', 4, 10, 52)]:
            value = expected[identity][name]
            assert bool(body[-1] & mask) == (value is not None)
            if value is not None:
                actual = (timestamp(body[start:end]) if name == 'Stamp'
                          else int.from_bytes(body[start:end], 'little', signed=True))
                assert actual == value
    trees = {}
    all_index_pages = set()
    for physical, column, names in [(0, 'ID', ('PrimaryID', 'PrimaryAlias')),
                                     (1, 'Group', ('GroupOrder', 'GroupAlias'))]:
        selected, alias = (table['indexes'][name] for name in names)
        assert selected['physical'] == alias['physical'] == physical
        assert selected['columns'] == alias['columns'] == [(column, True)]
        for field in ('root', 'entries', 'history', 'history_offset', 'flags', 'rows'):
            assert selected[field] == alias[field]
        assert selected['flags'] == (0x89 if physical == 0 else 0x80)
        assert selected['primary'] == (physical == 0) and not alias['primary']
        assert selected['entries'] == 4
        nodes, leaves, entries = native.index_wire.tree(raw, selected['root'], table['definition'])
        expected_keys = sorted(native.index_wire.key(expected[identity][column], row['page'] << 8 | row['slot'])
                               for identity, row in table['rows'].items())
        assert entries == expected_keys and not all_index_pages.intersection(nodes)
        all_index_pages.update(nodes)
        assert maps[owner + ':IndexData:' + str(physical)]['file_members'] == sorted(nodes)
        references = [int.from_bytes(entry[-4:], 'big') for entry in entries]
        assert [page << 8 | slot for identity, page, slot in selected['rows']] == references
        for identity, page, slot in selected['rows']:
            assert (page, slot) == (table['rows'][identity]['page'], table['rows'][identity]['slot'])
        at = table['definition'] * PAGE + selected['history_offset']
        assert native.number(raw, at, 4) == selected['history']
        assert selected['history'] == len({value[column] for value in expected.values()})
        trees[physical] = {'nodes': nodes, 'leaves': leaves, 'entries': entries, 'history_at': at}
    assert list(table['indexes']) == ['PrimaryID', 'PrimaryAlias', 'GroupOrder', 'GroupAlias']
    assert all_index_pages.isdisjoint(maps[owner + ':Owned']['file_members'])
    sentinel = tables['Sentinel']
    assert sentinel['count'] == 1 and set(sentinel['rows']) == {91}
    assert sentinel['values'] == {91: {'ID': 91, 'Marker': 12345}}
    assert sentinel['columns_count'] == sentinel['max_columns'] == 2 and not sentinel['indexes']
    sentinel_owner = 'Table:' + str(sentinel['definition'])
    bind_definition(raw, sentinel, claims, [])
    assert not properties.get(sentinel_owner)
    assert list(sentinel['columns']) == ['ID', 'Marker']
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
    ordinary_rows(raw, sentinel, maps)
    row = sentinel['rows'][91]['raw']
    assert row == b'\x02\0' + (91).to_bytes(4, 'little') + (12345).to_bytes(4, 'little') + b'\x03'
    census = native.census.prove(raw, tables, declarations, maps, claims, {}, native.long_wire)
    free = set(maps['GlobalFree']['file_members'])
    assert free.isdisjoint(map(int, census['protected_pages']))
    assert free.isdisjoint(map(int, census['owned_claims']))
    assert free.isdisjoint(map(int, census['live_claims']))
    future = list(range(len(raw) // PAGE, len(raw) // PAGE + 8))
    for role, claim in claims.items():
        for number in future:
            cell = native.represented_cell(raw, claim, number)
            if role == 'GlobalFree':
                assert cell is not None and raw[cell[0]] & cell[1]
            elif cell is not None:
                assert not raw[cell[0]] & cell[1], (role, number)
    return {'raw': raw, 'tables': tables, 'table': table, 'maps': maps,
            'properties': properties, 'absent': absent, 'declarations': declarations,
            'map_claims': claims, 'trees': trees, 'census': census,
            'certified_future_pages': future}
