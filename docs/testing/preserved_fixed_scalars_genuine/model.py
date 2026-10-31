"""Independent original/request-only eight-byte preserved scalar expectations."""
import hashlib
import math
import struct

from preserved_int32_genuine import framing

PAGE = 4096
WIDTH = 8
FIELDS = ['ID', 'Large_Number', 'Date_Field', 'Time_Field', 'Currency_Field',
          'Base', 'Cached', 'Flag', 'Memo', 'Blob', 'Files']
TARGETS = {'Large_Number': (19, 'Int64'), 'Currency_Field': (5, 'Currency'),
           'Date_Field': (8, 'DateTime')}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def payload(value):
    """Encode reviewed exact carriers, independently of native writer code."""
    require(isinstance(value, dict), 'Exact typed request')
    if value == {'kind': 'Null'}:
        return None
    kind = value.get('kind')
    if kind in ['Int64', 'Currency']:
        key = 'value' if kind == 'Int64' else 'scaled'
        require(set(value) == {'kind', key} and type(value[key]) is int
                and -(1 << 63) <= value[key] < (1 << 63), 'Exact signed eight-byte integer')
        return value[key].to_bytes(WIDTH, 'little', signed=True)
    require(kind == 'DateTime' and set(value) == {'kind', 'bits'}
            and isinstance(value['bits'], str) and len(value['bits']) == 16
            and all(letter in '0123456789abcdef' for letter in value['bits']), 'Exact DATE bits')
    bits = int(value['bits'], 16)
    raw = bits.to_bytes(WIDTH, 'little')
    days = struct.unpack('<d', raw)[0]
    require(math.isfinite(days) and -657434 <= math.trunc(days) <= 2958465,
            'Finite DATE calendar envelope without normalization')
    return raw


def reader_token(value):
    if value == {'kind': 'Null'}:
        return 'N'
    payload(value)
    if value['kind'] == 'Int64':
        return 'I64:' + str(value['value'])
    if value['kind'] == 'Currency':
        scaled = value['scaled']
        whole, fraction = divmod(abs(scaled), 10000)
        return f'CUR:{"-" if scaled < 0 else ""}{whole}.{fraction:04d}'
    # These are reviewed literal observations, not conversion through an SDK
    # calendar object or a read of any candidate.
    tokens = {'0000000000000000': 'LDT:1899-12-30T00:00',
              '8000000000000000': 'LDT:1899-12-30T00:00',
              '3fd0000000000000': 'LDT:1899-12-30T06:00'}
    require(value['bits'] in tokens, 'Independently reviewed DATE reader token')
    return tokens[value['bits']]


def qualify(raw, getter, header):
    require(raw[20] == header and header in [5, 6] and len(raw) % PAGE == 0,
            'Complete original extent/header')
    metadata = framing.sdk_inventory(getter)
    observed = metadata['tables']['PreservedRows']
    layout = framing.definition(raw, observed)
    require([field['name'] for field in layout['fields']] == FIELDS
            and observed['maximum'] == 11, 'Actual complete eleven-field schema')
    fields = {field['name']: field for field in layout['fields']}
    rows = framing.records(raw, layout, observed)
    require(len(rows) == 3 and [framing.integer(raw, row, fields['ID']) for row in rows]
            == [1, 2, 3], 'Original ID/root/ordinal binding')
    for name, (native, _) in TARGETS.items():
        field = fields[name]
        require((field['native'], field['flags'], field['extended'], field['length'])
                == (native, 3, 0, WIDTH), 'Ordinary original target shape')
        for other in layout['fields']:
            if other['number'] != field['number'] and other['flags'] & 1 and other['length']:
                require(other['offset'] + other['length'] <= field['offset']
                        or field['offset'] + WIDTH <= other['offset'], 'Disjoint fixed target')
        require(all(all(key[0] != field['number'] for key in index['keys'])
                    for index in layout['physical']), 'No selected INDEX owner')
        rules = [row for row in metadata['records']
                 if row[:4] == ['PROPERTY', 'PreservedRows', name, 'Required']]
        require(len(rules) == 1 and rules[0][4] == '1' and rules[0][5] in ['0', '1']
                and rules[0][6] == 'B0', 'Original nullable target rule')
    require(fields['Time_Field']['native'] == 8 and fields['Time_Field']['length'] == WIDTH,
            'Actual preserved legacy-time sibling')
    base = fields['Base']
    require([framing.integer(raw, row, base) for row in rows] == [7, 11, 13], 'Original Base values')
    require(len(layout['physical']) == 2, 'Actual parent physical INDEX count')
    for index in layout['physical']:
        require(len(index['keys']) == 1 and index['keys'][0][1], 'Original ascending one-field key')
        number = index['keys'][0][0]
        require(number in [fields['ID']['number'], base['number']], 'Only ID/Base INDEX keys')
        framing.integer_leaf(raw, layout, index, rows, layout['fields'][number])
    require({index['keys'][0][0] for index in layout['physical']}
            == {fields['ID']['number'], base['number']}, 'Complete ID/Base physical owners')
    expression = [row for row in metadata['records']
                  if row[:4] == ['PROPERTY', 'PreservedRows', 'Cached', 'Expression']]
    require(len(expression) == 1 and expression[0][6] == 'T:W0Jhc2VdKjI=',
            'Original Cached dependency is exactly Base')
    cached = next(column['ordinal'] for column in observed['columns'] if column['name'] == 'Cached')
    require([row['cells'][cached] for row in observed['records']]
            == ['I32:999', 'I32:-333', 'I32:0'], 'Original stale cache inventory')
    return {'metadata': metadata, 'observed': observed, 'layout': layout,
            'fields': fields, 'rows': rows}


def construct(raw, qualified, case):
    request = case['native_request']
    require(request['table'] == 'PreservedRows' and request['column'] in TARGETS,
            'Exact selected original target')
    updates = request['updates']
    require(len(updates) == 1, 'Finite singleton original request')
    update = updates[0]
    require(update['row_id'] in [{'ID': 1}, {'ID': 2}], 'Literal selected manual ID')
    matches = [row for row in qualified['rows']
               if framing.integer(raw, row, qualified['fields']['ID']) == update['row_id']['ID']]
    require(len(matches) == 1, 'Unique raw original ID')
    row = matches[0]
    require(update['ordinal'] == row['ordinal'], 'Frozen original ordinal')
    field = qualified['fields'][request['column']]
    native, kind = TARGETS[request['column']]
    for value in [update['expected'], update['replacement']]:
        require(value == {'kind': 'Null'} or value.get('kind') == kind, 'Exact selected carrier')
    before = payload(update['expected'])
    replacement = payload(update['replacement'])
    start = row['start'] + 2 + field['offset']
    end = start + WIDTH
    mask_at = row['mask'] + field['number'] // 8
    bit = 1 << (field['number'] % 8)
    require(row['start'] + 2 <= start < end <= row['fixed_end'] <= row['footer']
            <= row['mask'] and mask_at < row['end'], 'Original fixed/footer/mask extents')
    require(bool(raw[mask_at] & bit) == (before is not None), 'Original selected presence')
    if before is not None:
        require(raw[start:end] == before, 'Exact original expected payload')
    expected = bytearray(raw)
    if replacement is None:
        expected[mask_at] &= ~bit
    else:
        expected[start:end] = replacement
        expected[mask_at] |= bit
    expected = bytes(expected)
    require(len(expected) == len(raw) and expected[:start] == raw[:start]
            and expected[end:mask_at] == raw[end:mask_at]
            and expected[mask_at + 1:] == raw[mask_at + 1:], 'Entire original complement retained')
    require((expected[mask_at] ^ raw[mask_at]) & ~bit == 0, 'Every unrelated mask bit retained')
    if replacement is None:
        require(expected[start:end] == raw[start:end], 'Inactive payload retained')
    changed = int(update['expected'] != update['replacement'])
    require(bool(expected != raw) == bool(changed), 'Representational changed-row count')
    observed = qualified['observed']
    column = next(item for item in observed['columns'] if item['name'] == request['column'])
    lines = list(qualified['metadata']['lines'])
    matched = 0
    for index, line in enumerate(lines):
        cells = line.split('\t')
        if cells[:3] != ['ROW', 'PreservedRows', str(row['ordinal'])]:
            continue
        require(cells[3] == f'{row["reference"] >> 8}:{row["reference"] & 255}'
                and cells[4] == f'I32:{update["row_id"]["ID"]}'
                and cells[4 + column['ordinal']] == reader_token(update['expected']),
                'Exact original reader identity/precondition')
        cells[4 + column['ordinal']] = reader_token(update['replacement'])
        lines[index] = '\t'.join(cells)
        matched += 1
    require(matched == 1, 'One original reader cell')
    proof = {'request': request, 'source_sha256': hashlib.sha256(raw).hexdigest(),
             'expected_sha256': hashlib.sha256(expected).hexdigest(),
             'definition_page': qualified['layout']['page'],
             'descriptor_offset': field['descriptor_offset'], 'native_type': native,
             'native_field_number': field['number'], 'fixed_offset': field['offset'],
             'source_ordinal': row['ordinal'], 'logical_root_reference': row['reference'],
             'terminal_reference': row['reference'], 'record_span': [row['start'], row['end']],
             'original_variable_offsets': row['offsets'], 'footer_offset': row['footer'],
             'mask_offset': mask_at, 'mask_bit': bit, 'payload_span': [start, end],
             'before_hex': raw[start:end].hex(), 'after_hex': expected[start:end].hex(),
             'before_mask': raw[mask_at], 'after_mask': expected[mask_at],
             'actual_difference_offsets': [at for at in [*range(start, end), mask_at]
                                           if raw[at] != expected[at]],
             'expected_outcome': {'kind': 'candidate', 'changed_rows': changed},
             'all_other_bytes_and_eof_unchanged': True, 'normal_save_or_journal_authority': False}
    return expected, proof, ('\n'.join(lines) + '\n').encode('utf-8')
