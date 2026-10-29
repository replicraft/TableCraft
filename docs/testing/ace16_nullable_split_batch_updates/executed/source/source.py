"""Independent source-only gates for nullable ordinary/IgnoreNULL physical owners.

Only raw reader/census primitives are reused. No AFTER or engineering constructor
is imported, and native source packing, payload residue and ROWs stay observed.
"""
import hashlib
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_fixed_ignore_null_updates import source as previous

native = previous.native
ordinary = previous.ordinary
records = previous.records
PAGE = 4096


def signed(ordinal):
    assert 0 <= ordinal < 1000
    return -(1 << 31) + ordinal * 17 if ordinal % 2 == 0 else (1 << 31) - 1 - ordinal * 17


def source_rows(sentinel):
    if sentinel:
        return [[91, -12345]]
    return [[ordinal + 1, None if ordinal < 300 else signed(ordinal), None]
            for ordinal in range(500)]


def leaf(raw, root, owner, columns):
    assert 0 < root < len(raw) // PAGE
    block = raw[root * PAGE:(root + 1) * PAGE]
    assert block[:2] == b'\x04\x01' and native.number(block, 4, 4) == owner
    assert block[26] == 0 and block[8:24] == bytes(16)
    prefix = native.number(block, 24)
    assert prefix <= 5 * len(columns) + 4
    assert not block[27] & 1 and not block[479] & 0xfe
    ends = [position for position in range(1, 3617)
            if block[27 + position // 8] & (1 << (position % 8))]
    assert (ends[-1] if ends else 0) == 3616 - native.number(block, 2)
    entries = []
    decoded = []
    start = 0
    for end in ends:
        assert start < end <= 3616
        payload = block[480 + start:480 + end]
        complete = payload if not entries else entries[-1][:prefix] + payload
        assert not entries or prefix <= len(entries[-1])
        current = previous.decode_entry(complete, columns)
        if entries:
            assert (entries[-1][:-4], entries[-1][-4:]) < (complete[:-4], complete[-4:])
        entries.append(complete)
        decoded.append(current)
        start = end
    return entries, decoded, {
        'root': root, 'prefix': prefix, 'free': native.number(block, 2),
        'active_bytes': 3616 - native.number(block, 2),
        'header_hex': block[:480].hex(), 'raw_hex': block.hex(),
        'uncompressed_bytes': sum(map(len, entries)),
    }


def future(raw, description, number):
    start, end = description['row_start'], description['row_end']
    payload = raw[start:end]
    if payload[0] == 0:
        base = native.number(payload, 1, 4)
        relative = number - base
        if relative < 0 or relative >= (len(payload) - 5) * 8:
            return None
        offset = start + 5 + relative // 8
    else:
        assert payload[0] == 1
        block, relative = divmod(number, (PAGE - 4) * 8)
        pointer_offset = 1 + block * 4
        if pointer_offset + 4 > len(payload):
            return None
        pointer = native.number(payload, pointer_offset, 4)
        if pointer == 0:
            return None
        assert pointer < len(raw) // PAGE
        offset = pointer * PAGE + 4 + relative // 8
    mask = 1 << (relative % 8)
    return {'number': number, 'offset': offset, 'mask': mask, 'set': bool(raw[offset] & mask)}


def inspect(path, getter, profile, direction):
    assert profile in ('V2016', 'V2019') and direction in ('ASC', 'DESC')
    ascending = direction == 'ASC'
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= 8 * 1024 * 1024 and len(raw) % PAGE == 0
    assert raw[4:20] == b'Standard ACE DB\0' and raw[20] == (5 if profile == 'V2016' else 6)
    tables, descriptions, properties, absent, counters = previous.parse(Path(getter).read_text())
    users = {'Items', 'UniqueItems', 'Sentinel'}
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
    facts = {}
    data_roles = []
    for name in sorted(users):
        sentinel = name == 'Sentinel'
        expected_rows = source_rows(sentinel)
        columns = ['ID', 'Marker'] if sentinel else ['ID', 'Group', 'Score']
        table = tables[name]
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
        rows = []
        for root, body in live:
            identity = by_root[root]
            reported = table['rows'][identity]
            assert reported['target_page'] == root >> 8 and reported['target_slot'] == root & 255
            assert len(body) == 2 + 4 * len(columns) + 1
            assert native.number(body, 0) == len(columns) and not body[-1] & ~((1 << len(columns)) - 1)
            assert body == reported['raw']
            decoded = {item['name']: records.scalar_value(body, item) for item in declarations[name]['columns']}
            assert decoded == table['values'][identity]
            rows.append({'id': identity, 'root': root, 'raw_hex': body.hex(), 'values': decoded})
        assert [row['id'] for row in rows] == [row[0] for row in expected_rows]
        owned = set(maps[owner + ':Owned']['file_members'])
        assert set(maps[owner + ':Hint']['file_members']) <= owned
        for number in owned:
            block = raw[number * PAGE:(number + 1) * PAGE]
            assert all(native.number(block, 14 + 2 * slot) & 0xe000 == 0
                       for slot in range(native.number(block, 12)))
        data_roles.append(owned)
        indexes = {}
        if sentinel:
            assert not table['indexes']
            assert native.number(definition, 47, 4) == native.number(definition, 51, 4) == 0
        else:
            logical, pages = previous.index_declarations(raw, table, maps)
            assert pages == definition_pages
            assert native.number(definition, 47, 4) == 4 and native.number(definition, 51, 4) == 2
            assert set(logical) == {'LeftOrder', 'LeftAlias', 'PairOrder', 'PairAlias'}
            physical = {}
            for index_name, index in table['indexes'].items():
                pair = index_name.startswith('Pair')
                position = int(pair)
                unique = name == 'UniqueItems'
                key_columns = [('Score', not ascending), ('Group', ascending)] if pair else [('Group', ascending)]
                flags = 0x80 | int(unique) | (2 if pair else 0)
                metadata = logical[index_name]
                assert index['physical'] == position and index['columns'] == key_columns
                assert index['flags'] == flags and not index['primary'] and metadata['kind'] == 0
                assert metadata['role'] == metadata['update'] == metadata['delete'] == 0
                assert metadata['target_index'] == 0xffffffff and metadata['target_table'] == 0
                entries, decoded_entries, framing = leaf(raw, index['root'], table['definition'], key_columns)
                eligible = []
                for root, identity in by_root.items():
                    values = tuple(table['values'][identity][column] for column, _ in key_columns)
                    if pair and all(value is None for value in values):
                        continue
                    key = b''.join(previous.component(value, order)
                                   for value, (_, order) in zip(values, key_columns, strict=True))
                    eligible.append((key, root, values))
                eligible.sort(key=lambda item: (item[0], item[1]))
                assert entries == [key + root.to_bytes(4, 'big') for key, root, _ in eligible]
                assert decoded_entries == [(values, root) for _, root, values in eligible]
                assert index['entries'] == len(eligible) == (200 if pair else 500)
                assert index['history'] == (200 if pair else 201)
                assert framing['uncompressed_bytes'] == (2000 if pair else 3300)
                assert index['rows'] == [(by_root[root], root >> 8, root & 255) for _, root, _ in eligible]
                if unique:
                    represented = [key for key, _, values in eligible if not all(value is None for value in values)]
                    assert len(set(represented)) == len(represented)
                owned_index = maps[owner + ':IndexData:' + str(position)]['file_members']
                assert owned_index == [index['root']]
                if position in physical:
                    assert physical[position] == index['root']
                else:
                    physical[position] = index['root']
                    data_roles.append({index['root']})
                indexes[index_name] = dict(index, framing=framing, entry_hex=[entry.hex() for entry in entries])
            assert len(physical) == len(set(physical.values())) == 2
        facts[name] = {'definition': table['definition'], 'definition_pages': definition_pages,
                       'definition_hex': definition.hex(), 'columns': declarations[name]['columns'],
                       'rows': rows, 'records': len(rows), 'indexes': indexes,
                       'owned': sorted(owned), 'hint': maps[owner + ':Hint']['file_members']}
    selected = [role for role in claims if any(
        role.startswith('Table:' + str(tables[name]['definition']) + ':') for name in users)]
    ordinary.require_exclusive(claims, selected)
    metadata_pages = set()
    for claim in claims.values():
        metadata_pages.add(claim['container'])
        metadata_pages.update(claim['bitmaps'])
    for name, table in tables.items():
        definition, pages = native.declarations.definition(raw, table['definition'])
        metadata_pages.update(pages)
        assert counters[name] == (int.from_bytes(definition[20:24], 'little', signed=True),
                                  int.from_bytes(definition[28:32], 'little', signed=True))
    free = set(maps['GlobalFree']['file_members'])
    for position, members in enumerate(data_roles):
        assert members.isdisjoint(metadata_pages | free)
        assert all(members.isdisjoint(other) for other in data_roles[:position])
    eof = len(raw) // PAGE
    future_global = [future(raw, maps['GlobalFree'], number) for number in range(eof, eof + 8)]
    assert all(cell is not None and cell['set'] for cell in future_global)
    future_index = {}
    for name in ('Items', 'UniqueItems'):
        owner = 'Table:' + str(tables[name]['definition'])
        for physical in range(2):
            role = owner + ':IndexData:' + str(physical)
            mapping = maps[role]
            cells = [future(raw, mapping, number) for number in range(eof, eof + 8)]
            assert all(cell is None or not cell['set'] for cell in cells)
            if any(cell is None for cell in cells):
                assert mapping['payload_type'] == 0 and mapping['row_end'] - mapping['row_start'] == 69
            future_index[name + ':' + str(physical)] = cells
    catalog = records.catalog_long_values(raw, tables, declarations, maps, claims)
    return {'profile': profile, 'direction': direction, 'pages': eof, 'tables': facts,
            'catalog_long_values': catalog, 'absent_maps': absent, 'maps': maps,
            'claims': claims, 'future_global': future_global, 'future_indexes': future_index,
            'source_sha256': hashlib.sha256(raw).hexdigest()}
