"""Closed new-source gates from native metadata, raw records and inert getters."""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_ordinary import row_codec, record_decode
from slot_preview_originals import facts
from parent_index_split_originals import wire
from ace16_insert_candidates import model as storage_wire
import delete_reference_maps as usage

PAGE = 4096
MAX_IMAGE = 8 * 1024 * 1024
KEYS = list(range(-226, 0)) + list(range(1, 226))
COMMON = [
    ('Earlier', 4, 4, 0), ('Flag', 1, 0, 0), ('ID', 4, 4, 4),
    ('Tiny', 2, 1, 8), ('Narrow', 3, 2, 9), ('Wide', 19, 8, 11),
    ('Money', 5, 8, 19), ('Single', 6, 4, 27), ('Double', 7, 8, 31),
    ('Moment', 8, 8, 39), ('Token', 15, 16, 47),
]
GUID = '{00112233-4455-6677-8899-AABBCCDDEEFF}'


def number(raw, at, width=4):
    result = raw[at:at + width]
    assert len(result) == width
    return int.from_bytes(result, 'little')


def columns(table):
    assert table in ('FixedRows', 'MixedRows')
    result = list(COMMON)
    if table == 'MixedRows':
        result += [('Compact', 10, 510, 0), ('Bytes', 9, 255, 0),
                   ('Raw', 10, 510, 0), ('Tail', 9, 255, 0)]
    return result + [('Spare', 4, 4, 63), ('LastFlag', 1, 0, 0)]


def parsed(text):
    seen = set()
    for line in text.splitlines():
        values = line.split('\t')
        tag = values[0]
        assert tag in {'TABLE', 'COLUMN', 'PROPERTY', 'ROW', 'RAWROW', 'INDEX', 'INDEX_COLUMN', 'ENTRY'}
        if tag in ('TABLE', 'COLUMN', 'ROW', 'RAWROW', 'INDEX'):
            identity = tuple(values[:2 if tag == 'TABLE' else 3])
        else:
            identity = tuple(values)
        assert identity not in seen, ('Duplicate getter fact', identity)
        seen.add(identity)
        if tag == 'COLUMN':
            assert values[7] in ('true', 'false')
        if tag == 'INDEX':
            assert all(value in ('true', 'false') for value in values[7:11])
        if tag == 'INDEX_COLUMN':
            assert values[-1] in ('true', 'false')
    tables = facts.parse(text)
    assert set(tables) == {'FixedRows', 'MixedRows', 'Sentinel'}
    sentinel = tables['Sentinel']
    assert sentinel['count'] == 1 and set(sentinel['rows']) == {91}
    assert list(sentinel['columns']) == ['ID', 'Marker'] and not sentinel['indexes']
    assert sentinel['properties'] == []
    assert sentinel['rows'][91]['values'] == ['Integer:91', 'Integer:12345']
    return tables


def schema(raw, table, name):
    expected = columns(name)
    count = len(expected)
    variables = 4 if name == 'MixedRows' else 0
    assert table['count'] == len(table['rows']) == len(table['raw_rows']) == 451
    assert table['columns_count'] == table['max_columns'] == count
    assert list(table['columns']) == [field[0] for field in expected]
    properties = [['ID', 'Required', '1', 'true', 'Boolean:true']]
    if variables:
        properties += [[field, 'AllowZeroLength', '1', 'true', 'Boolean:true'] for field in ('Compact', 'Raw')]
    assert sorted(table['properties']) == sorted(properties)
    assert set(table['indexes']) == {'PrimaryID', 'Twin'}
    primary = table['indexes']['PrimaryID']
    for label, index in table['indexes'].items():
        assert index['physical'] == 0 and index['entries'] == 451 and index['owned_pages'] == 1
        assert index['root'] == primary['root']
        assert index['primary'] == (label == 'PrimaryID')
        assert index['unique'] and index['required'] and not index['ignored_null']
        assert index['columns'] == [['ID', 'true']] and index['order'] == KEYS
    at = table['definition'] * PAGE
    assert raw[at:at + 2] == bytes([2, 1])
    assert number(raw, at + 16) == 451
    assert number(raw, at + 41, 2) == number(raw, at + 45, 2) == count
    assert number(raw, at + 43, 2) == variables
    assert number(raw, at + 47) == 2 and number(raw, at + 51) == 1
    fields = []
    start = at + 75
    for position, (label, kind, length, offset) in enumerate(expected):
        field = start + position * 25
        variable = name == 'MixedRows' and 11 <= position <= 14
        ordinal = position - 11 if variable else None
        flags = 2 if variable else 3
        extension = 1 if label == 'Compact' else 0
        assert raw[field] == kind and number(raw, field + 5, 2) == position
        assert raw[field + 15] == flags and raw[field + 16] == extension
        declared = number(raw, field + 23, 2)
        assert number(raw, field + 21, 2) == offset
        if kind == 1:
            assert declared in (0, 1)
        else:
            assert declared == length
        if variable:
            assert number(raw, field + 7, 2) == ordinal
        public = table['columns'][label]
        assert (public['position'], public['number'], public['native_type'], public['declared_bytes']) == (position, position, kind, declared)
        assert public['compressed'] is (extension == 1)
        if variable:
            assert public['variable'] == ordinal
        fields.append({'name': label, 'number': position, 'kind': kind, 'length': length,
                       'offset': offset, 'variable': ordinal, 'flags': flags, 'extended_flags': extension,
                       'declared_length': declared})
    cursor = start + count * 25
    for label, *_ in expected:
        length = number(raw, cursor, 2)
        assert raw[cursor + 2:cursor + 2 + length].decode('utf-16le') == label
        cursor += 2 + length
    assert number(raw, cursor + 4, 2) == 2 and raw[cursor + 6] == 1
    assert all(number(raw, cursor + 4 + slot * 3, 2) == 0xffff for slot in range(1, 10))
    assert raw[cursor + 46] == 0x89 and number(raw, cursor + 38) == primary['root']
    physical = row_codec.Layout.bind(raw, table)
    assert physical.fixed_end == 69
    return {'definition': table['definition'], 'root': primary['root'], 'fields': fields,
            'max_columns': count, 'variable_slots': variables, 'fixed_end': physical.fixed_end,
            'owned': number(raw, at + 55), 'hint': number(raw, at + 59),
            'index_owned': number(raw, cursor + 34), 'global': 1 << 8,
            'table_count': at + 16, 'history': at + 67, 'id': 2,
            'historical_before': number(raw, at + 67)}


def expected_values(identity, mixed):
    odd = bool(identity & 1)
    result = [
        ('int32', 1_000_000 + identity) if odd else ('null', None),
        ('boolean', odd), ('int32', identity), ('byte', 255),
        ('int16', -32768), ('int64', -(1 << 63)),
        ('currency-scaled', -123456), ('single-bits', 0x80000000),
        ('double-bits', 0x3ff8000000000000), ('date-bits', 0),
        ('guid', GUID),
    ]
    if mixed:
        result += [('text', 'ABC'), ('binary', bytes.fromhex('00017f80feff')),
                   ('text', 'Ā'), ('binary', b'') if odd else ('null', None)]
    result += [('int32', -999), ('boolean', bool(identity & 2))]
    return result


def storage(raw, text, layout):
    seen = set()
    for line in text.splitlines():
        fields = line.split('\t')
        identity = tuple(fields) if fields[0] == 'MEMBER' else tuple(fields[:2])
        assert identity not in seen
        seen.add(identity)
    public = usage.parse(text)
    assert public['page_bytes'] == PAGE and public['page_count'] == len(raw) // PAGE
    assert set(public['maps']) == {'JackcessGlobalFree', 'TableOwned', 'TableFreeSpace'}
    assert not public['indexes']
    maps = {}
    for label, role in [('JackcessGlobalFree', 'global'), ('TableOwned', 'owned'), ('TableFreeSpace', 'hint')]:
        described = usage.decode_map(raw, public['maps'][label], PAGE, len(raw) // PAGE)
        direct = storage_wire.map_record(raw, layout[role])
        assert described['row_start'] == direct['range'][0]
        assert set(described['file_members']) == {page for page in direct['members'] if page < len(raw) // PAGE}
        maps[role] = direct
    maps['index_owned'] = storage_wire.map_record(raw, layout['index_owned'])
    return maps


def inspect(case, path, getter_path, maps_path):
    raw = Path(path).read_bytes()
    assert 0 < len(raw) <= MAX_IMAGE and len(raw) % PAGE == 0
    assert raw[:4] == bytes([0, 1, 0, 0]) and raw[4:20] == b'Standard ACE DB\0'
    assert raw[20] == {'V2016': 5, 'V2019': 6}[case['profile']]
    assert raw[21:24] == bytes(3) and raw[62:66] == bytes.fromhex('fb8abc4e')
    tables = parsed(Path(getter_path).read_text())
    table = tables[case['table']]
    layout = schema(raw, table, case['table'])
    assert set(table['rows']) == set(KEYS)
    physical = row_codec.Layout.bind(raw, table)
    sentinel = tables['Sentinel']
    sentinel_layout = row_codec.Layout.bind(raw, sentinel)
    sentinel_body = bytes.fromhex(sentinel['raw_rows'][91])
    sentinel_values = record_decode.decode(sentinel_layout, sentinel_body)
    record_decode.same_values(sentinel_values, [('int32', 91), ('int32', 12345)])
    record_decode.check_getter(sentinel_layout, sentinel_values, sentinel['rows'][91]['values'])
    sentinel_row = sentinel['rows'][91]
    sentinel_start, sentinel_end = storage_wire.frame(raw, sentinel_row['page'], sentinel_row['slot'])
    assert raw[sentinel_start:sentinel_end] == sentinel_body
    assert number(raw, sentinel_row['page'] * PAGE + 4) == sentinel['definition']
    references = {}
    populations = {}
    for identity, row in table['rows'].items():
        reference = (row['page'] << 8) | row['slot']
        assert reference not in references and 0 <= row['slot'] < 255
        references[reference] = identity
        body = bytes.fromhex(table['raw_rows'][identity])
        decoded = record_decode.decode(physical, body)
        record_decode.same_values(decoded, expected_values(identity, case['table'] == 'MixedRows'))
        record_decode.check_getter(physical, decoded, row['values'])
        # This byte count certifies the finite pressure witness, not canonical
        # source serialization. NULL payloads and valid Text wires are decoded.
        assert len(body) == (97 if case['table'] == 'MixedRows' else 71)
        start, end = storage_wire.frame(raw, row['page'], row['slot'])
        assert raw[start:end] == body
        assert number(raw, row['page'] * PAGE + 4) == layout['definition']
        populations.setdefault(row['page'], []).append(row['slot'])
    for page, slots in populations.items():
        assert sorted(slots) == list(range(len(slots)))
        at = page * PAGE
        assert raw[at:at + 2] == bytes([1, 1])
        assert number(raw, at + 12, 2) == len(slots)
        last = number(raw, at + 14 + (len(slots) - 1) * 2, 2)
        assert last & ~0x1fff == 0
        assert number(raw, at + 2, 2) == last - 14 - 2 * len(slots)
        assert number(raw, at + 2, 2) == (23 if case['table'] == 'MixedRows' else 4082 - len(slots) * 73)
    expected_populations = [41] * 11 if case['table'] == 'MixedRows' else [55] * 8 + [11]
    assert sorted(map(len, populations.values())) == sorted(expected_populations)
    maps = storage(raw, Path(maps_path).read_text(), layout)
    nodes, leaves, entries = wire.tree(raw, layout['root'], layout['definition'])
    assert entries == sorted(storage_wire.int_key(identity, reference) for reference, identity in references.items())
    assert len(nodes) == len(leaves) == 1
    root = nodes[layout['root']]
    assert root['height'] == 0 and root['prefix'] == 1 and root['free'] == 7
    assert (root['previous'], root['next'], root['tail']) == (0, 0, 0)
    assert maps['owned']['members'] == set(populations)
    assert maps['index_owned']['members'] == set(nodes)
    assert maps['hint']['members'] <= maps['owned']['members']
    assert maps['hint']['members']
    highest = max(maps['hint']['members'])
    assert number(raw, highest * PAGE + 2, 2) == (23 if case['table'] == 'MixedRows' else 3279)
    assert not maps['global']['members'] & (set(populations) | set(nodes))
    eof = len(raw) // PAGE
    for page in range(eof, eof + 3):
        assert page in maps['global']['members'] and page in maps['global']['locations']
        assert page in maps['index_owned']['locations']
    if case['table'] == 'MixedRows':
        assert eof in maps['owned']['locations'] and eof in maps['hint']['locations']
    return raw, table, layout, maps, nodes, leaves
