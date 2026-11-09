"""Pure original IgnoreNULL membership; old NULL payload bytes remain opaque."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_global_bootstrap import source as native
from ace16_insert_empty_owned import source as ordinary
from ace16_set_null_deletes import source as records

PAGE = 4096
PRIMARY = 1
FOREIGN = 2
HERE = Path(__file__).resolve().parent


def parse(text):
    ordinary_lines = []
    counters = {}
    for line in text.splitlines():
        fields = line.split('\t')
        if fields[0] == 'AUTONUMBER':
            assert len(fields) == 5 and fields[4] == 'false'
            name = native.label(fields[1])
            assert name not in counters
            counters[name] = tuple(map(int, fields[2:4]))
        else:
            ordinary_lines.append(line)
    tables, descriptions, properties, absent, relationships = records.parse('\n'.join(ordinary_lines))
    assert not relationships
    return tables, descriptions, properties, absent, counters


def map_reference(metadata):
    page, slot = metadata['map_page'], metadata['map_slot']
    assert 0 < page < 1 << 24 and 0 <= slot < 256
    return page << 8 | slot


def index_declarations(raw, table, maps):
    data, pages = native.declarations.definition(raw, table['definition'])
    physical_count = native.number(data, 51, 4)
    logical_count = native.number(data, 47, 4)
    assert logical_count == len(table['indexes']) <= 32
    assert native.number(data, 16, 4) == table['count']
    owner = 'Table:' + str(table['definition'])
    for offset, suffix in ((55, ':Owned'), (59, ':Hint')):
        assert native.number(data, offset, 4) == map_reference(maps[owner + suffix])
    at = 63 + physical_count * 12 + table['columns_count'] * 25
    for column in table['columns']:
        length = native.number(data, at)
        at += 2
        assert data[at:at + length].decode('utf-16le') == column
        at += length
    physical = [data[at + number * 52:at + (number + 1) * 52]
                for number in range(physical_count)]
    logical_start = at + physical_count * 52
    names = logical_start + logical_count * 28
    logical = {}
    for number in range(logical_count):
        block = data[logical_start + number * 28:logical_start + (number + 1) * 28]
        length = native.number(data, names)
        names += 2
        name = data[names:names + length].decode('utf-16le')
        names += length
        assert name not in logical and native.number(block, 4, 4) == number
        index = table['indexes'][name]
        position = native.number(block, 8, 4)
        assert position == index['physical'] < physical_count
        header = physical[position]
        assert native.number(header, 38, 4) == index['root']
        assert native.number(header, 34, 4) == map_reference(maps[owner + ':IndexData:' + str(position)])
        assert header[46] == index['flags'] and index['flags'] & ~0x8b == 0
        assert index['history_offset'] == 63 + position * 12 + 4
        assert native.number(data, index['history_offset'], 4) == index['history']
        assert 1 <= len(index['columns']) <= 2
        for component, (column_name, ascending) in enumerate(index['columns']):
            key_column = table['columns'][column_name]['number']
            assert native.number(header, 4 + component * 3) == key_column
            assert header[6 + component * 3] == int(ascending)
        assert all(native.number(header, 4 + component * 3) == 0xffff
                   for component in range(len(index['columns']), 10))
        assert block[23] in (0, PRIMARY, FOREIGN)
        assert index['primary'] == (block[23] == PRIMARY)
        logical[name] = dict(number=number, physical=position, kind=block[23],
                             role=block[12], target_index=native.number(block, 13, 4),
                             target_table=native.number(block, 17, 4),
                             update=block[21], delete=block[22])
    assert native.number(data, names) == 0xffff
    return logical, pages


def component(value, ascending):
    if value is None:
        result = b'\x00'
    else:
        assert -(1 << 31) <= value < 1 << 31
        result = b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
    return result if ascending else bytes(byte ^ 255 for byte in result)


def decode_entry(entry, columns):
    values = []
    at = 0
    for name, ascending in columns:
        assert at < len(entry) - 4
        marker = entry[at] if ascending else entry[at] ^ 255
        at += 1
        if marker == 0:
            values.append(None)
            continue
        assert marker == 127 and at + 4 <= len(entry) - 4
        payload = entry[at:at + 4]
        if not ascending:
            payload = bytes(byte ^ 255 for byte in payload)
        unsigned = int.from_bytes(payload, 'big') ^ 0x80000000
        values.append(unsigned if unsigned < 1 << 31 else unsigned - (1 << 32))
        at += 4
    assert at + 4 == len(entry)
    return tuple(values), int.from_bytes(entry[at:], 'big')


def leaf(raw, root, owner, columns):
    assert 0 < root < len(raw) // PAGE
    page = raw[root * PAGE:(root + 1) * PAGE]
    assert page[:2] == b'\x04\x01' and native.number(page, 4, 4) == owner
    assert page[26] == 0 and page[8:24] == bytes(16)
    prefix = native.number(page, 24)
    assert prefix <= 5 * len(columns) + 4
    assert not page[27] & 1 and not page[479] & 0xfe
    ends = [position for position in range(1, 3617)
            if page[27 + position // 8] & (1 << (position % 8))]
    assert (ends[-1] if ends else 0) == 3616 - native.number(page, 2)
    entries = []
    previous = b''
    start = 0
    decoded = []
    for end in ends:
        assert start < end <= 3616
        payload = page[480 + start:480 + end]
        complete = payload if not entries else previous[:prefix] + payload
        assert not entries or (prefix <= len(previous) and previous < complete)
        decoded.append(decode_entry(complete, columns))
        entries.append(complete)
        previous = complete
        start = end
    return entries, decoded


def inspect(path, getter, profile):
    plan = json.loads((HERE / 'plan.json').read_text())
    assert profile in plan['layouts']
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= 8388608 and len(raw) % PAGE == 0
    assert raw[4:20] == b'Standard ACE DB\x00' and raw[20] == (5 if profile == 'V2016' else 6)
    tables, descriptions, properties, absent, counters = parse(Path(getter).read_text())
    users = set(plan['tables']) | {'Sentinel'}
    assert {name for name in tables if not name.startswith('MSys') and not name.startswith('f_')} == users
    assert set(counters) == set(tables)
    maps = {}
    for role, item in descriptions.items():
        assert item['all_members'] == sorted(set(item['all_members']))
        item['file_members'] = [number for number in item['all_members'] if number < len(raw) // PAGE]
        maps[role] = native.map_wire.decode_map(raw, item, PAGE, len(raw) // PAGE)
    declarations = {name: native.declarations.declarations(raw, table) for name, table in tables.items()}
    assert all(item['complete'] for item in declarations.values())
    claims = native.declarations.claims(raw, tables, declarations, maps,
                                       {name: table['indexes'] for name, table in tables.items()})
    all_roles = []
    facts = {}
    for name in sorted(users):
        table = tables[name]
        recipe = plan['sentinel'] if name == 'Sentinel' else plan['tables'][name]
        expected_rows = recipe['rows']
        columns = ['ID', 'Marker'] if name == 'Sentinel' else ['ID', 'Left', 'Right', 'Marker']
        owner = 'Table:' + str(table['definition'])
        definition, definition_pages = native.declarations.definition(raw, table['definition'])
        assert list(table['columns']) == columns
        assert table['columns_count'] == table['max_columns'] == len(columns)
        assert native.number(definition, 41) == native.number(definition, 45) == len(columns)
        assert native.number(definition, 43) == 0
        assert table['count'] == native.number(definition, 16, 4) == len(expected_rows) == len(table['rows'])
        assert table['values'] == {row[0]: dict(zip(columns, row, strict=True)) for row in expected_rows}
        assert counters[name] == (0, 0)
        assert not properties.get(owner) and not declarations[name]['pairs']
        for position, declaration in enumerate(declarations[name]['columns']):
            block = bytes.fromhex(declaration['raw_hex'])
            column = table['columns'][columns[position]]
            assert declaration['number'] == column['number'] == column['position'] == position
            assert declaration['native'] == column['native_type'] == 4
            assert declaration['flags'] == 3 and declaration['extended'] == 0
            assert native.number(block, 21) == position * 4
            assert native.number(block, 23) == column['length'] == 4
            assert not column['auto'] and not column['compressed']
            assert not properties.get(owner + ':Column:' + str(position))
        live = records.live_records(raw, table, maps)
        by_root = {item['page'] << 8 | item['slot']: identity for identity, item in table['rows'].items()}
        assert len(live) == len(by_root) == len(expected_rows)
        assert set(by_root) == {root for root, body in live}
        for root, body in live:
            reported = table['rows'][by_root[root]]
            assert reported['target_page'] == root >> 8 and reported['target_slot'] == root & 255
            assert len(body) == 2 + 4 * len(columns) + 1
            assert native.number(body, 0) == len(columns) and not body[-1] & ~((1 << len(columns)) - 1)
            assert body == reported['raw']
            decoded = {item['name']: records.scalar_value(body, item)
                       for item in declarations[name]['columns']}
            assert decoded == table['values'][by_root[root]]
        owned = set(maps[owner + ':Owned']['file_members'])
        assert set(maps[owner + ':Hint']['file_members']) <= owned
        # This original corpus is intentionally unforwarded, with no deleted slots.
        for number in owned:
            page = raw[number * PAGE:(number + 1) * PAGE]
            assert all(native.number(page, 14 + 2 * slot) & 0xe000 == 0
                       for slot in range(native.number(page, 12)))
        all_roles.append(owned)
        indexes = {}
        if name == 'Sentinel':
            assert not table['indexes'] and native.number(definition, 47, 4) == native.number(definition, 51, 4) == 0
        else:
            logical, pages = index_declarations(raw, table, maps)
            assert pages == definition_pages
            assert native.number(definition, 47, 4) == 4 and native.number(definition, 51, 4) == 2
            assert set(logical) == {'PrimaryID', 'PrimaryAlias', 'ValueOrder', 'ValueAlias'}
            expected_secondary = [('Right', False), ('Left', True)] if recipe['shape'] == 'tuple' else [('Left', True)]
            secondary_flags = 0x82 | int(recipe['unique']) | (8 if recipe['required'] else 0)
            physical = {}
            for index_name, index in table['indexes'].items():
                primary = index_name.startswith('Primary')
                position = 0 if primary else 1
                key_columns = [('ID', True)] if primary else expected_secondary
                flags = 0x89 if primary else secondary_flags
                metadata = logical[index_name]
                assert index['physical'] == position and index['columns'] == key_columns
                assert index['flags'] == flags and metadata['kind'] == (1 if index_name == 'PrimaryID' else 0)
                assert metadata['role'] == metadata['update'] == metadata['delete'] == 0
                assert metadata['target_index'] == 0xffffffff and metadata['target_table'] == 0
                entries, decoded = leaf(raw, index['root'], table['definition'], key_columns)
                eligible = []
                decoded_expected = {}
                for root, identity in by_root.items():
                    values = tuple(table['values'][identity][column] for column, ascending in key_columns)
                    if not primary and all(value is None for value in values):
                        continue
                    if recipe['required'] or primary:
                        assert all(value is not None for value in values)
                    wire = b''.join(component(value, ascending)
                                    for value, (column, ascending) in zip(values, key_columns, strict=True))
                    eligible.append((wire, root))
                    decoded_expected[root] = values
                eligible.sort()
                assert entries == [wire + root.to_bytes(4, 'big') for wire, root in eligible]
                assert decoded == [(decoded_expected[root], root) for wire, root in eligible]
                assert index['entries'] == len(eligible)
                assert index['history'] == len({wire for wire, root in eligible})
                assert index['rows'] == [(by_root[root], root >> 8, root & 255) for wire, root in eligible]
                if primary or recipe['unique']:
                    assert len({wire for wire, root in eligible}) == len(eligible)
                owned_index = maps[owner + ':IndexData:' + str(position)]['file_members']
                assert owned_index == [index['root']]
                if position in physical:
                    assert physical[position] == index['root']
                else:
                    physical[position] = index['root']
                    all_roles.append({index['root']})
                indexes[index_name] = {key: value for key, value in index.items() if key != 'rows'}
            assert len(physical) == 2 and len(set(physical.values())) == 2
        facts[name] = dict(definition=table['definition'], records=len(live),
                           logical_rows=sorted(by_root), indexes=indexes,
                           body_sha256=[hashlib.sha256(body).hexdigest() for root, body in live])
    selected = [role for role in claims
                if any(role.startswith('Table:' + str(tables[name]['definition']) + ':') for name in users)]
    ordinary.require_exclusive(claims, selected)
    metadata_pages = set()
    for claim in claims.values():
        metadata_pages.add(claim['container'])
        metadata_pages.update(claim['bitmaps'])
    for table in tables.values():
        definition, pages = native.declarations.definition(raw, table['definition'])
        metadata_pages.update(pages)
        assert counters[next(name for name, item in tables.items() if item is table)] == (
            int.from_bytes(definition[20:24], 'little', signed=True),
            int.from_bytes(definition[28:32], 'little', signed=True))
    free = set(maps['GlobalFree']['file_members'])
    for position, members in enumerate(all_roles):
        assert members.isdisjoint(metadata_pages | free)
        assert all(members.isdisjoint(other) for other in all_roles[:position])
    global_map = maps['GlobalFree']
    assert global_map['payload_type'] == 0
    start, end = global_map['row_start'], global_map['row_end']
    base = native.number(raw, start + 1, 4)
    eof = len(raw) // PAGE
    for number in range(eof, eof + 8):
        relative = number - base
        offset = start + 5 + relative // 8
        assert relative >= 0 and offset < end and raw[offset] & (1 << (relative % 8))
    catalog = records.catalog_long_values(raw, tables, declarations, maps, claims)
    return dict(profile=profile, pages=eof, tables=facts, catalog_long_values=catalog,
                absent_maps=absent, map_roles=sorted(maps), claims=claims,
                source_sha256=hashlib.sha256(raw).hexdigest())
