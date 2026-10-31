"""Original-only native framing and SDK observations; no construction/processes."""
import base64
import hashlib

from ace16_insert_candidates import model as frames

PAGE = 4096
FIXED_FLAG = 1
BOOL_NATIVE = 1
I32_NATIVE = 4
PHYSICAL_HEADER_MAGIC = 1923
LOGICAL_HEADER_MAGIC = 1625
LOGICAL_NUMBER_OFFSET = 4
LOGICAL_DATA_NUMBER_OFFSET = 8
LOGICAL_KIND_OFFSET = 23


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(raw, at, width=4):
    data = raw[at:at + width]
    require(len(data) == width, 'Truncated original integer')
    return int.from_bytes(data, 'little')


def sdk_inventory(text):
    lines = text.splitlines()
    require(bool(lines), 'Empty original SDK observations')
    rows = [line.split('\t') for line in lines]
    allowed = {'TABLE', 'COLUMN', 'OWNED_MAP', 'INDEX', 'ROW', 'PROPERTY',
               'COMPLEX', 'ATTACHMENT'}
    require(all(row[0] in allowed for row in rows), 'Unknown SDK observation')
    tables = {}
    for row in rows:
        if row[0] != 'TABLE':
            continue
        require(len(row) == 5 and row[1] not in tables, 'SDK TABLE identity')
        tables[row[1]] = {
            'name': row[1], 'rows': int(row[2]), 'page': int(row[3]),
            'maximum': int(row[4]), 'columns': [], 'indexes': [], 'records': [],
        }
    for row in rows:
        if row[0] in {'TABLE', 'COMPLEX', 'ATTACHMENT'}:
            continue
        require(row[1] in tables, 'SDK table-bound observation')
        table = tables[row[1]]
        if row[0] == 'COLUMN':
            require(len(row) == 12, 'SDK COLUMN arity')
            table['columns'].append({
                'name': row[2], 'native': int(row[3]), 'number': int(row[4]),
                'ordinal': int(row[5]), 'variable': row[6] == 'true',
                'offset': int(row[7]), 'variable_index': int(row[8]),
                'length': int(row[9]), 'calculated': row[10] == 'true',
                'auto': row[11] == 'true',
            })
        elif row[0] == 'OWNED_MAP':
            require(len(row) == 4 and 'map' not in table, 'SDK OWNED_MAP identity')
            table['map'] = [int(row[2]), int(row[3])]
        elif row[0] == 'INDEX':
            require(len(row) == 12, 'SDK INDEX arity')
            keys = []
            for key in row[5].split(','):
                name, ascending = key.rsplit(':', 1)
                require(ascending in {'true', 'false'}, 'SDK index direction')
                keys.append([name, ascending == 'true'])
            table['indexes'].append({
                'name': row[2], 'primary': row[3] == 'true',
                'unique': row[4] == 'true', 'keys': keys,
                'physical': int(row[6]), 'root': int(row[7]),
                'map': [int(row[8]), int(row[9])],
                'flags': int(row[10]), 'history': int(row[11]),
            })
        elif row[0] == 'ROW':
            require(len(row) == 4 + len(table['columns']), 'SDK ROW arity')
            page, slot = map(int, row[3].split(':'))
            table['records'].append({
                'ordinal': int(row[2]), 'reference': page << 8 | slot,
                'cells': row[4:],
            })
    require(set(tables) >= {'PreservedRows', 'Sentinel'}, 'Required original tables')
    require(len(tables) == 3, 'Exact two user tables plus attachment flat table')
    for table in tables.values():
        require(len(table['records']) == table['rows'], 'SDK row cardinality')
        require([row['ordinal'] for row in table['records']] == list(range(table['rows'])),
                'SDK source ordinal sequence')
    return {'tables': tables, 'records': rows, 'lines': lines}


def definition(raw, observed):
    page = observed['page']
    require(0 < page < len(raw) // PAGE, 'Original definition page')
    body = raw[page * PAGE:(page + 1) * PAGE]
    require(body[:2] == bytes([2, 1]) and number(body, 4) == 0,
            'Bounded single-page original definition')
    length = number(body, 8)
    maximum = number(body, 41, 2)
    count = number(body, 45, 2)
    physical_count = number(body, 51)
    logical_count = number(body, 47)
    require(12 <= length <= PAGE and maximum == count == observed['maximum'],
            'Complete original field extent')
    start = 63 + 12 * physical_count
    position = start + 25 * count
    fields = []
    for ordinal in range(count):
        at = start + 25 * ordinal
        width = number(body, position, 2)
        position += 2
        require(position + width <= length and width % 2 == 0, 'Original field name')
        name = body[position:position + width].decode('utf-16le')
        position += width
        field = {
            'name': name, 'number': number(body, at + 5, 2), 'native': body[at],
            'flags': body[at + 15], 'extended': body[at + 16],
            'offset': number(body, at + 21, 2), 'length': number(body, at + 23, 2),
            'variable_index': number(body, at + 7, 2),
            'descriptor_offset': page * PAGE + at,
        }
        seen = observed['columns'][ordinal]
        require(seen['ordinal'] == ordinal and all(
            field[key] == seen[key] for key in ['name', 'number', 'native', 'length']
        ), 'Raw/SDK original column identity')
        variable = not bool(field['flags'] & FIXED_FLAG)
        require(variable == seen['variable'], 'Raw/SDK variable declaration')
        if variable:
            require(field['variable_index'] == seen['variable_index'], 'Raw/SDK variable index')
        else:
            require(field['offset'] == seen['offset'], 'Raw/SDK fixed offset')
        fields.append(field)
    require([field['number'] for field in fields] == list(range(maximum)),
            'Dense immutable native field inventory')
    variables = number(body, 43, 2)
    require(sorted(field['variable_index'] for field in fields
                   if not field['flags'] & FIXED_FLAG) == list(range(variables)),
            'Complete dense original variable slots')
    physical = []
    for index in range(physical_count):
        at = position + 52 * index
        require(at + 52 <= length, 'Original physical descriptor extent')
        keys = []
        for key in range(10):
            native = number(body, at + 4 + key * 3, 2)
            if native != 65535:
                require(native < maximum, 'Physical key native number')
                keys.append([native, body[at + 6 + key * 3] == 1])
        require(number(body, at) == PHYSICAL_HEADER_MAGIC, 'Original physical header tag')
        physical.append({
            'number': index, 'header_magic': number(body, at), 'keys': keys,
            'map_reference': number(body, at + 34), 'root': number(body, at + 38),
            'flags': body[at + 46],
            'history': number(body, 63 + 12 * index + 4),
        })
    position += 52 * physical_count
    require(position + 28 * logical_count <= length, 'Original logical descriptors')
    aliases = []
    for ordinal in range(logical_count):
        at = position + 28 * ordinal
        require(number(body, at) == LOGICAL_HEADER_MAGIC, 'Original logical header tag')
        aliases.append({
            'number': number(body, at + LOGICAL_NUMBER_OFFSET),
            'data_number': number(body, at + LOGICAL_DATA_NUMBER_OFFSET),
            'kind': body[at + LOGICAL_KIND_OFFSET],
        })
    position += 28 * logical_count
    names = []
    for _ in range(logical_count):
        width = number(body, position, 2)
        position += 2
        require(position + width <= length, 'Original index name extent')
        names.append(body[position:position + width].decode('utf-16le'))
        position += width
    require(names == [index['name'] for index in observed['indexes']], 'Original index names')
    require(logical_count == physical_count == len(observed['indexes']),
            'Closed original one-alias-per-owner profile')
    require(len({alias['number'] for alias in aliases}) == logical_count,
            'Unique original logical index numbers')
    require(sorted(alias['data_number'] for alias in aliases) == list(range(physical_count)),
            'Complete unique logical-to-physical ownership')
    for alias, seen in zip(aliases, observed['indexes'], strict=True):
        require(0 <= alias['data_number'] < physical_count, 'Logical physical-owner extent')
        found = physical[alias['data_number']]
        require(alias['data_number'] == found['number'] == seen['physical'],
                'Raw logical data-number selects exact SDK physical owner')
        require(alias['kind'] == (1 if seen['primary'] else 0),
                'Raw/SDK logical primary kind')
        require(bool(found['flags'] & 1) == seen['unique'], 'Raw/SDK physical Unique flag')
        require(all(found[key] == seen[key] for key in ['root', 'flags', 'history']),
                'Raw/SDK original physical facts')
        require([[fields[number]['name'], direction] for number, direction in found['keys']]
                == seen['keys'], 'Raw/SDK original keys')
        mapping = original_map(raw, found['map_reference'])
        require(mapping['container_and_start'] == seen['map'], 'Raw/SDK INDEX map record')
        require(found['root'] in mapping['members'], 'Original root owned by physical index')
        found['owned'] = mapping
    pairs = []
    while number(body, position, 2) != 65535:
        require(position + 10 <= length and len(pairs) < maximum, 'Original LVAL declaration')
        pairs.append({
            'number': number(body, position, 2),
            'owned': number(body, position + 2), 'candidate': number(body, position + 6),
        })
        position += 10
    require(position + 2 == length, 'Complete original declaration terminator')
    require(number(body, 16) == observed['rows'], 'Original declared row count')
    owned = original_map(raw, number(body, 55))
    require(owned['container_and_start'] == observed['map'], 'Raw/SDK ordinary map record')
    return {
        'page': page, 'length': length, 'fields': fields, 'physical': physical,
        'logical': aliases,
        'owned': owned, 'pairs': pairs, 'rows': observed['rows'],
        'variables': number(body, 43, 2),
    }


def original_map(raw, reference):
    mapping = frames.map_record(raw, reference)
    page = reference >> 8
    require(number(raw, page * PAGE + 4) == (1 if reference == 256 else 0),
            'Original map container owner')
    locations = mapping['locations']
    require(bool(locations), 'Original represented map cells')
    storage = [page]
    if mapping['kind'] == 1:
        storage.append(number(raw, mapping['range'][0] + 1))
    return {
        'reference': reference, 'range': list(mapping['range']),
        'coverage_span': [min(locations), max(locations) + 1],
        'storage_pages': storage,
        'container_and_start': [page, mapping['range'][0] % PAGE],
        'kind': mapping['kind'], 'members': sorted(mapping['members']),
    }


def records(raw, layout, observed):
    fields = layout['fields']
    maximum = len(fields)
    fixed_end = 2 + max(
        (field['offset'] + field['length'] for field in fields
         if field['flags'] & FIXED_FLAG and field['native'] != BOOL_NATIVE),
        default=0,
    )
    result = []
    for page in layout['owned']['members']:
        require(0 < page < len(raw) // PAGE, 'Original DATA member in file')
        block = raw[page * PAGE:(page + 1) * PAGE]
        require(block[:2] == bytes([1, 1]) and number(block, 4) == layout['page'],
                'Original ordinary DATA owner')
        count = number(block, 12, 2)
        require(0 <= count <= 255, 'Original slot count bound')
        for slot in range(count):
            word = number(block, 14 + 2 * slot, 2)
            if word & 32768:
                continue
            require(word & ~8191 == 0, 'Original ordinary live slot, no forwarding interpretation')
            start, end = frames.frame(raw, page, slot)
            record = raw[start:end]
            require(number(record, 0, 2) == maximum, 'Original full-width record')
            mask = len(record) - (maximum + 7) // 8
            require(fixed_end <= mask, 'Original fixed payload framing')
            variables = layout['variables']
            offsets = []
            if variables:
                require(number(record, mask - 2, 2) == variables, 'Original variable count')
                footer = mask - 2 - 2 * (variables + 1)
                offsets = [number(record, mask - 4 - 2 * i, 2) for i in range(variables + 1)]
                require(offsets == sorted(offsets) and fixed_end <= offsets[0]
                        <= offsets[-1] <= footer, 'Original variable footer/spans')
            else:
                footer = mask
            result.append({
                'ordinal': len(result), 'reference': page << 8 | slot,
                'start': start, 'end': end, 'mask': start + mask,
                'mask_hex': record[mask:].hex(), 'fixed_end': start + fixed_end,
                'footer': start + footer, 'offsets': offsets,
            })
    require(len(result) == layout['rows'], 'Original ordinary root count')
    require([record['reference'] for record in result]
            == [row['reference'] for row in observed['records']], 'Raw/SDK original row order')
    return result


def integer(raw, record, field):
    mask = raw[record['mask'] + field['number'] // 8]
    require(mask & (1 << (field['number'] % 8)), 'Original selected non-NULL integer')
    start = record['start'] + 2 + field['offset']
    require(field['native'] == I32_NATIVE and field['length'] == 4,
            'Original selected Int32 descriptor')
    require(start + 4 <= record['fixed_end'], 'Original selected fixed span')
    return int.from_bytes(raw[start:start + 4], 'little', signed=True)


def integer_leaf(raw, layout, index, rows, field):
    page = index['root']
    body = raw[page * PAGE:(page + 1) * PAGE]
    require(body[:2] == bytes([4, 1]) and number(body, 4) == layout['page'],
            'Original literal INDEX leaf ownership')
    require(body[26] == 0 and number(body, 24, 2) == 0
            and all(number(body, at) == 0 for at in [12, 16, 20]),
            'Original closed leaf topology')
    ends = [byte * 8 + bit for byte, value in enumerate(body[27:480])
            for bit in range(8) if value & (1 << bit)]
    require(not body[27] & 1 and not body[479] & 254,
            'Original INDEX bitmap bounds')
    require(ends == [9, 18, 27] and number(body, 2, 2) == 3616 - 27,
            'Original exact three-entry leaf')
    expected = []
    for record in rows:
        value = integer(raw, record, field)
        wire = b'\x7f' + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big')
        wire += record['reference'].to_bytes(4, 'big')
        expected.append(wire)
    require(body[480:507] == b''.join(sorted(expected)), 'Original exact KEY/ROW membership')


def qualify(raw, metadata, code):
    require(raw[20] == code and len(raw) % PAGE == 0, 'Original header/extent')
    tables = metadata['tables']
    layouts = {name: definition(raw, table) for name, table in tables.items()}
    observed = tables['PreservedRows']
    layout = layouts['PreservedRows']
    require([field['name'] for field in layout['fields']]
            == ['ID', 'Score', 'Base', 'Cached', 'Flag', 'Memo', 'Blob', 'Files'],
            'Exact original rich field inventory')
    rows = records(raw, layout, observed)
    require(len(rows) == 3 and [integer(raw, row, layout['fields'][0]) for row in rows]
            == [1, 2, 3], 'Original typed row identities')
    require([integer(raw, row, layout['fields'][1]) for row in rows]
            == [1000, -2000, 3000], 'Original selected typed values')
    require([integer(raw, row, layout['fields'][2]) for row in rows]
            == [7, 11, 13], 'Original calculation inputs')
    require([row['cells'][3] for row in observed['records']]
            == ['I32:999', 'I32:-333', 'I32:0'], 'Original stored cache, no recomputation')
    for record, row in zip(rows, observed['records'], strict=True):
        require(row['cells'][:3] == [f'I32:{integer(raw, record, layout["fields"][i])}'
                                   for i in range(3)], 'Raw/SDK typed original integers')
    target = layout['fields'][1]
    require((target['native'], target['flags'], target['extended'], target['length'],
             target['offset']) == (4, 3, 0, 4, 4), 'Exact original ordinary Score declaration')
    for other in layout['fields']:
        if other['number'] == target['number'] or other['native'] == BOOL_NATIVE:
            continue
        if other['flags'] & FIXED_FLAG and other['length']:
            require(other['offset'] + other['length'] <= target['offset']
                    or target['offset'] + 4 <= other['offset'], 'No fixed sibling overlap')
    require(len(layout['physical']) == 2, 'Original two physical indexes')
    for index, native in zip(layout['physical'], [0, 2], strict=True):
        require(index['keys'] == [[native, True]], 'Score absent from original key owners')
        integer_leaf(raw, layout, index, rows, layout['fields'][native])
    properties = metadata['records']
    expression = next(row for row in properties
                      if row[:4] == ['PROPERTY', 'PreservedRows', 'Cached', 'Expression'])
    require(expression[6].startswith('T:'), 'Original calculation text carrier')
    require(base64.b64decode(expression[6][2:], validate=True).decode('utf-8') == '[Base]*2',
            'Original calculation dependency excludes Score')
    require(['PROPERTY', 'PreservedRows', '', 'PublishToWeb', '2', '0', 'U8:0']
            in properties, 'Original source policy, no property patch')
    require(['PROPERTY', 'PreservedRows', 'Score', 'Required', '1', '1', 'B1']
            in properties, 'Original selected Required rule')
    global_free = original_map(raw, 256)
    selected_pages = set(layout['owned']['members'])
    require(all(global_free['coverage_span'][0] <= page < global_free['coverage_span'][1]
                for page in selected_pages), 'Actual GLOBAL cells represent selected DATA')
    require(not selected_pages.intersection(global_free['members']), 'Live DATA is allocated')
    protected_storage = set(global_free['storage_pages'])
    for name, other in layouts.items():
        protected_storage.update(other['owned']['storage_pages'])
        if name != 'PreservedRows':
            require(not selected_pages.intersection(other['owned']['members']),
                    'No other table ordinary ownership of selected DATA')
        require(other['page'] not in selected_pages, 'Definition is not selected DATA')
        for index in other['physical']:
            protected_storage.update(index['owned']['storage_pages'])
            require(not selected_pages.intersection(index['owned']['members']),
                    'No INDEX body ownership of selected DATA')
        for pair in other['pairs']:
            owned = original_map(raw, pair['owned'])
            candidate = original_map(raw, pair['candidate'])
            protected_storage.update(owned['storage_pages'])
            protected_storage.update(candidate['storage_pages'])
            require(not selected_pages.intersection(owned['members']),
                    'No LVAL body ownership of selected DATA')
    require(not selected_pages.intersection(protected_storage),
            'Map storage cannot be selected DATA')
    return {'layout': layout, 'rows': rows, 'metadata': metadata,
            'source_sha256': hashlib.sha256(raw).hexdigest()}
