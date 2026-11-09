"""Independent complete expectations from qualified original bytes and requests."""
import hashlib

from .framing import integer, require


def encoded(value):
    require(type(value) is int and -(1 << 31) <= value < (1 << 31),
            'Literal signed Int32 request')
    return value.to_bytes(4, 'little', signed=True)


def construct(raw, qualified, request):
    layout = qualified['layout']
    require(request['table'] == 'PreservedRows' and request['column'] == 'Score',
            'Exact selected original table/column')
    require(request['row_id'] == {'ID': 1}
            and request['expected'] == {'kind': 'Int32', 'value': 1000},
            'Exact original typed request')
    replacement = request['replacement']
    require(replacement['kind'] == 'Int32', 'Replacement carrier')
    payload = encoded(replacement['value'])
    selected = [row for row in qualified['rows']
                if integer(raw, row, layout['fields'][0]) == request['row_id']['ID']]
    require(len(selected) == 1, 'Unique actual source identity')
    row = selected[0]
    require(request['ordinal'] == row['ordinal'], 'Frozen actual source ordinal')
    field = layout['fields'][1]
    require(integer(raw, row, field) == request['expected']['value'],
            'Original request precondition')
    start = row['start'] + 2 + field['offset']
    end = start + len(payload)
    require(row['start'] + 2 <= start < end <= row['fixed_end'] <= row['mask'],
            'Selected payload independent framing')
    before = raw[start:end]
    require(before == encoded(request['expected']['value']), 'Original exact payload bytes')

    expected = bytearray(raw)
    expected[start:end] = payload
    expected = bytes(expected)
    require(len(expected) == len(raw), 'Expected original length/EOF')
    require(expected[:start] == raw[:start] and expected[end:] == raw[end:],
            'Every byte outside the exact payload remains original')
    require(expected[start:end] == payload, 'Literal replacement payload')
    differences = [offset for offset in range(start, end)
                   if raw[offset] != expected[offset]]
    if replacement['value'] == request['expected']['value']:
        require(expected == raw and not differences, 'Whole-image exact no-op')

    resolved = dict(request)
    resolved['ordinal'] = row['ordinal']
    proof = {
        'scope': 'original/request-only four-byte engineering expectation',
        'source_sha256': qualified['source_sha256'],
        'definition_page': layout['page'],
        'descriptor_offset': field['descriptor_offset'],
        'native_field_number': field['number'],
        'fixed_offset': field['offset'],
        'source_ordinal': row['ordinal'],
        'logical_root_reference': row['reference'],
        'terminal_reference': row['reference'],
        'record_span': [row['start'], row['end']],
        'mask_offset': row['mask'],
        'mask_bytes': row['mask_hex'],
        'selected_presence_bit': 1 << (field['number'] % 8),
        'original_variable_offsets': row['offsets'],
        'original_footer_offset': row['footer'],
        'allowed_payload_span': [start, end],
        'before_hex': before.hex(),
        'after_hex': payload.hex(),
        'actual_difference_offsets': differences,
        'protected_spans': [[0, start], [end, len(raw)]],
        'protected_bytes': len(raw) - len(payload),
        'length_eof_unchanged': True,
        'metadata_indexes_maps_caches_lval_children_unchanged': True,
        'request': resolved,
        'expected_sha256': hashlib.sha256(expected).hexdigest(),
        'normal_save_or_journal_authority': False,
    }
    # Record a reproducible original-reader projection, not an UPDATE oracle.
    lines = list(qualified['metadata']['lines'])
    matched = 0
    for index, line in enumerate(lines):
        parts = line.split('\t')
        if parts[:3] != ['ROW', 'PreservedRows', str(row['ordinal'])]:
            continue
        require(parts[4] == 'I32:1' and parts[5] == 'I32:1000',
                'Original SDK selected row identity/precondition')
        parts[5] = f'I32:{replacement["value"]}'
        lines[index] = '\t'.join(parts)
        matched += 1
    require(matched == 1, 'Exactly one expected typed SDK row change')
    reader = ('\n'.join(lines) + '\n').encode('utf-8')
    return expected, proof, resolved, reader

