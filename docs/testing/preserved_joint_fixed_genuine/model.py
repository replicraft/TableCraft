"""Original-only joint fixed images; Boolean is a mask value, not a payload."""
import hashlib
import math
import struct

from preserved_fixed_scalars_genuine import model as original_model

NUMERIC_WIDTH = 8
SELECTED = {
    'Large_Number': (19, 'Int64'),
    'Currency_Field': (5, 'Currency'),
    'Date_Field': (8, 'DateTime'),
    'Time_Field': (8, 'DateTime'),
    'Flag': (1, 'Boolean'),
}
DATE_TOKENS = {
    '0000000000000000': 'LDT:1899-12-30T00:00',
    '8000000000000000': 'LDT:1899-12-30T00:00',
    '3fd0000000000000': 'LDT:1899-12-30T06:00',
    '3fe0000000000000': 'LDT:1899-12-30T12:00',
    # Bound to the original bytes 00 00 00 00 e0 24 e6 40, not derived from LDT.
    '40e624e000000000': 'LDT:2024-02-29T00:00',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def encoded(value, kind):
    require(isinstance(value, dict), 'Exact typed original request')
    if value == {'kind': 'Null'}:
        require(kind != 'Boolean', 'Boolean false is not NULL')
        return None
    require(value.get('kind') == kind, 'Exact native carrier without coercion')
    if kind == 'Boolean':
        require(set(value) == {'kind', 'value'} and type(value['value']) is bool,
                'Closed Boolean carrier')
        return b''
    if kind in ['Int64', 'Currency']:
        key = 'value' if kind == 'Int64' else 'scaled'
        require(set(value) == {'kind', key} and type(value[key]) is int
                and -(1 << 63) <= value[key] < (1 << 63), 'Exact signed payload')
        return value[key].to_bytes(NUMERIC_WIDTH, 'little', signed=True)
    require(kind == 'DateTime' and set(value) == {'kind', 'bits'}
            and isinstance(value['bits'], str) and len(value['bits']) == 16
            and all(letter in '0123456789abcdef' for letter in value['bits']),
            'Exact unnormalized DATE bits')
    payload = int(value['bits'], 16).to_bytes(NUMERIC_WIDTH, 'little')
    days = struct.unpack('<d', payload)[0]
    require(math.isfinite(days) and -657434 <= math.trunc(days) <= 2958465,
            'Finite original DATE envelope')
    return payload


def reader_token(value, kind):
    encoded(value, kind)
    if value == {'kind': 'Null'}:
        return 'N'
    if kind == 'Boolean':
        return 'B1' if value['value'] else 'B0'
    if kind == 'Int64':
        return 'I64:' + str(value['value'])
    if kind == 'Currency':
        scaled = value['scaled']
        whole, fraction = divmod(abs(scaled), 10000)
        return f'CUR:{"-" if scaled < 0 else ""}{whole}.{fraction:04d}'
    require(value['bits'] in DATE_TOKENS, 'Separately reviewed DATE reader token')
    return DATE_TOKENS[value['bits']]


def locate(row, field, kind):
    mask = row['mask'] + field['number'] // 8
    bit = 1 << (field['number'] % 8)
    require(row['mask'] <= mask < row['end'], 'Actual selected mask extent')
    if kind == 'Boolean':
        return None, mask, bit
    start = row['start'] + 2 + field['offset']
    end = start + NUMERIC_WIDTH
    require(row['start'] + 2 <= start < end <= row['fixed_end']
            <= row['footer'] <= row['mask'], 'Actual numeric fixed/footer extents')
    return [start, end], mask, bit


def observe(raw, row, field, kind):
    span, mask, bit = locate(row, field, kind)
    if kind == 'Boolean':
        return {'kind': 'Boolean', 'value': bool(raw[mask] & bit)}
    if not raw[mask] & bit:
        return {'kind': 'Null'}
    payload = raw[span[0]:span[1]]
    if kind == 'Int64':
        return {'kind': kind, 'value': int.from_bytes(payload, 'little', signed=True)}
    if kind == 'Currency':
        return {'kind': kind, 'scaled': int.from_bytes(payload, 'little', signed=True)}
    return {'kind': kind, 'bits': f'{int.from_bytes(payload, "little"):016x}'}


def qualify(raw, getter, header):
    # The maintained predecessor contributes immutable original framing and
    # original ID/Base/cached/property facts only. Its constructor is never used.
    original = original_model.qualify(raw, getter, header)
    fields = original['fields']
    for name, (native, kind) in SELECTED.items():
        field = fields[name]
        declared_width = 1 if kind == 'Boolean' else NUMERIC_WIDTH
        require((field['native'], field['flags'], field['extended'], field['length'])
                == (native, 3, 0, declared_width), 'Selected original native declaration')
        require(all(all(key[0] != field['number'] for key in index['keys'])
                    for index in original['layout']['physical']), 'No selected key owner')
        if kind != 'Boolean':
            for other in original['layout']['fields']:
                if other['number'] == field['number'] or other['native'] == 1:
                    continue
                if other['flags'] & 1 and other['length']:
                    require(other['offset'] + other['length'] <= field['offset']
                            or field['offset'] + NUMERIC_WIDTH <= other['offset'],
                            'No numeric fixed overlap')
        if name == 'Time_Field':
            rules = [item for item in original['metadata']['records']
                     if item[:4] == ['PROPERTY', 'PreservedRows', name, 'Required']]
            require(len(rules) == 1 and rules[0][4] == '1' and rules[0][5] in ['0', '1']
                    and rules[0][6] == 'B0',
                    'Actual Time_Field nullable rule')
    original['columns'] = {
        column['name']: column for column in original['observed']['columns']
    }
    original['original_lines'] = {
        int(parts[2]): parts
        for line in original['metadata']['lines']
        if (parts := line.split('\t'))[:2] == ['ROW', 'PreservedRows']
    }
    require(set(original['original_lines']) == {0, 1, 2}, 'Complete original row observations')
    for row, flag in zip(original['rows'], [False, True, False], strict=True):
        value = observe(raw, row, fields['Flag'], 'Boolean')
        require(value == {'kind': 'Boolean', 'value': flag}, 'Original Flag bits false/true/false')
        ordinal = original['columns']['Flag']['ordinal']
        require(original['original_lines'][row['ordinal']][4 + ordinal]
                == reader_token(value, 'Boolean'), 'Actual raw/SDK Boolean binding')
    return original


def construct(raw, original, case):
    request = case['native_request']
    require(set(request) == {'table', 'groups'} and request['table'] == 'PreservedRows',
            'Closed joint original table request')
    groups = request['groups']
    require(len(groups) == 5 and {group['column'] for group in groups} == set(SELECTED),
            'All five exact distinct original groups')
    expected = bytearray(raw)
    allowed_payloads = set()
    allowed_masks = {}
    changed_roots = set()
    bindings = []
    reader_edits = []
    after_lines = list(original['metadata']['lines'])
    line_positions = {
        int(parts[2]): index
        for index, line in enumerate(after_lines)
        if (parts := line.split('\t'))[:2] == ['ROW', 'PreservedRows']
    }
    for group in groups:
        require(set(group) == {'column', 'updates'}, 'Closed original field group')
        name = group['column']
        field = original['fields'][name]
        native, kind = SELECTED[name]
        seen = set()
        for update in group['updates']:
            require(set(update) == {'ordinal', 'row_id', 'expected', 'replacement'}
                    and type(update['ordinal']) is int and update['ordinal'] not in seen,
                    'Distinct closed original row request')
            seen.add(update['ordinal'])
            require(update['row_id'] in [{'ID': 1}, {'ID': 2}, {'ID': 3}], 'Literal original manual ID')
            matches = [row for row in original['rows']
                       if original_model.framing.integer(raw, row, original['fields']['ID'])
                       == update['row_id']['ID']]
            require(len(matches) == 1, 'Unique actual original ID')
            row = matches[0]
            require(row['ordinal'] == update['ordinal'], 'Actual original ordinal')
            encoded(update['expected'], kind)
            replacement = encoded(update['replacement'], kind)
            require(observe(raw, row, field, kind) == update['expected'],
                    'Exact original carrier/presence/bits precondition')
            span, mask, bit = locate(row, field, kind)
            allowed_masks[mask] = allowed_masks.get(mask, 0) | bit
            if span is not None:
                allowed_payloads.update(range(span[0], span[1]))
            if kind == 'Boolean':
                if update['replacement']['value']:
                    expected[mask] |= bit
                else:
                    expected[mask] &= ~bit
            elif replacement is None:
                expected[mask] &= ~bit
            else:
                expected[span[0]:span[1]] = replacement
                expected[mask] |= bit
            if update['expected'] != update['replacement']:
                changed_roots.add(row['reference'])
            column = original['columns'][name]
            before_cells = original['original_lines'][row['ordinal']]
            before_token = reader_token(update['expected'], kind)
            after_token = reader_token(update['replacement'], kind)
            require(before_cells[3] == f'{row["reference"] >> 8}:{row["reference"] & 255}'
                    and before_cells[4 + original['columns']['ID']['ordinal']]
                    == f'I32:{update["row_id"]["ID"]}'
                    and before_cells[4 + column['ordinal']] == before_token,
                    'Actual complete original reader identity/precondition')
            position = line_positions[row['ordinal']]
            cells = after_lines[position].split('\t')
            cells[4 + column['ordinal']] = after_token
            after_lines[position] = '\t'.join(cells)
            reader_edits.append({'column': name, 'ordinal': row['ordinal'],
                                 'before_token': before_token, 'after_token': after_token})
            bindings.append({
                'column': name, 'ordinal': row['ordinal'], 'row_id': update['row_id'],
                'logical_root_reference': row['reference'], 'terminal_reference': row['reference'],
                'definition_page': original['layout']['page'],
                'descriptor_offset': field['descriptor_offset'], 'native_type': native,
                'native_field_number': field['number'], 'declared_storage_length': field['length'],
                'physical_payload_width': 0 if kind == 'Boolean' else NUMERIC_WIDTH,
                'record_span': [row['start'], row['end']], 'payload_span': span,
                'mask_offset': mask, 'mask_bit': bit, 'before_mask': raw[mask],
                'before_payload_hex': None if span is None else raw[span[0]:span[1]].hex(),
                'original_variable_offsets': row['offsets'], 'original_footer': row['footer'],
                'expected': update['expected'], 'replacement': update['replacement'],
            })
    expected = bytes(expected)
    require(len(expected) == len(raw), 'Exact original image length/EOF')
    allowed = sorted(allowed_payloads | set(allowed_masks))
    previous = 0
    differences = []
    for offset in allowed:
        require(raw[previous:offset] == expected[previous:offset], 'Complete other-byte complement')
        if raw[offset] != expected[offset]:
            differences.append(offset)
        previous = offset + 1
    require(raw[previous:] == expected[previous:], 'Complete final image complement')
    for mask, bits in allowed_masks.items():
        require((raw[mask] ^ expected[mask]) & ~bits == 0, 'All unrelated mask bits retained')
    for binding in bindings:
        span = binding['payload_span']
        binding['after_mask'] = expected[binding['mask_offset']]
        binding['after_payload_hex'] = None if span is None else expected[span[0]:span[1]].hex()
        if span is not None and (binding['replacement'] == {'kind': 'Null'}
                                 or binding['expected'] == binding['replacement']):
            require(expected[span[0]:span[1]] == raw[span[0]:span[1]], 'Inactive/no-op raw payload retained')
    count = len(changed_roots)
    require(bool(differences) == bool(count), 'Exact representational change union')
    require(count == case['expected_changed_rows'], 'Literal expected root count')
    if count == 0:
        require(expected == raw, 'Exact whole-image no-op')
    proof = {
        'request': request, 'source_sha256': hashlib.sha256(raw).hexdigest(),
        'expected_sha256': hashlib.sha256(expected).hexdigest(), 'records': bindings,
        'changed_root_references': sorted(changed_roots), 'reader_edits': reader_edits,
        'actual_difference_offsets': differences,
        'expected_outcome': {'kind': 'candidate', 'changed_rows': count},
        'all_other_bytes_and_eof_unchanged': True, 'normal_save_or_journal_authority': False,
    }
    return expected, proof, ('\n'.join(after_lines) + '\n').encode('utf-8')
