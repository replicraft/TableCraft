"""Predeclared PAGE selectors; no candidate invocation or production imports."""
import argparse
import copy
import hashlib
import json
from collections import Counter
from pathlib import Path

from slot_preview_originals import common, facts, model as SLOT

ROOT = common.ROOT

PAGE_BYTES = 4096
SLOT_START = 14
SLOT_BYTES = 2
ROW_COUNT_OFFSET = 12
FREE_OFFSET = 2
OWNER_OFFSET = 4
OFFSET_MASK = 0x1FFF
FORWARDED = 0x4000
HIDDEN = 0x8000
POINTER_BYTES = 4
MAX_PAGE_ROWS = 255
MAP_HEADER_BYTES = 5
TABLE_FREE_REFERENCE = 59
COLUMN_COUNT_BYTES = 2
VARIABLE_WORD_BYTES = 2
MASK_BITS = 8
COMPACT_MARKER = b'\xff\xfe'


def pin(path):
    path = Path(path)
    data = path.read_bytes()
    return {
        'path': path.as_posix(),
        'bytes': len(data),
        'sha256': hashlib.sha256(data).hexdigest(),
    }


def verify(record):
    assert pin(record['path']) == record


def number(raw, offset, width=SLOT_BYTES):
    return int.from_bytes(raw[offset:offset + width], 'little')


def frame(raw, page, slot):
    base = page * PAGE_BYTES
    count = number(raw, base + ROW_COUNT_OFFSET)
    assert 0 <= slot < count <= MAX_PAGE_ROWS
    word = number(raw, base + SLOT_START + slot * SLOT_BYTES)
    start = base + (word & OFFSET_MASK)
    end = base + PAGE_BYTES
    if slot:
        end = base + (number(raw, base + SLOT_START + (slot - 1) * SLOT_BYTES) & OFFSET_MASK)
    assert start < end
    return {
        'page': page,
        'slot': slot,
        'word': word,
        'start': start,
        'end': end,
        'length': end - start,
        'body': raw[start:end],
    }


def final_frame(raw, row):
    current = frame(raw, row['page'], row['slot'])
    visited = set()
    while current['word'] & FORWARDED:
        identity = (current['page'], current['slot'])
        assert identity not in visited
        visited.add(identity)
        assert current['length'] >= POINTER_BYTES
        reference = number(current['body'], 0, POINTER_BYTES)
        current = frame(raw, reference >> 8, reference & 255)
    return current


def payload(typed, field):
    kind, data = typed
    if kind == 'null':
        return b''
    if kind == 'binary':
        return data
    assert kind == 'text' and len(data) % 2 == 0
    units = [number(data, offset) for offset in range(0, len(data), 2)]
    if field['compressed'] and len(units) > 2 and all(0 < unit <= 255 for unit in units):
        return COMPACT_MARKER + bytes(units)
    if data.startswith(COMPACT_MARKER):
        raise ValueError('unfaithful leading FEFF representation')
    return data


def constraints(table, identity, column, requested):
    field = table['columns'][column]
    if field['flags'] != 2 or field['native_type'] not in (9, 10):
        return 'field profile'
    if field['extended_flags'] & ~1:
        return 'field extension profile'
    affected = []
    for index in table['indexes'].values():
        if any(component[0] == column for component in index['columns']):
            affected.append(index)
    for index in affected:
        if index['ignored_null']:
            return 'ignore-NULL index profile'
        for label, _ in index['columns']:
            component = table['columns'][label]
            if component['native_type'] == 10:
                sort = component['sort']
                if (sort['lcid'], sort['variant'], sort['version']) not in [(1033, 0, 0), (1033, 0, 1)]:
                    return 'indexed non-General sort profile'
    kind, data = requested
    if kind == 'null' and SLOT.property_value(table, column, 'Required') == 'Boolean:true':
        return 'Required final value'
    if kind == 'text' and not data and SLOT.property_value(table, column, 'AllowZeroLength') == 'Boolean:false':
        return 'AllowZeroLength final value'
    rule = SLOT.property_value(table, column, 'ValidationRule')
    if rule is not None and kind != 'null':
        expression = bytes.fromhex(rule.removeprefix('String16:')).decode('utf-16-le')
        if expression == 'Len([Label]) <= 3':
            if len(data) // 2 > 3:
                return 'Len validation rule'
        elif expression == 'StrComp([Label],"z",0) < 0':
            if data.decode('utf-16-le') >= 'z':
                return 'Binary StrComp validation rule'
        else:
            raise AssertionError('Unmodeled validation rule')
    for index in affected:
        if not index['unique'] or len(index['columns']) != 1:
            continue
        for other_id, other in table['rows'].items():
            actual = SLOT.value(other['values'][field['position']])
            if int(other_id) != identity and actual == requested:
                return 'unique exact-value duplicate'
    return None


def admission(source, table, identity, column, requested):
    verify(source)
    raw = Path(source['path']).read_bytes()
    row = table['rows'][str(identity)]
    field = table['columns'][column]
    old = SLOT.value(row['values'][field['position']])
    root = frame(raw, row['page'], row['slot'])
    target = final_frame(raw, row)
    owners = []
    for other_id, other in table['rows'].items():
        other_target = final_frame(raw, other)
        if other_target['start'] == target['start']:
            owners.append(int(other_id))
    assert owners == [identity]
    assert number(raw, root['page'] * PAGE_BYTES + OWNER_OFFSET, 4) == table['definition']
    assert number(raw, target['page'] * PAGE_BYTES + OWNER_OFFSET, 4) == table['definition']
    body = target['body']
    assert body.hex() == table['raw_rows'][str(identity)]
    mask_bytes = (table['max_columns'] + MASK_BITS - 1) // MASK_BITS
    mask = len(body) - mask_bytes
    variables = number(body, mask - VARIABLE_WORD_BYTES)
    offsets = []
    for position in range(variables + 1):
        offsets.append(number(body, mask - VARIABLE_WORD_BYTES * (position + 2)))
    variable = field['variable']
    old_width = offsets[variable + 1] - offsets[variable]
    data_end = mask - VARIABLE_WORD_BYTES * (variables + 2)
    assert offsets[-1] <= data_end
    proof = {'root_page': root['page'], 'root_slot': root['slot'],
             'root_word': root['word'], 'root_bytes': root['length'],
             'target_page': target['page'], 'target_slot': target['slot'],
             'old_physical_record_bytes': len(body), 'old_field_span_bytes': old_width,
             'payload_end': offsets[-1], 'data_end': data_end,
             'footer_bytes': len(body) - data_end, 'exclusive_target_owners': owners,
             'old_getter': SLOT.getter(old), 'new_getter': SLOT.getter(requested)}
    reason = constraints(table, identity, column, requested)
    if reason:
        return False, reason, proof
    if old == requested:
        proof['mode'] = 'semantic-noop'
        proof['new_physical_record_bytes'] = len(body)
        return True, 'original byte identity', proof
    try:
        encoded = payload(requested, field)
    except ValueError as error:
        return False, str(error), proof
    minimum = offsets[-1] - old_width + len(encoded) + len(body) - data_end
    rebuilt = max(len(body), minimum)
    proof['preferred_payload_hex'] = encoded.hex()
    proof['minimum_rebuilt_record_bytes'] = minimum
    proof['new_physical_record_bytes'] = rebuilt
    if rebuilt == len(body):
        proof['mode'] = 'fitted'
        return True, 'existing physical capacity', proof
    page_start = root['page'] * PAGE_BYTES
    count = number(raw, page_start + ROW_COUNT_OFFSET)
    free = number(raw, page_start + FREE_OFFSET)
    reference = number(raw, table['definition'] * PAGE_BYTES + TABLE_FREE_REFERENCE, 4)
    hint_frame = frame(raw, reference >> 8, reference & 255)
    hint = hint_frame['body']
    assert hint[0] == 0
    base = number(hint, 1, 4)
    relative = root['page'] - base
    assert relative >= 0
    byte = MAP_HEADER_BYTES + relative // MASK_BITS
    represented = byte < len(hint)
    bit = represented and bool(hint[byte] & (1 << relative % MASK_BITS))
    proof.update({'mode': 'append', 'original_physical_slots': count,
                  'root_page_free_bytes': free, 'append_bytes_including_slot': rebuilt + SLOT_BYTES,
                  'hint_reference': reference, 'hint_represented': represented, 'hint_true': bit})
    if root['length'] < POINTER_BYTES or count >= MAX_PAGE_ROWS or not bit or free < rebuilt + SLOT_BYTES:
        return False, 'append authority or capacity', proof
    return True, 'same-root-page hidden append', proof


def make_selector(series, label, profile, source, table, identity, column, requested, getter_pin=None):
    expected, reason, proof = admission(source, table, identity, column, requested)
    actual = SLOT.value(table['rows'][str(identity)]['values'][table['columns'][column]['position']])
    physical = sorted(table['rows'], key=lambda key: (table['rows'][key]['page'], table['rows'][key]['slot']))
    return {
        'series': series,
        'label': label,
        'profile': profile,
        'input': source,
        'table': table['name'],
        'column': column,
        'ID': identity,
        'ordinal': physical.index(str(identity)),
        'expected': SLOT.getter(actual),
        'replacement': SLOT.getter(requested),
        'expected_candidate': expected,
        'reason': reason,
        'proof': proof,
        'getter': getter_pin,
    }


def history_table(original, getter, name):
    verify(getter)
    parsed = facts.parse(Path(getter['path']).read_text())[name]
    table = copy.deepcopy(original)
    table['rows'] = {str(key): value for key, value in parsed['rows'].items()}
    table['raw_rows'] = {str(key): value for key, value in parsed['raw_rows'].items()}
    table['name'] = name
    return table


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--initial-report', type=Path, required=True)
    parser.add_argument('--null-report', type=Path, required=True)
    parser.add_argument('--resize-report', type=Path, required=True)
    parser.add_argument('--history-report', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    for name in vars(args):
        setattr(args, name, common.path(getattr(args, name)).relative_to(ROOT))
    args.output.relative_to('.cache')
    assert not args.output.exists()
    assert common.pin(args.jar)['sha256'] == common.JAR_SHA256
    originals = json.loads(args.facts.read_text())
    by_profile = {item['profile']: item for item in originals}
    selectors = []
    input_reports = [args.initial_report, args.null_report]
    for report_path in input_reports:
        report = json.loads(report_path.read_text())
        for case in report['cases']:
            profile = case['profile']
            original = by_profile[profile]
            table = copy.deepcopy(original['tables'][case['table']])
            table['name'] = case['table']
            requested = SLOT.value(case.get('requested', 'null'))
            selectors.append(make_selector('Slot162', case.get('case', case['table'] + '-nonempty-null'),
                                           profile, original['input'], table, case['ID'], case['column'], requested))
    assert len(selectors) == 162
    resize = json.loads(args.resize_report.read_text())
    grown = [case for case in resize['cases'] if case['header_after']['flags'] == FORWARDED]
    assert len(grown) == 14
    references = json.loads(input_reports[0].read_text())
    by_copy = {case['created_copy']['path']: case for case in references['cases'] if 'created_copy' in case}
    for case in grown:
        source = case['copy']
        reference = by_copy[source['path']]
        getter = reference['getter']['stdout']
        original = by_profile[case['profile']]['tables'][case['table']]
        table = history_table(original, getter, case['table'])
        position = table['columns'][reference['column']]['position']
        actual = SLOT.value(table['rows'][str(case['ID'])]['values'][position])
        selectors.append(make_selector('grown-reference-noop', reference['case'], case['profile'], source,
                                       table, case['ID'], reference['column'], actual, getter))
    history = json.loads(args.history_report.read_text())
    for profile in history['profiles']:
        original = by_profile[profile['profile']]['tables']['CompactLegacy']
        for step in profile['steps']:
            getter = step['after_capture'][0]['stdout']
            table = history_table(original, getter, 'CompactLegacy')
            replacement = 'B' if step['case'] == 'grow' else 'ABCDEFGHIJKLMN'
            requested = ('text', replacement.encode('utf-16-le'))
            selectors.append(make_selector('forward-history', step['case'], profile['profile'],
                                           step['created_copy'], table, 1, 'Label', requested, getter))
    assert len(selectors) == 180
    summary = Counter('candidate' if case['expected_candidate'] else case['reason'] for case in selectors)
    modes = Counter(case['proof'].get('mode', 'refused-before-record-rebuild') for case in selectors)
    immutable_paths = set(input_reports)
    immutable_paths.update([
        args.facts,
        args.resize_report,
        args.history_report,
        Path('docs/testing/slot_preview_originals/model.py'),
        Path('docs/testing/slot_preview_originals/facts.py'),
        Path('docs/testing/slot_preview_originals/common.py'),
        Path('docs/testing/capture_delete_trees.py'),
        Path('docs/testing/SlotUpdateFixture.java'),
        args.jar,
        args.classes / 'SlotUpdateFixture.class',
        Path(__file__).relative_to(ROOT) if Path(__file__).is_absolute() else Path(__file__),
    ])
    for original in originals:
        immutable_paths.add(Path(original['input']['path']))
        immutable_paths.add(Path(original['getter']['path']))
    for selector in selectors:
        immutable_paths.add(Path(selector['input']['path']))
        if selector['getter'] is not None:
            immutable_paths.add(Path(selector['getter']['path']))
    immutable_before = [pin(path) for path in sorted(immutable_paths)]
    for record in immutable_before:
        verify(record)

    output = {'scope': 'Predeclared PAGE engineering plan only; no candidate calls or new public actions.',
              'selectors': selectors, 'selector_count': len(selectors), 'summary': dict(summary),
              'modes': dict(modes), 'immutable_pins': immutable_before,
              'limitations': ['FEFF public contrary writes are not expected-value goldens.',
                              'No native maximum-slot or false-hint PAGE mutation case is claimed.',
                              'The original Slot162 selectors and receipts remain unchanged.']}
    output['immutable_pins_reverified'] = all(pin(item['path']) == item for item in immutable_before)
    assert output['immutable_pins_reverified']
    args.output.write_text(json.dumps(output, indent=2) + '\n')
    print(json.dumps({'selectors': len(selectors), 'summary': summary, 'modes': modes, 'plan': pin(args.output)}))


if __name__ == '__main__':
    main()
