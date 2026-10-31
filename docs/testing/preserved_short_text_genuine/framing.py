"""Original-only rich Short Text framing; no construction or subprocesses."""
import base64
import hashlib

EXPECTED_COLUMNS = ['ID', 'Short_Text', 'Base', 'Cached', 'Flag', 'Memo', 'Blob', 'Files']
EXPECTED_VALUES = ['alpha', None, 'omega']
SOURCE_BYTES_MAX = 2 * 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def inspect_original(raw, getter, code, framing):
    require(0 < len(raw) <= SOURCE_BYTES_MAX and len(raw) % framing.PAGE == 0,
            'Bounded complete original')
    require(raw[20] == code, 'Exact original header')
    metadata = framing.sdk_inventory(getter)
    tables = metadata['tables']
    layouts = {
        name: framing.definition(raw, table)
        for name, table in tables.items()
    }
    observed = tables['PreservedRows']
    layout = layouts['PreservedRows']
    require([field['name'] for field in layout['fields']] == EXPECTED_COLUMNS,
            'Original rich field inventory')
    require(len(observed['records']) == 3, 'Three original parent records')
    target = layout['fields'][1]
    require((target['native'], target['number'], target['flags'],
             target['extended'], target['length']) == (10, 1, 2, 1, 80),
            'Original nullable compressed ordinary TEXT(40) descriptor')
    require(not observed['columns'][1]['calculated']
            and not observed['columns'][1]['auto'], 'Ordinary selected field')
    require(len(layout['physical']) == 2, 'Two original physical indexes')
    require(all(target['number'] != key[0] for index in layout['physical']
                for key in index['keys']), 'Selected text is not indexed')
    properties = metadata['records']
    for name, value in [('Required', 'B0'), ('AllowZeroLength', 'B1')]:
        require(['PROPERTY', 'PreservedRows', 'Short_Text', name, '1', '1', value]
                in properties, 'Exact original selected field property')
    require(['PROPERTY', 'PreservedRows', '', 'PublishToWeb', '2', '0', 'U8:0']
            in properties, 'Original source policy')
    expressions = [row for row in properties
                   if row[:4] == ['PROPERTY', 'PreservedRows', 'Cached', 'Expression']]
    require(len(expressions) == 1 and expressions[0][6].startswith('T:'),
            'Single original calculated expression')
    require(base64.b64decode(expressions[0][6][2:], validate=True).decode('utf-8')
            == '[Base]*2', 'Original dependency excludes selected text')
    require([row['cells'][3] for row in observed['records']]
            == ['I32:999', 'I32:-333', 'I32:0'], 'Stale original stored caches')

    rows = framing.records(raw, layout, observed)
    require([framing.integer(raw, row, layout['fields'][0]) for row in rows]
            == [1, 2, 3], 'Raw original IDs and ordinals')
    require([framing.integer(raw, row, layout['fields'][2]) for row in rows]
            == [7, 11, 13], 'Raw original calculation inputs')
    for index in layout['physical']:
        require(len(index['keys']) == 1, 'Closed ordinary integer key profile')
        native, ascending = index['keys'][0]
        require(native in {0, 2} and ascending, 'Only original ID/Base key owners')
        framing.integer_leaf(raw, layout, index, rows, layout['fields'][native])

    # Every role/map observation is read from original bytes, without granting
    # reservations, rewrite authority or interpreting opaque sibling records.
    global_free = framing.original_map(raw, 256)
    selected_pages = set(layout['owned']['members'])
    protected = set(global_free['storage_pages'])
    for name, other in layouts.items():
        protected.add(other['page'])
        protected.update(other['owned']['storage_pages'])
        if name != 'PreservedRows':
            require(not selected_pages.intersection(other['owned']['members']),
                    'No other-table DATA ownership of parent records')
        for index in other['physical']:
            protected.update(index['owned']['storage_pages'])
            require(not selected_pages.intersection(index['owned']['members']),
                    'No INDEX body ownership of parent DATA')
        for pair in other['pairs']:
            owned = framing.original_map(raw, pair['owned'])
            candidate = framing.original_map(raw, pair['candidate'])
            protected.update(owned['storage_pages'])
            protected.update(candidate['storage_pages'])
            require(not selected_pages.intersection(owned['members']),
                    'No LVAL body ownership of parent DATA')
    require(not selected_pages.intersection(protected), 'No metadata/map DATA overlap')
    require(all(global_free['coverage_span'][0] <= page < global_free['coverage_span'][1]
                for page in selected_pages), 'GLOBAL represents original parent DATA')
    require(not selected_pages.intersection(global_free['members']),
            'Original parent DATA is allocated')

    measurements = []
    variable = target['variable_index']
    for row, sdk, value in zip(rows, observed['records'], EXPECTED_VALUES, strict=True):
        require(sdk['cells'][0] == f'I32:{row["ordinal"] + 1}', 'SDK original ID')
        mask = raw[row['mask'] + target['number'] // 8]
        present = bool(mask & (1 << (target['number'] % 8)))
        require(present == (value is not None), 'Original selected presence')
        offsets = row['offsets']
        require(variable + 1 < len(offsets), 'Original selected variable slot')
        start = row['start'] + offsets[variable]
        stop = row['start'] + offsets[variable + 1]
        payload = raw[start:stop]
        if value is None:
            require(sdk['cells'][1] == 'N', 'SDK original NULL')
            representation = 'absent; payload bytes remain uninterpreted'
        else:
            expected_cell = 'T:' + base64.b64encode(value.encode('utf-8')).decode('ascii')
            require(sdk['cells'][1] == expected_cell, 'SDK original literal text')
            raw_utf16 = value.encode('utf-16le')
            compact = b'\xff\xfe' + value.encode('latin-1')
            require(payload in {raw_utf16, compact}, 'Original selected text wire/SDK equality')
            representation = 'raw UTF16' if payload == raw_utf16 else 'compact Unicode'
        logical_end = row['start'] + offsets[-1]
        slack = row['footer'] - logical_end
        require(slack >= 0, 'Original internal slack extent')
        capacity = len(payload) + slack
        measurements.append({
            'source_ordinal': row['ordinal'],
            'logical_root_reference': row['reference'],
            'terminal_reference': row['reference'],
            'record_span': [row['start'], row['end']],
            'record_length': row['end'] - row['start'],
            'native_field_number': target['number'],
            'variable_index': variable,
            'present': present,
            'original_text': value,
            'representation': representation,
            'selected_payload_span': [start, stop],
            'selected_payload_hex': payload.hex(),
            'mask_offset': row['mask'],
            'mask_hex': row['mask_hex'],
            'variable_offsets': offsets,
            'data_end': row['footer'],
            'logical_payload_end': logical_end,
            'internal_slack': slack,
            'fitting_payload_capacity': capacity,
            'capacity_equation': 'selected original span + original internal slack',
        })
    return {
        'scope': 'original-only raw framing measurement; no expected requests or images',
        'header': code,
        'source_sha256': hashlib.sha256(raw).hexdigest(),
        'definition_page': layout['page'],
        'target_descriptor_offset': target['descriptor_offset'],
        'physical_index_count': len(layout['physical']),
        'logical_index_count': len(layout['logical']),
        'fields': layout['fields'],
        'rows': measurements,
        'source_admission_or_write_permission': False,
    }
