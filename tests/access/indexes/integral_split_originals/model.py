"""Independent closed integral source gates; no process or file writes at import."""
from decimal import Decimal
import hashlib
from pathlib import Path

from slot_preview_originals.facts import parse as parse_getters
from delete_reference_maps import decode_map, parse as parse_maps
from parent_index_split_originals.wire import future_cell

PAGE = 4096
ENTRY = 480
MASK = 27
CAPACITY = PAGE - ENTRY
ROW_BYTES = 4
CHILD_BYTES = 4
HEADER = 63
COUNTER_BYTES = 12
COLUMN_BYTES = 25
PHYSICAL_COUNT = 51
COLUMN_FLAGS = 15
EXTENDED_FLAGS = 16
FIXED_OFFSET = 21
COLUMN_NAME_BYTES = 2
PHYSICAL_BYTES = 52
USAGE = 34
ROOT = 38
INDEX_FLAGS = 46
SLOT_START = 14
SLOT_BYTES = 2
SLOT_MASK = 0x1FFF
SLOT_COUNT = 12
MAX_DEPTH = 16
MAX_BYTES = 64 * 1024 * 1024
MAX_ROWS = 4096
ENCODING_FIELD_OFFSET = 62
ENCODING_FIELD_MASK = bytes.fromhex('fb8abc4e')
KINDS = {
    'ByteKeys': (2, 1, 900, 'byte', 255),
    'Int16Keys': (3, 2, 900, 'int16', 32767),
    'Int64Keys': (0x13, 8, 400, 'int64', (1 << 63) - 1),
    'CurrencyKeys': (5, 8, 400, 'currency-scaled', (1 << 63) - 1),
}


def number(data, offset, width=4):
    assert 0 <= offset <= len(data) - width
    return int.from_bytes(data[offset:offset + width], 'little')


def wire(value, reference, width, byte, descending):
    if byte:
        assert 0 <= value <= 255
        payload = bytes([value])
    else:
        limit = 1 << (8 * width)
        assert -(limit // 2) <= value < limit // 2
        payload = ((value % limit) ^ (limit // 2)).to_bytes(width, 'big')
    if descending:
        payload = bytes(part ^ 255 for part in payload)
    return bytes([128 if descending else 127]) + payload + reference.to_bytes(ROW_BYTES, 'big')


def size(entries):
    assert entries
    shared = len(entries[0])
    for entry in entries[1:]:
        shared = min(shared, len(entry))
        shared = next((position for position in range(shared)
                       if entry[position] != entries[0][position]), shared)
    return sum(map(len, entries)) - shared * (len(entries) - 1)


def tree(raw, root, owner, width):
    seen = {}
    levels = {}

    def visit(number_value, depth):
        assert depth < MAX_DEPTH and number_value not in seen
        body = raw[number_value * PAGE:(number_value + 1) * PAGE]
        assert len(body) == PAGE and body[:2] in (b'\x03\x01', b'\x04\x01')
        assert number(body, 4) == owner and body[8:12] == bytes(4)
        height = body[26]
        leaf = body[0] == 4
        assert leaf == (height == 0)
        prefix = number(body, 24, 2)
        key_row_bytes = 1 + width + ROW_BYTES
        assert prefix <= key_row_bytes + (0 if leaf else CHILD_BYTES)
        entries = []
        previous = 0
        for end in range(1, CAPACITY + 1):
            if not body[MASK + end // 8] & (1 << (end % 8)):
                continue
            full = (entries[0][:prefix] if entries else b'') + body[ENTRY + previous:ENTRY + end]
            assert len(full) == key_row_bytes + (0 if leaf else CHILD_BYTES)
            entries.append(full)
            previous = end
        assert entries and number(body, 2, 2) == CAPACITY - previous
        current = {'height': height, 'previous': number(body, 12), 'next': number(body, 16),
                   'tail': number(body, 20), 'prefix': prefix, 'free': number(body, 2, 2),
                   'entries_hex': [entry.hex() for entry in entries],
                   'sha256': hashlib.sha256(body).hexdigest()}
        seen[number_value] = current
        levels.setdefault(height, []).append(number_value)
        if leaf:
            assert current['tail'] == 0
            return entries
        result = []
        for entry in entries:
            child = int.from_bytes(entry[-CHILD_BYTES:], 'big')
            values = visit(child, depth + 1)
            assert seen[child]['height'] + 1 == height and values[-1] == entry[:-CHILD_BYTES]
            result.extend(values)
        if current['tail']:
            values = visit(current['tail'], depth + 1)
            assert seen[current['tail']]['height'] + 1 == height
            result.extend(values)
        return result

    keys = visit(root, 0)
    for pages in levels.values():
        for position, page in enumerate(pages):
            assert seen[page]['previous'] == (pages[position - 1] if position else 0)
            assert seen[page]['next'] == (pages[position + 1] if position + 1 < len(pages) else 0)
    return {'pages': seen, 'keys_hex': [entry.hex() for entry in keys]}


def map_metadata(raw, reference, members, count):
    page, slot = divmod(reference, 256)
    body = raw[page * PAGE:(page + 1) * PAGE]
    assert body[:2] == b'\x01\x01' and number(body, 4) == 0
    assert slot < number(body, SLOT_COUNT, SLOT_BYTES)
    start = number(body, SLOT_START + slot * SLOT_BYTES, SLOT_BYTES) & SLOT_MASK
    end = number(body, SLOT_START + (slot - 1) * SLOT_BYTES, SLOT_BYTES) & SLOT_MASK if slot else PAGE
    return {'table_page': page, 'row_start': start, 'row_end': end,
            'file_members': sorted(members), 'reported_count': count}


def split_description(text):
    before, text = text.split('BEGIN_GETTERS\n', 1)
    assert not before
    getters, rest = text.split('END_GETTERS\n', 1)
    maps = {}
    while rest:
        line, rest = rest.split('\n', 1)
        assert line.startswith('BEGIN_MAPS\t')
        table = line.split('\t')[1]
        block, rest = rest.split('END_MAPS\t' + table + '\n', 1)
        lines = block.splitlines()
        location = [line.split('\t')[1:] for line in lines if line.startswith('LOCATION\t')]
        assert len(location) == 1
        maps[table] = parse_maps('\n'.join(line for line in lines if not line.startswith('LOCATION\t')))
        maps[table]['location'] = list(map(int, location[0]))
    return parse_getters(getters), maps



def qualify_header(raw, profile):
    assert raw[4:20] == b'Standard ACE DB\0' and raw[20] == {'V2016': 5, 'V2019': 6}[profile]
    assert raw[:4] == b'\x00\x01\x00\x00' and raw[21:24] == bytes(3)
    field = raw[ENCODING_FIELD_OFFSET:ENCODING_FIELD_OFFSET + len(ENCODING_FIELD_MASK)]
    decoded_key = bytes(value ^ mask for value, mask in zip(field, ENCODING_FIELD_MASK, strict=True))
    assert decoded_key == bytes(len(ENCODING_FIELD_MASK))

def certify(path, profile, description):
    assert Path(path).stat().st_size <= MAX_BYTES
    raw = Path(path).read_bytes()
    assert len(raw) <= MAX_BYTES and len(raw) % PAGE == 0
    qualify_header(raw, profile)
    tables, map_reports = split_description(description)
    expected_tables = set(KINDS) | {name + 'Unique' for name in KINDS} | {'Preserved'}
    assert set(tables) == expected_tables
    assert sum(table['count'] for table in tables.values()) == 2729 <= MAX_ROWS
    facts = {}
    for name, (native_type, width, count, kind, replacement) in KINDS.items():
        print('Checking source geometry:', profile, name, flush=True)
        table = tables[name]
        assert table['count'] == len(table['rows']) == count
        assert set(table['columns']) == {'ID', 'KeyValue', 'Marker'}
        assert set(table['indexes']) == {'PrimaryID', 'Ascending', 'Descending', 'Alias'}
        selected = table['columns']['KeyValue']
        assert (selected['native_type'], selected['declared_bytes']) == (native_type, width)
        required = [property for property in table['properties']
                    if property[0:2] == ['KeyValue', 'Required']]
        assert len(required) == 1 and required[0][-1] == 'Boolean:true'
        definition = raw[table['definition'] * PAGE:(table['definition'] + 1) * PAGE]
        physical_count = number(definition, PHYSICAL_COUNT)
        column_start = HEADER + physical_count * COUNTER_BYTES
        field = column_start + selected['position'] * COLUMN_BYTES
        assert definition[field + COLUMN_FLAGS] == 3 and definition[field + EXTENDED_FLAGS] == 0
        physical_start = column_start + table['columns_count'] * COLUMN_BYTES
        for column in table['columns']:
            length = number(definition, physical_start, COLUMN_NAME_BYTES)
            physical_start += COLUMN_NAME_BYTES
            actual_name = definition[physical_start:physical_start + length].decode('utf-16le')
            assert actual_name == column
            physical_start += length
        references = {}
        for identity, row in table['rows'].items():
            assert row['values'][0] == 'Integer:' + str(identity)
            key = row['values'][1].split(':', 1)[1]
            if native_type == 5:
                assert row['values'][1].startswith('BigDecimal:')
                units = Decimal(key) * 10000
            else:
                units = int(key)
            assert units == 0 and row['values'][2] == 'Integer:' + str(identity * 17)
            reference = row['page'] << 8 | row['slot']
            assert reference not in references and row['slot'] < 255
            references[reference] = identity
            body = bytes.fromhex(table['raw_rows'][identity])
            assert number(body, 0, 2) == table['max_columns'] == 3
            for label, payload in [('ID', identity.to_bytes(4, 'little', signed=True)),
                                   ('KeyValue', bytes(width)),
                                   ('Marker', (identity * 17).to_bytes(4, 'little', signed=True))]:
                metadata = table['columns'][label]
                at = column_start + metadata['position'] * COLUMN_BYTES
                offset = 2 + number(definition, at + FIXED_OFFSET, 2)
                assert offset + len(payload) <= len(body) - 1
                assert body[offset:offset + len(payload)] == payload
                assert body[-1] & (1 << metadata['number'])
            slot_offset = row['page'] * PAGE + SLOT_START + row['slot'] * SLOT_BYTES
            word = number(raw, slot_offset, SLOT_BYTES)
            assert word & ~SLOT_MASK == 0
            start = row['page'] * PAGE + word
            end = row['page'] * PAGE + (number(raw, slot_offset - SLOT_BYTES, SLOT_BYTES) & SLOT_MASK) if row['slot'] else (row['page'] + 1) * PAGE
            assert raw[start:end] == body
        report = map_reports[name]
        decoded_maps = {label: decode_map(raw, metadata, PAGE, len(raw) // PAGE)
                        for label, metadata in report['maps'].items()}
        for label, decoded in decoded_maps.items():
            page = decoded['map_page']
            expected_owner = 1 if label == 'JackcessGlobalFree' else 0
            assert number(raw, page * PAGE + 4) == expected_owner
            if label == 'JackcessGlobalFree':
                assert page == 1 and decoded['map_slot'] == 0
        owned = decoded_maps['TableOwned']['file_members']
        ordinal = 0
        selected_ordinal = None
        for page in owned:
            body = raw[page * PAGE:(page + 1) * PAGE]
            assert body[:2] == b'\x01\x01' and number(body, 4) == table['definition']
            for slot in range(number(body, SLOT_COUNT, SLOT_BYTES)):
                word = number(body, SLOT_START + slot * SLOT_BYTES, SLOT_BYTES)
                assert word & ~SLOT_MASK == 0
                reference = page << 8 | slot
                assert reference in references
                if references[reference] == 1:
                    selected_ordinal = ordinal
                ordinal += 1
        assert ordinal == count and selected_ordinal is not None
        original_reference = next(reference for reference, identity in references.items() if identity == 1)
        location = report['location']
        assert location == [original_reference >> 8, original_reference & 255] * 2
        indexes = {}
        affected = {}
        for label, index in table['indexes'].items():
            assert len(index['columns']) == 1 and not index['ignored_null']
            column, ascending = index['columns'][0]
            descending = ascending != 'true'
            data_width = width if column == 'KeyValue' else 4
            assert column in ('KeyValue', 'ID')
            geometry = tree(raw, index['root'], table['definition'], data_width)
            keys = [wire(0 if column == 'KeyValue' else identity, reference,
                         data_width, native_type == 2 and column == 'KeyValue', descending)
                    for reference, identity in references.items()]
            keys.sort()
            assert geometry['keys_hex'] == [key.hex() for key in keys]
            assert index['order'] == [references[int.from_bytes(key[-ROW_BYTES:], 'big')] for key in keys]
            indexes[label] = geometry
            if column != 'KeyValue':
                continue
            assert ascending == ('false' if label == 'Descending' else 'true')
            assert index['required'] and not index['unique'] and not index['primary']
            assert geometry['pages'][index['root']]['height'] == 0
            updated = [wire(replacement if identity == 1 else 0, reference,
                            width, native_type == 2, descending)
                       for reference, identity in references.items()]
            updated.sort()
            assert size(updated) > CAPACITY
            middle = len(updated) // 2
            assert size(updated[:middle]) <= CAPACITY and size(updated[middle:]) <= CAPACITY
            at = physical_start + index['physical'] * PHYSICAL_BYTES
            assert number(definition, at + ROOT) == index['root']
            metadata = map_metadata(raw, number(definition, at + USAGE), geometry['pages'], index['owned_pages'])
            decode_map(raw, metadata, PAGE, len(raw) // PAGE)
            affected[index['physical']] = metadata
            geometry['updated_best_bytes'] = size(updated)
            geometry['updated_half_best_bytes'] = [size(updated[:middle]), size(updated[middle:])]
            geometry['physical_flags'] = definition[at + INDEX_FLAGS]
        assert len(affected) >= 2
        eof = len(raw) // PAGE
        future = []
        for physical, metadata in affected.items():
            for page in range(eof, eof + 2 * len(affected)):
                own = future_cell(raw, metadata, page)
                global_free = future_cell(raw, report['maps']['JackcessGlobalFree'], page)
                assert not own['set'] and global_free['set']
                future.append({'physical': physical, 'owned': own, 'global': global_free})
        facts[name] = {'count': count, 'native_type': native_type, 'width': width,
                       'selected_ordinal': selected_ordinal, 'selected_reference': original_reference,
                       'kind': kind, 'physical_split_indexes': sorted(affected),
                       'properties': table['properties'], 'indexes': indexes,
                       'maps': decoded_maps, 'future_cells': future}
    return {'profile': profile, 'tables': facts, 'all_tables': tables,
            'source_bytes': len(raw), 'source_sha256': hashlib.sha256(raw).hexdigest()}
