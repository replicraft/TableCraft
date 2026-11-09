"""Independent same-page forwarding images from immutable original records."""
import base64
import hashlib

from preserved_int32_genuine import framing

PAGE = 4096


def require(condition, message):
    if not condition:
        raise ValueError(message)


def put16(data, at, value):
    data[at:at + 2] = value.to_bytes(2, 'little')


def token(value):
    if value == {'kind': 'Null'}:
        return 'N'
    require(set(value) == {'kind', 'value'} and value['kind'] == 'Text', 'Text carrier')
    return 'T:' + base64.b64encode(value['value'].encode('utf-8')).decode('ascii')


def rebuild(raw, row, update):
    old = raw[row['start']:row['end']]
    offsets = row['offsets']
    require(len(offsets) == 5 and offsets[0] == 14, 'Original four-variable record')
    require(row['footer'] == row['start'] + offsets[-1], 'No original hidden slack')
    require(offsets[2] - offsets[1] == 27, 'Actual cached capsule width')
    replacement = update['replacement']['value']
    require(update['replacement']['kind'] == 'Text', 'Present literal replacement')
    payload = (bytes.fromhex('ea96' * 6) if replacement == '雪雪雪雪雪雪'
               else bytes.fromhex('fffe62657461'))
    require(replacement in ['雪雪雪雪雪雪', 'beta'], 'Frozen literal replacement domain')
    require(payload == (replacement.encode('utf-16le') if replacement != 'beta'
                        else b'\xff\xfe' + replacement.encode('ascii')), 'Literal wire')
    result = bytearray(old[:14] + payload + old[offsets[1]:offsets[-1]])
    final_offsets = [14] + [14 + len(payload) + offset - offsets[1] for offset in offsets[1:]]
    footer = old[offsets[-1]:]
    require(len(footer) == 13, 'Original footer and one-byte mask')
    result.extend(footer)
    mask = len(result) - 1
    for index, offset in enumerate(final_offsets):
        put16(result, mask - 4 - index * 2, offset)
    result[mask] |= 2
    require(framing.number(result, mask - 2, 2) == 4, 'Retained variable count')
    require(result[:14] == old[:14] and result[mask] & ~2 == old[-1] & ~2,
            'Fixed prefix and every other mask bit retained')
    for index in range(1, 4):
        require(result[final_offsets[index]:final_offsets[index + 1]]
                == old[offsets[index]:offsets[index + 1]], 'Exact opaque sibling payload')
    require(len(result) == ({0: 126, 1: 78}[update['ordinal']]
                           if replacement != 'beta' else 72), 'Literal minimum record length')
    return bytes(result), final_offsets


def construct(raw, getter, case, measured):
    metadata = framing.sdk_inventory(getter)
    observed = metadata['tables']['PreservedRows']
    layout = framing.definition(raw, observed)
    rows = framing.records(raw, layout, observed)
    header = case['header']
    request = case['native_request']
    require(raw[20] == header and request['table'] == 'PreservedRows'
            and request['column'] == 'Short_Text', 'Exact profile and target')
    field = layout['fields'][1]
    require((field['native'], field['number'], field['flags'], field['extended'],
             field['length'], field['variable_index']) == (10, 1, 2, 1, 80, 0),
            'Exact ordinary compressed nullable TEXT(40)')
    page = 100 if header == 5 else 118
    base = page * PAGE
    original = raw[base:base + PAGE]
    require(len(rows) == 3 and [row['end'] - row['start'] for row in rows] == [121, 66, 73],
            'Actual complete original record lengths')
    require([row['reference'] for row in rows] == [page << 8 | slot for slot in range(3)],
            'Original live root identities')
    require(measured['source_sha256'] == hashlib.sha256(raw).hexdigest()
            and measured['getter_sha256'] == hashlib.sha256(getter.encode()).hexdigest(),
            'Bound original map evidence')
    require(len(measured['data_pages']) == 1 and measured['data_pages'][0]
            == {'number': page, 'slot_count': 3, 'last_record_start': 3836,
                'free_bytes': 3816, 'candidate': True, 'global_free': False,
                'sha256': hashlib.sha256(original).hexdigest()}, 'Measured unique hinted parent')
    require(framing.number(original, 12, 2) == 3
            and framing.number(original, 2, 2) == 3816, 'Original DATA header')
    candidate = framing.original_map(raw, framing.number(raw, layout['page'] * PAGE + 59))
    require(layout['owned'] == measured['owned'] and candidate == measured['candidate']
            and framing.original_map(raw, 256) == measured['global_free'], 'Exact original map inventories')
    require(page in candidate['members'] and page in layout['owned']['members']
            and page not in measured['global_free']['members'], 'Hinted allocated DATA')
    require(candidate['members'] == [page], 'Finite unique candidate order')
    updates = request['updates']
    require(len(updates) in [1, 2] and len({item['ordinal'] for item in updates}) == len(updates),
            'Distinct finite original requests')
    result = bytearray(original)
    count, end = 3, 3836
    records = []
    lines = list(metadata['lines'])
    for update in sorted(updates, key=lambda item: item['ordinal']):
        ordinal = update['ordinal']
        require(ordinal in [0, 1] and update['row_id'] == {'ID': ordinal + 1}, 'Literal row authority')
        row = rows[ordinal]
        require(framing.integer(raw, row, layout['fields'][0]) == ordinal + 1, 'Raw original ID')
        expected = {'kind': 'Text', 'value': 'alpha'} if ordinal == 0 else {'kind': 'Null'}
        require(update['expected'] == expected, 'Literal original precondition')
        start, stop = row['offsets'][:2]
        old = raw[row['start']:row['end']]
        require(bool(old[-1] & 2) == (ordinal == 0)
                and old[start:stop] == (b'\xff\xfealpha' if ordinal == 0 else b''),
                'Raw original selected presence and bytes')
        rebuilt, offsets = rebuild(raw, row, update)
        destination = end - len(rebuilt)
        require(destination >= 14 + (count + 1) * 2, 'Literal same-page capacity')
        result[destination:end] = rebuilt
        put16(result, 14 + count * 2, destination | 0x8000)
        root_start = row['start'] - base
        root_end = row['end'] - base
        require(framing.number(original, 14 + ordinal * 2, 2) == root_start,
                'Original unforwarded root word')
        put16(result, 14 + ordinal * 2, root_start | 0x4000)
        pointer = page << 8 | count
        result[root_start:root_start + 4] = pointer.to_bytes(4, 'little')
        result[root_start + 4:root_end] = bytes(root_end - root_start - 4)
        records.append({'ordinal': ordinal, 'root_reference': row['reference'],
                        'terminal_reference': pointer, 'original_span': [row['start'], row['end']],
                        'hidden_span': [base + destination, base + end],
                        'length': len(rebuilt), 'offsets': offsets,
                        'sha256': hashlib.sha256(rebuilt).hexdigest()})
        matches = 0
        for index, line in enumerate(lines):
            fields = line.split('\t')
            if fields[:3] == ['ROW', 'PreservedRows', str(ordinal)]:
                require(fields[3] == f'{page}:{ordinal}' and fields[4] == f'I32:{ordinal + 1}'
                        and fields[5] == token(expected), 'SDK original root and typed cell')
                fields[5] = token(update['replacement'])
                lines[index] = '\t'.join(fields)
                matches += 1
        require(matches == 1, 'One original reader row')
        count += 1
        end = destination
    free = end - (14 + count * 2)
    require(free == (3614 if len(updates) == 2 else 3688 if updates[0]['ordinal'] == 0 else 3736),
            'Literal final free counter')
    put16(result, 12, count)
    put16(result, 2, free)
    expected = raw[:base] + bytes(result) + raw[base + PAGE:]
    require(len(expected) == len(raw), 'Exact EOF and all other pages retained')
    # Re-read only the bounded forwarding domain, never the ordinary-only reader.
    allowed = set(range(2, 4)) | set(range(12, 14 + count * 2))
    for ordinal in range(3):
        if ordinal not in {record['ordinal'] for record in records}:
            at = 14 + ordinal * 2
            require(result[at:at + 2] == original[at:at + 2],
                    'Every unselected original directory word retained')
    for record in records:
        ordinal = record['ordinal']
        root = rows[ordinal]['start'] - base
        root_end = rows[ordinal]['end'] - base
        require(framing.number(result, root) == record['terminal_reference']
                and result[root + 4:root_end] == bytes(root_end - root - 4), 'Exact forwarding capsule')
        require(framing.number(result, 14 + ordinal * 2, 2) == root | 0x4000,
                'Exact relocated live root slot word')
        slot = record['terminal_reference'] & 255
        first, last = [value - base for value in record['hidden_span']]
        require(framing.number(result, 14 + slot * 2, 2) == first | 0x8000,
                'Hidden terminal slot word')
        require(hashlib.sha256(result[first:last]).hexdigest() == record['sha256'],
                'Complete independently rebuilt terminal')
        mask = last - 1
        reread = [framing.number(result, mask - 4 - index * 2, 2) for index in range(5)]
        require(reread == record['offsets'] and framing.number(result, mask - 2, 2) == 4
                and result[mask] & 2, 'Bounded terminal footer and selected presence')
        allowed.update(range(root, root_end))
        allowed.update(range(first, last))
    require(all(before == after or at in allowed
                for at, (before, after) in enumerate(zip(original, result, strict=True))),
            'Every other parent byte and opaque free tail retained')
    require(framing.number(result, 12, 2) == count and framing.number(result, 2, 2) == free,
            'Re-read DATA slot count and free counter')
    proof = {'request': request, 'source_sha256': hashlib.sha256(raw).hexdigest(),
             'expected_sha256': hashlib.sha256(expected).hexdigest(),
             'records': records, 'parent_page': page, 'final_slot_count': count,
             'final_free_bytes': free, 'all_other_pages_maps_indexes_unchanged': True,
             'expected_outcome': {'kind': 'candidate', 'changed_rows': len(updates)},
             'normal_save_or_journal_authority': False}
    return expected, proof, ('\n'.join(lines) + '\n').encode('utf-8')
