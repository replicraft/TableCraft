"""Independent fitting Text plus Boolean images from immutable original facts."""
import base64
import hashlib

from preserved_int32_genuine import framing
from preserved_short_text_genuine import framing as text_framing


def require(condition, message):
    if not condition:
        raise ValueError(message)


def text_value(value):
    if value == {'kind': 'Null'}:
        return None
    require(set(value) == {'kind', 'value'} and value['kind'] == 'Text'
            and isinstance(value['value'], str), 'Exact Text/NULL carrier')
    return value['value']


def token(value, column):
    if column == 'Flag':
        require(set(value) == {'kind', 'value'} and value['kind'] == 'Boolean'
                and type(value['value']) is bool, 'Exact Boolean carrier')
        return 'B1' if value['value'] else 'B0'
    value = text_value(value)
    return 'N' if value is None else 'T:' + base64.b64encode(value.encode('utf-8')).decode('ascii')


def qualify(raw, getter, header):
    measured = text_framing.inspect_original(raw, getter, header, framing)
    metadata = framing.sdk_inventory(getter)
    observed = metadata['tables']['PreservedRows']
    layout = framing.definition(raw, observed)
    rows = framing.records(raw, layout, observed)
    fields = {field['name']: field for field in layout['fields']}
    flag = fields['Flag']
    require((flag['native'], flag['number'], flag['flags'], flag['extended'], flag['length'])
            == (1, 4, 3, 0, 1), 'Actual ordinary Boolean declaration; physical payload width zero')
    require(not observed['columns'][4]['calculated'] and not observed['columns'][4]['auto'],
            'Actual Flag is not calculated or generated')
    require(all(flag['number'] != key[0] for index in layout['physical'] for key in index['keys']),
            'Flag is absent from every original physical index')
    # inspect_original already independently binds the sole expression [Base]*2.
    # It never permits a selected calculated input or grants absent-event authority.
    for row, sdk, value in zip(rows, observed['records'], [False, True, False], strict=True):
        mask_at = row['mask'] + flag['number'] // 8
        require(row['mask'] <= mask_at < row['end'], 'Original Flag mask extent')
        require(bool(raw[mask_at] & (1 << (flag['number'] % 8))) == value,
                'Original raw Boolean value')
        require(sdk['cells'][flag['number']] == ('B1' if value else 'B0'),
                'Original raw/getter Flag agreement')
    return {'metadata': metadata, 'layout': layout, 'rows': rows,
            'measured': measured, 'getter': getter, 'header': header,
            'source_sha256': hashlib.sha256(raw).hexdigest()}


def literal_payload(value, capacity):
    if value is None:
        return b''
    raw = value.encode('utf-16le')
    compact = None
    if all(0 < ord(character) <= 255 for character in value):
        compact = b'\xff\xfe' + value.encode('latin-1')
    payload = compact if compact is not None and len(compact) <= capacity else raw
    require(len(payload) <= capacity, 'Literal selected value fits original physical extent')
    return payload


def construct(raw, original, case):
    require(hashlib.sha256(raw).hexdigest() == original['source_sha256'], 'Exact original binding')
    require(case['header'] == original['header'] == raw[20], 'Exact source header')
    request = case['native_request']
    require(request['table'] == 'PreservedRows', 'Exact original table')
    groups = request['groups']
    require(len(groups) == 2 and {group['column'] for group in groups} == {'Short_Text', 'Flag'},
            'One explicit Text and one Boolean group, including empty groups')
    layout = original['layout']
    rows = original['rows']
    observed = original['metadata']['tables']['PreservedRows']
    fields = {field['name']: field for field in layout['fields']}
    result = bytearray(raw)
    changed = set()
    spans = []
    reader_edits = []
    after_lines = original['getter'].splitlines()
    row_lines = {
        int(parts[2]): index
        for index, line in enumerate(after_lines)
        if (parts := line.split('\t'))[:2] == ['ROW', 'PreservedRows']
    }
    require(set(row_lines) == {0, 1, 2}, 'Exact complete original reader rows')
    requests = {}
    for group in groups:
        name = group['column']
        field = fields[name]
        seen = set()
        for update in group['updates']:
            identifier = update['row_id']['ID']
            matching = [row for row in rows
                        if framing.integer(raw, row, fields['ID']) == identifier]
            require(len(matching) == 1, 'One actual original ID')
            row = matching[0]
            ordinal = row['ordinal']
            require(update['ordinal'] == ordinal and ordinal not in seen,
                    'Exact source ordinal and unique per-group assignment')
            seen.add(ordinal)
            requests[(name, ordinal)] = update
            before_token = token(update['expected'], name)
            after_token = token(update['replacement'], name)
            require(observed['records'][ordinal]['cells'][field['number']] == before_token,
                    'Whole original SDK precondition')
            parts = after_lines[row_lines[ordinal]].split('\t')
            require(parts[3] == f'{row["reference"] >> 8}:{row["reference"] & 255}'
                    and parts[4 + field['number']] == before_token,
                    'Exact original logical root reader identity')
            parts[4 + field['number']] = after_token
            after_lines[row_lines[ordinal]] = '\t'.join(parts)
            reader_edits.append({'column': name, 'ordinal': ordinal,
                                 'before_token': before_token, 'after_token': after_token})

    # Rebuild only actual Text changes from ORIGINAL spans. Fixed values are
    # overlaid afterward regardless of caller group or assignment ordering.
    for row in rows:
        ordinal = row['ordinal']
        text = requests.get(('Short_Text', ordinal))
        field = fields['Short_Text']
        variable = field['variable_index']
        offsets = list(row['offsets'])
        final_offsets = list(offsets)
        start = row['start'] + offsets[variable]
        stop = row['start'] + offsets[variable + 1]
        logical_end = row['start'] + offsets[-1]
        text_mask = row['mask'] + field['number'] // 8
        text_bit = 1 << (field['number'] % 8)
        flag = requests.get(('Flag', ordinal))
        flag_field = fields['Flag']
        flag_mask = row['mask'] + flag_field['number'] // 8
        flag_bit = 1 << (flag_field['number'] % 8)
        if text is not None:
            before = text_value(text['expected'])
            value = text_value(text['replacement'])
            require(bool(raw[text_mask] & text_bit) == (before is not None),
                    'Original selected Text presence')
            if before is None:
                require(start == stop, 'Original NULL has the measured zero-byte slot')
            else:
                require(raw[start:stop] == b'\xff\xfe' + before.encode('latin-1'),
                        'Original compact text bytes match immutable request')
            capacity = stop - start + row['footer'] - logical_end
            payload = literal_payload(value, capacity)
            require(case['literal_text_payloads'][str(text['row_id']['ID'])] == payload.hex(),
                    'Frozen independently chosen literal wire payload')
            if before != value:
                final_end = start + len(payload) + logical_end - stop
                require(final_end <= row['footer'], 'Same-length fitting physical record')
                result[start:start + len(payload)] = payload
                result[start + len(payload):final_end] = raw[stop:logical_end]
                for position, offset in enumerate(offsets):
                    if position > variable:
                        final_offsets[position] = offsets[variable] + len(payload) + offset - offsets[variable + 1]
                    at = row['mask'] - 4 - position * 2
                    result[at:at + 2] = final_offsets[position].to_bytes(2, 'little')
                result[text_mask] = (result[text_mask] & ~text_bit) | (text_bit if value is not None else 0)
                changed.add(row['reference'])
        if flag is not None:
            before = flag['expected']['value']
            value = flag['replacement']['value']
            require(bool(raw[flag_mask] & flag_bit) == before, 'Actual original Flag value')
            if before != value:
                result[flag_mask] = (result[flag_mask] & ~flag_bit) | (flag_bit if value else 0)
                changed.add(row['reference'])

        # Complete retained fixed prefix, raw sibling payloads, padding and mask
        # complements. No rich sibling is decoded or fabricated as scalar Value.
        require(result[row['start']:start] == raw[row['start']:start], 'Every fixed-prefix byte exact')
        for index in range(len(offsets) - 1):
            if index != variable:
                old = slice(row['start'] + offsets[index], row['start'] + offsets[index + 1])
                new = slice(row['start'] + final_offsets[index], row['start'] + final_offsets[index + 1])
                require(result[new] == raw[old], 'Every opaque variable sibling raw-exact')
        final_end = row['start'] + final_offsets[-1]
        require(result[final_end:row['footer']] == raw[final_end:row['footer']],
                'All unused tail padding at final positions exact')
        for at in range(row['mask'], row['end']):
            selected = (text_bit if at == text_mask and text is not None else 0)
            selected |= flag_bit if at == flag_mask and flag is not None else 0
            require(result[at] & ~selected == raw[at] & ~selected, 'Every unselected mask bit exact')
        require(result[row['mask'] - 2:row['mask']] == raw[row['mask'] - 2:row['mask']],
                'Variable count exact')
        if text is None or text['expected'] == text['replacement']:
            require(final_offsets == offsets and result[start:stop] == raw[start:stop],
                    'Text/NULL no-op raw representation and footer exact')
        if text is None and flag is None:
            require(result[row['start']:row['end']] == raw[row['start']:row['end']],
                    'Every wholly unrequested record exact')
        spans.append([row['start'], row['end']])

    expected = bytes(result)
    require(len(expected) == len(raw), 'Whole original image length unchanged')
    cursor = 0
    for start, stop in sorted(spans):
        require(expected[cursor:start] == raw[cursor:start], 'Every non-record byte exact')
        cursor = stop
    require(expected[cursor:] == raw[cursor:], 'Complete trailing image and EOF exact')
    require(len(changed) == case['expected_changed_rows'], 'Union of changed original roots')
    if not changed:
        require(expected == raw, 'Complete all-noop and empty-operation image exact')
    reader = ('\n'.join(after_lines) + '\n').encode('utf-8')
    proof = {
        'scope': 'original/request-only joint fitting Text and Boolean complete image',
        'source_sha256': original['source_sha256'],
        'expected_sha256': hashlib.sha256(expected).hexdigest(),
        'header': case['header'], 'request': request,
        'definition_page': layout['page'],
        'fields': {name: fields[name] for name in ['Short_Text', 'Flag']},
        'physical_boolean_payload_width': 0,
        'records': original['measured']['rows'],
        'changed_root_references': sorted(changed),
        'changed_rows': len(changed), 'reader_edits': reader_edits,
        'reader_sha256': hashlib.sha256(reader).hexdigest(),
        'normal_save_or_journal_authority': False,
    }
    return expected, proof, reader
