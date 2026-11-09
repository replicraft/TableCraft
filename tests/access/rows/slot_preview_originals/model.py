"""Independent fixed-selector model for existing-span typed Slot candidates."""
import json

from . import common, facts

TEXT_TYPE = 10
BINARY_TYPE = 9
UTF16_UNIT_BYTES = 2
COMPACT_MARKER = b'\xff\xfe'
WRITABLE_FLAG = 2
COMPRESSION_FLAG = 1
GENERAL_LCID = 1033
GENERAL_VARIANT = 0
GENERAL_VERSIONS = (0, 1)
VARIABLE_WORD_BYTES = 2
MASK_UNIT_BITS = 8


def value(token):
    if token in ['NULL', 'null']:
        return ('null', b'')
    for prefix, kind in [('String16:', 'text'), ('text16:', 'text'),
                         ('Binary:', 'binary'), ('binary:', 'binary')]:
        if token.startswith(prefix):
            return kind, bytes.fromhex(token[len(prefix):])
    raise AssertionError('Unexpected selected getter/value carrier')


def getter(typed):
    kind, data = typed
    return {'null': 'NULL', 'text': 'String16:' + data.hex(),
            'binary': 'Binary:' + data.hex()}[kind]


def representations(typed, width):
    kind, data = typed
    if kind == 'binary':
        return [data.hex()] if len(data) == width else []
    if kind == 'null':
        return []
    assert len(data) % UTF16_UNIT_BYTES == 0
    units = [int.from_bytes(data[start:start + UTF16_UNIT_BYTES], 'little')
             for start in range(0, len(data), UTF16_UNIT_BYTES)]
    choices = []
    # A leading FEFF raw unit aliases the native compression marker. It cannot
    # establish a faithful plain UTF16 representation for this admitted codec.
    if len(data) == width and not data.startswith(COMPACT_MARKER):
        choices.append(data.hex())
    if len(units) + len(COMPACT_MARKER) == width and all(0 < unit <= 255 for unit in units):
        choices.append((COMPACT_MARKER + bytes(units)).hex())
    return choices


def property_value(table, column, name):
    properties = [entry for entry in table['properties'] if entry[0] == column and entry[1] == name]
    assert len(properties) <= 1
    return properties[0][-1] if properties else None


def admission(table, row_id, column, requested):
    field = table['columns'][column]
    row = table['rows'][str(row_id)]
    slot = row['slots'][column]
    old = value(row['values'][field['position']])
    kind, data = requested
    proof = {'old_getter': getter(old), 'replacement_getter': getter(requested),
             'original_width': slot['byte_width'], 'original_present': slot['present'],
             'original_wire_hex': slot['wire_hex'], 'legal_representations': []}
    if field['flags'] != WRITABLE_FLAG:
        return False, 'field writable flag', proof
    if field['native_type'] not in [TEXT_TYPE, BINARY_TYPE]:
        return False, 'field carrier profile', proof
    if field['extended_flags'] & ~COMPRESSION_FLAG:
        return False, 'field extension profile', proof
    affected = [index for index in table['indexes'].values()
                if any(entry[0] == column for entry in index['columns'])]
    for index in affected:
        if index['ignored_null']:
            return False, 'ignore-NULL index profile', proof
        for indexed_column, _ in index['columns']:
            component = table['columns'][indexed_column]
            if component['native_type'] == TEXT_TYPE:
                sort = component['sort']
                known_family = (sort['lcid'], sort['variant']) == (GENERAL_LCID, GENERAL_VARIANT)
                if not known_family or sort['version'] not in GENERAL_VERSIONS:
                    return False, 'indexed non-General sort profile', proof
    if requested != old:
        if kind == 'null':
            proof['legal_representations'] = [slot['wire_hex']]
        else:
            proof['legal_representations'] = representations(requested, slot['byte_width'])
            if not proof['legal_representations']:
                return False, 'no faithful representation in original span', proof
    else:
        proof['legal_representations'] = [slot['wire_hex']]
    if kind == 'null' and property_value(table, column, 'Required') == 'Boolean:true':
        return False, 'Required final value', proof
    if kind == 'text' and not data and property_value(table, column, 'AllowZeroLength') == 'Boolean:false':
        return False, 'AllowZeroLength final value', proof
    rule = property_value(table, column, 'ValidationRule')
    if rule is not None and kind != 'null':
        assert rule.startswith('String16:')
        expression = bytes.fromhex(rule[len('String16:'):]).decode('utf-16-le')
        if expression == 'Len([Label]) <= 3':
            if len(data) // UTF16_UNIT_BYTES > 3:
                return False, 'Len validation rule', proof
        elif expression == 'StrComp([Label],"z",0) < 0':
            assert kind == 'text'
            if data.decode('utf-16-le') >= 'z':
                return False, 'Binary StrComp validation rule', proof
        else:
            raise AssertionError('Unmodeled field rule')
    for index in affected:
        if index['unique'] and len(index['columns']) == 1:
            for identity, other in table['rows'].items():
                if int(identity) != row_id and value(other['values'][field['position']]) == requested:
                    return False, 'unique exact-value duplicate', proof
    return True, 'qualified original-span assignment', proof


def fixed_plan(facts, reports):
    by_profile = {item['profile']: item for item in facts}
    selectors = []
    for series, report_path in reports:
        report = json.loads(report_path.read_text())
        for position, case in enumerate(report['cases']):
            profile = case['profile']
            original = by_profile[profile]
            assert common.pin(common.path(original['input']['path'])) == original['input']
            table = original['tables'][case['table']]
            identity = case['ID']
            requested = value(case.get('requested', 'null'))
            expected, reason, proof = admission(table, identity, case['column'], requested)
            rows = list(table['rows'])
            physical = sorted(
                rows, key=lambda name: (table['rows'][name]['page'], table['rows'][name]['slot']),
            )
            assert rows == physical
            ordinal = physical.index(str(identity))
            old = value(table['rows'][str(identity)]['values'][table['columns'][case['column']]['position']])
            selectors.append({'series': series, 'reference_position': position,
                              'profile': profile, 'label': case.get('case', case['table'] + '-nonempty-null'),
                              'table': case['table'], 'column': case['column'], 'ID': identity,
                              'ordinal': ordinal, 'expected_kind': old[0], 'expected_data_hex': old[1].hex(),
                              'replacement_kind': requested[0], 'replacement_data_hex': requested[1].hex(),
                              'expected_candidate': expected, 'reason': reason, 'proof': proof,
                              'reference_succeeded': case['action']['status'] == 0,
                              'reference_reopened_requested_value': case.get(
                                  'requested_value_reopened_exactly', case.get('reopened_NULL'),
                              ),
                              'reference_record': common.pin(report_path)})
    assert len(selectors) == 162
    return selectors


def string(text):
    return 'text16:' + text.encode('utf-16-le').hex()


def binary(length, seed=255):
    data = bytes((seed + index * 31 & 255 for index in range(length)))
    return 'binary:' + data.hex()


def reference_cases():
    result = []
    for table in ['CompactLegacy', 'RawLegacy', 'CompactModern', 'RawModern']:
        entries = [
            ('raw-to-ascii', 3, string('ABCD'), 'storage'),
            ('ascii-to-raw', 4, string('ĀāĂ'), 'storage'),
            ('latin', 5, string('abc'), 'storage'),
            ('mixed', 7, string('Bβ'), 'storage'),
            ('nul', 9, string('C\x00D'), 'storage'),
            ('null-to-empty', 1, string(''), 'null-empty'),
            ('empty-to-null', 2, 'null', 'null-empty'),
            ('null-to-nonempty', 1, string('A'), 'resize-observation'),
        ]
        for name, identity, value, scope in entries:
            result.append((table + '-' + name, table, identity, 'Label', value, scope))
    result.extend([
        ('noncanonical-empty-noop', 'CompactLegacy', 10, 'Label', string(''), 'noncanonical-noop'),
        ('noncanonical-ab-noop', 'CompactLegacy', 11, 'Label', string('AB'), 'noncanonical-noop'),
        ('leading-feff-write', 'CompactLegacy', 3, 'Label', string('\ufeffAB'), 'ambiguous-feff'),
        ('one-unit-threshold', 'CompactLegacy', 8, 'Label', string('Z'), 'storage'),
    ])
    result.extend([
        ('binary-null-to-empty', 'BinaryStorage', 1, 'Bytes', binary(0), 'null-empty'),
        ('binary-empty-to-null', 'BinaryStorage', 2, 'Bytes', 'null', 'null-empty'),
    ])
    for identity, length in enumerate([1, 7, 8, 9, 16, 17, 254, 255], start=3):
        result.append((
            'binary-width-' + str(length),
            'BinaryStorage',
            identity,
            'Bytes',
            binary(length),
            'storage',
        ))
    result.append(('binary-null-to-nonempty', 'BinaryStorage', 1, 'Bytes', binary(7), 'resize-observation'))
    for table in ['IndexedLegacy', 'IndexedModern']:
        result.extend([
            (table + '-text-new-key', table, 3, 'Label', string('Omega'), 'indexed'),
            (table + '-text-case-key', table, 4, 'Label', string('BETA'), 'indexed'),
            (table + '-text-null-empty', table, 1, 'Label', string(''), 'indexed'),
            (table + '-text-six-byte', table, 11, 'Label', string('ABCD'), 'indexed'),
            (table + '-binary-seven', table, 3, 'Bytes', binary(7), 'indexed'),
            (table + '-binary-eight', table, 4, 'Bytes', binary(8), 'indexed'),
            (table + '-binary-nine', table, 5, 'Bytes', binary(9), 'indexed'),
            (table + '-binary-empty-null', table, 2, 'Bytes', 'null', 'indexed'),
        ])
    for table in ['UniqueLegacy', 'UniqueModern']:
        result.extend([
            (table + '-duplicate-text', table, 1, 'Label', string('uniq-0002'), 'unique-observation'),
            (
                table + '-duplicate-binary',
                table,
                1,
                'Bytes',
                'binary:' + 2 .to_bytes(8, 'big').hex(),
                'unique-observation',
            ),
            (table + '-unique-text', table, 1, 'Label', string('uniq-1001'), 'indexed'),
        ])
    result.extend([
        ('required-to-null', 'RequiredText', 1, 'Label', 'null', 'required-property'),
        ('noempty-to-empty', 'NoEmptyText', 1, 'Label', string(''), 'empty-property'),
        (
            'len-disabled-expression',
            'LenLimited',
            1,
            'Label',
            string('ABCD'),
            'expression-disabled',
        ),
        ('strcmp-disabled-expression', 'StrComp0', 1, 'Label', string('z'), 'expression-disabled'),
        ('french-public', 'FrenchBlocked', 1, 'Label', string('def'), 'engine-held-profile'),
        ('ignored-text-public', 'IgnoredText', 2, 'Label', string('def'), 'engine-held-profile'),
        ('ignored-binary-public', 'IgnoredBinary', 2, 'Bytes', binary(7), 'engine-held-profile'),
    ])
    assert len(result) == 76
    return result


def slot(table, identity, column):
    row = table['rows'][identity]
    data = bytes.fromhex(table['raw_rows'][identity])
    field = table['columns'][column]
    mask = len(data) - (table['max_columns'] + MASK_UNIT_BITS - 1) // MASK_UNIT_BITS
    count = facts.number(data, mask - VARIABLE_WORD_BYTES, VARIABLE_WORD_BYTES)
    assert field['variable'] < count
    first = mask - VARIABLE_WORD_BYTES - VARIABLE_WORD_BYTES * (field['variable'] + 1)
    second = mask - VARIABLE_WORD_BYTES - VARIABLE_WORD_BYTES * (field['variable'] + 2)
    start = facts.number(data, first, VARIABLE_WORD_BYTES)
    end = facts.number(data, second, VARIABLE_WORD_BYTES)
    present = bool(data[mask + field['number'] // MASK_UNIT_BITS] & 1 << field['number'] % MASK_UNIT_BITS)
    return {
        'start': start,
        'end': end,
        'byte_width': end - start,
        'present': present,
        'wire_hex': data[start:end].hex(),
        'getter': row['values'][field['position']],
        'page': row['page'],
        'slot': row['slot'],
        'record_bytes': len(data),
    }
