"""Independent whole-image expectations from immutable originals and literal requests."""
import base64
import hashlib

from preserved_int32_genuine import framing


def require(condition, message):
    if not condition:
        raise ValueError(message)


def cell(value):
    if value == {'kind': 'Null'}:
        return None
    require(set(value) == {'kind', 'value'} and value['kind'] == 'Text'
            and isinstance(value['value'], str), 'Literal Text/NULL request')
    return value['value']


def choose_literal(request, capacity, payload_hex, outcome):
    value = cell(request['replacement'])
    literal = bytes.fromhex(payload_hex)
    if value is None:
        require(literal == b'', 'NULL removes the selected variable payload')
        return literal
    raw = value.encode('utf-16le')
    compact = None
    if all(0 < ord(character) <= 255 for character in value):
        compact = b'\xff\xfe' + value.encode('latin-1')
    if outcome == 'refusal':
        require(literal == raw and len(raw) > capacity
                and (compact is None or len(compact) > capacity),
                'Neither independent legal form fits original capacity')
    else:
        chosen = compact if compact is not None and len(compact) <= capacity else raw
        require(literal == chosen and len(literal) <= capacity,
                'Literal declared-preference/raw-fallback form fits original capacity')
    return literal


def construct(raw, getter, header, case):
    request = case['native_request']
    outcome = case['outcome']
    metadata = framing.sdk_inventory(getter)
    observed = metadata['tables']['PreservedRows']
    layout = framing.definition(raw, observed)
    rows = framing.records(raw, layout, observed)
    require(raw[20] == header, 'Exact source header')
    require(request['table'] == 'PreservedRows' and request['column'] == 'Short_Text',
            'Exact original target')
    field = layout['fields'][1]
    require((field['name'], field['native'], field['number'], field['flags'],
             field['extended'], field['length']) == ('Short_Text', 10, 1, 2, 1, 80),
            'Original ordinary TEXT(40), compression preferred')
    selected = [row for row in rows
                if framing.integer(raw, row, layout['fields'][0]) == request['row_id']['ID']]
    require(len(selected) == 1, 'One original typed ID')
    row = selected[0]
    require(row['ordinal'] == request['ordinal'], 'Original source ordinal')
    variable = field['variable_index']
    offsets = row['offsets']
    start = row['start'] + offsets[variable]
    stop = row['start'] + offsets[variable + 1]
    logical_end = row['start'] + offsets[-1]
    capacity = stop - start + row['footer'] - logical_end
    require(capacity == (7 if request['row_id']['ID'] == 1 else 0),
            'Previously measured unmodified original capacity')
    before_value = cell(request['expected'])
    replacement = cell(request['replacement'])
    bit = 1 << (field['number'] % 8)
    mask_at = row['mask'] + field['number'] // 8
    require(bool(raw[mask_at] & bit) == (before_value is not None),
            'Original presence matches the request')
    sdk_cell = observed['records'][row['ordinal']]['cells'][1]
    expected_sdk = 'N' if before_value is None else (
        'T:' + base64.b64encode(before_value.encode('utf-8')).decode('ascii'))
    require(sdk_cell == expected_sdk, 'Original typed request precondition')
    if before_value is not None:
        require(raw[start:stop] == b'\xff\xfe' + before_value.encode('latin-1'),
                'Measured original compact text wire')
    else:
        require(start == stop, 'Measured original zero-byte NULL span')
    literal = choose_literal(request, capacity, case['payload_hex'], outcome)
    proof = {
        'scope': 'original/request-only complete fitting preserved Short Text image',
        'source_sha256': hashlib.sha256(raw).hexdigest(),
        'header': header,
        'definition_page': layout['page'],
        'descriptor_offset': field['descriptor_offset'],
        'native_field_number': field['number'],
        'variable_index': variable,
        'source_ordinal': row['ordinal'],
        'logical_root_reference': row['reference'],
        'terminal_reference': row['reference'],
        'record_span': [row['start'], row['end']],
        'mask_offset': mask_at,
        'selected_presence_bit': bit,
        'original_mask_hex': row['mask_hex'],
        'original_variable_offsets': offsets,
        'data_end': row['footer'],
        'original_logical_payload_end': logical_end,
        'original_selected_payload_span': [start, stop],
        'original_selected_payload_hex': raw[start:stop].hex(),
        'fitting_capacity': capacity,
        'literal_replacement_payload_hex': literal.hex(),
        'request': request,
        'normal_save_or_journal_authority': False,
    }
    if outcome == 'refusal':
        proof['expected_outcome'] = {'kind': 'refusal', 'write_blocker': 'RowLayout'}
        return None, proof, None

    noop = before_value == replacement
    final_offsets = list(offsets)
    if noop:
        require(literal == raw[start:stop], 'No-op retains actual original wire')
        expected = raw
    else:
        result = bytearray(raw)
        replacement_end = start + len(literal)
        final_end = replacement_end + logical_end - stop
        require(final_end <= row['footer'], 'Final payload fits original physical record')
        result[start:replacement_end] = literal
        result[replacement_end:final_end] = raw[stop:logical_end]
        for position, offset in enumerate(offsets):
            if position > variable:
                final_offsets[position] = offsets[variable] + len(literal) + offset - offsets[variable + 1]
            at = row['mask'] - 4 - position * 2
            result[at:at + 2] = final_offsets[position].to_bytes(2, 'little')
        result[mask_at] = (raw[mask_at] & ~bit) | (bit if replacement is not None else 0)
        expected = bytes(result)

    require(len(expected) == len(raw), 'Complete physical length and EOF unchanged')
    require(expected[:row['start']] == raw[:row['start']]
            and expected[row['end']:] == raw[row['end']:], 'All other records/pages exact')
    require(expected[row['start']:start] == raw[row['start']:start], 'Unselected fixed prefix exact')
    require(expected[mask_at] & ~bit == raw[mask_at] & ~bit, 'Every other mask bit exact')
    require(expected[row['mask'] - 2:row['mask']] == raw[row['mask'] - 2:row['mask']],
            'Variable count unchanged')
    require(expected[row['mask'] + 1:row['end']] == raw[row['mask'] + 1:row['end']],
            'Any later mask bytes exact')
    retained = []
    for index in range(len(offsets) - 1):
        if index == variable:
            continue
        original_start = row['start'] + offsets[index]
        original_stop = row['start'] + offsets[index + 1]
        final_start = row['start'] + final_offsets[index]
        final_stop = row['start'] + final_offsets[index + 1]
        require(expected[final_start:final_stop] == raw[original_start:original_stop],
                'Unselected raw rich payload remains exact after relocation')
        retained.append({
            'variable_index': index,
            'original_span': [original_start, original_stop],
            'final_span': [final_start, final_stop],
            'sha256': hashlib.sha256(raw[original_start:original_stop]).hexdigest(),
        })
    final_rows = framing.records(expected, layout, observed)
    final = final_rows[row['ordinal']]
    require(final['start'] == row['start'] and final['end'] == row['end']
            and final['offsets'] == final_offsets, 'Independently reread final raw framing')
    require(bool(expected[mask_at] & bit) == (replacement is not None), 'Final selected presence')
    final_start = final['start'] + final_offsets[variable]
    final_stop = final['start'] + final_offsets[variable + 1]
    require(expected[final_start:final_stop] == literal, 'Exact final selected payload')
    if noop:
        require(expected == raw, 'Whole-image raw no-op')
    differences = [offset for offset, (before, after) in enumerate(zip(raw, expected, strict=True))
                   if before != after]
    proof.update({
        'expected_outcome': {'kind': 'candidate', 'changed_rows': 0 if noop else 1},
        'final_variable_offsets': final_offsets,
        'final_selected_payload_span': [final_start, final_stop],
        'final_mask_hex': expected[row['mask']:row['end']].hex(),
        'retained_raw_payloads': retained,
        'protected_outer_spans': [[0, row['start']], [row['end'], len(raw)]],
        'actual_difference_offsets': differences,
        'expected_sha256': hashlib.sha256(expected).hexdigest(),
        'length_eof_unchanged': True,
        'indexes_maps_caches_lval_children_unchanged': True,
    })
    lines = list(metadata['lines'])
    matches = 0
    for index, line in enumerate(lines):
        parts = line.split('\t')
        if parts[:3] != ['ROW', 'PreservedRows', str(row['ordinal'])]:
            continue
        require(parts[4] == f'I32:{request["row_id"]["ID"]}' and parts[5] == expected_sdk,
                'Original readonly reader request identity')
        parts[5] = 'N' if replacement is None else (
            'T:' + base64.b64encode(replacement.encode('utf-8')).decode('ascii'))
        lines[index] = '\t'.join(parts)
        matches += 1
    require(matches == 1, 'Exactly one original reader row projection')
    return expected, proof, ('\n'.join(lines) + '\n').encode('utf-8')
