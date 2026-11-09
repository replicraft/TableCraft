"""Independent raw observation/model over original SDK-authored QueryDef copies.

No TableCraft codec, planner, constructor or SDK encoder is called by this model.
The original metadata getter supplies only the source table-definition identity.
"""

from pathlib import Path
import hashlib
import json
import struct


PAGE = 4096
TARGET = -2147476548
ROOT = Path('.cache/querydef-existing-save-sdk-20261108')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def short(data, offset):
    return struct.unpack_from('<H', data, offset)[0]


def put_short(data, offset, value):
    struct.pack_into('<H', data, offset, value)


def records(image, owner):
    result = {}
    for number in range(len(image) // PAGE):
        page = image[number * PAGE:(number + 1) * PAGE]
        if page[:2] != b'\x01\x01' or struct.unpack_from('<I', page, 4)[0] != owner:
            continue
        end = PAGE
        for slot in range(short(page, 12)):
            raw = short(page, 14 + 2 * slot)
            start = raw & 0x1fff
            assert 14 + 2 * short(page, 12) <= start <= end <= PAGE
            result[(number, slot)] = {'word': raw, 'flags': raw & 0xe000,
                                      'start': start, 'end': end, 'raw': page[start:end]}
            end = start
    return result


def framing(raw):
    assert short(raw, 0) == 8
    assert short(raw, len(raw) - 3) == 4
    starts = [short(raw, len(raw) - 5 - 2 * index) for index in range(4)]
    data_end = short(raw, len(raw) - 13)
    footer = len(raw) - 13
    assert starts == sorted(starts)
    assert starts[0] == 13
    assert starts[-1] <= data_end <= footer
    return starts, data_end, footer


def selected(before, owner, object_id=TARGET, attribute=6, expression="42"):
    found = []
    for reference, record in records(before, owner).items():
        if record['flags'] != 0:
            continue
        raw = record['raw']
        if struct.unpack_from('<i', raw, 2)[0] == object_id and raw[6] == attribute:
            starts, end, footer = framing(raw)
            assert raw[starts[0]:starts[1]] == b'\x00\x00\x00\x01'
            encoded = expression.encode('utf-16le')
            assert raw[starts[3]:end] == struct.pack('<I', 0x80000000 | len(encoded)) + bytes(8) + encoded
            found.append((reference, record))
    assert len(found) == 1
    return found[0]


def authored_row(original, expression, minimum_space):
    starts, old_end, _ = framing(original)
    text = expression.encode('utf-16le')
    descriptor = struct.pack('<I', 0x80000000 | len(text)) + bytes(8) + text
    row = bytearray(original[:starts[3]] + descriptor)
    data_end = len(row)
    space = max(minimum_space, data_end + 13)
    row.extend(bytes(space - 13 - len(row)))
    row.extend(struct.pack('<H', data_end))
    for start in reversed(starts):
        row.extend(struct.pack('<H', start))
    row.extend(b'\x04\x00')
    row.extend(original[-1:])
    assert len(row) == space
    return bytes(row)


def expected(before, owner, expression, object_id=TARGET, attribute=6, original_expression="42"):
    (number, slot), original = selected(before, owner, object_id, attribute, original_expression)
    row = authored_row(original['raw'], expression, len(original['raw']))
    after = bytearray(before)
    base = number * PAGE
    if len(row) <= len(original['raw']):
        after[base + original['start']:base + original['end']] = row
    else:
        page = before[base:base + PAGE]
        count = short(page, 12)
        assert count < 255
        low = short(page, 14 + 2 * (count - 1)) & 0x1fff
        start = low - len(row)
        assert start >= 14 + 2 * (count + 1)
        root = struct.pack('<I', (number << 8) | count) + bytes(len(original['raw']) - 4)
        after[base + original['start']:base + original['end']] = root
        put_short(after, base + 14 + 2 * slot, original['word'] | 0x4000)
        put_short(after, base + 14 + 2 * count, 0x8000 | start)
        put_short(after, base + 12, count + 1)
        put_short(after, base + 2, short(page, 2) - len(row) - 2)
        after[base + start:base + low] = row
    return bytes(after)


def carrier(image, owner, reference):
    entries = records(image, owner)
    root = entries[reference]
    physical = reference
    if root['flags'] == 0x4000:
        pointer = struct.unpack_from('<I', root['raw'], 0)[0]
        assert root['raw'][4:] == bytes(len(root['raw']) - 4)
        physical = (pointer >> 8, pointer & 255)
    row = entries[physical]
    assert row['flags'] in [0, 0x8000]
    starts, end, footer = framing(row['raw'])
    raw = row['raw'][starts[3]:end]
    return {'logical_row': list(reference), 'root_flags': root['flags'],
            'root_span': [reference[0] * PAGE + root['start'], reference[0] * PAGE + root['end']],
            'physical_row': list(physical), 'physical_flags': row['flags'],
            'physical_span': [physical[0] * PAGE + row['start'], physical[0] * PAGE + row['end']],
            'expression_span': [physical[0] * PAGE + row['start'] + starts[3],
                                physical[0] * PAGE + row['start'] + end],
            'expression_field_hex': raw.hex(), 'used_data_end': end,
            'padding_hex': row['raw'][end:footer].hex(), 'physical_record_hex': row['raw'].hex()}


def main():
    observations = [line.split('\t') for path in [ROOT / 'sdk-observations.tsv', ROOT / 'sdk-where-observations.tsv'] for line in path.read_text().splitlines()]
    cases = []
    for case in ['shared-page', 'multi-page']:
        file = f'{case}-before.accdb'
        original_path = ROOT / 'files' / file
        before = original_path.read_bytes()
        owner = next(int(row[3]) for row in observations
                     if row[0] == 'table' and row[1] == file and row[2] == 'MSysQueries')
        reference, _ = selected(before, owner)
        original_records = records(before, owner)
        index_pages = [page for page in range(len(before) // PAGE)
                       if before[page * PAGE:page * PAGE + 2] in [b'\x03\x01', b'\x04\x01']]
        before_indexes = {(row[2], row[3]): row[4:] for row in observations
                          if row[0] == 'index' and row[1] == file}
        variants = []
        requests = [('grow', TARGET, 6, '42', '1000'), ('shrink', TARGET, 6, '42', '7')]
        if case == 'multi-page':
            requests.extend([
                ('where-grow', TARGET + 2, 8, '[p].[ID] >= 2', '[p].[ID] >= 1000'),
                ('where-shrink', TARGET + 2, 8, '[p].[ID] >= 2', '[p].[ID]>0'),
            ])
        for mode, object_id, attribute, original_expression, literal in requests:
            reference, _ = selected(before, owner, object_id, attribute, original_expression)
            after_path = ROOT / 'files' / f'{case}-{mode}.accdb'
            after = after_path.read_bytes()
            literal_after = expected(before, owner, literal, object_id, attribute, original_expression)
            literal_path = ROOT / 'files' / f'{case}-{mode}-literal.accdb'
            if literal_path.exists():
                assert literal_path.read_bytes() == literal_after
            else:
                literal_path.write_bytes(literal_after)
            differences = [index for index, (left, right) in enumerate(zip(before, after)) if left != right]
            mismatch = [index for index, (left, right) in enumerate(zip(literal_after, after)) if left != right]
            after_records = records(after, owner)
            other_records_unchanged = all(after_records[key]['raw'] == record['raw']
                                          and after_records[key]['word'] == record['word']
                                          for key, record in original_records.items() if key != reference)
            after_indexes = {(row[2], row[3]): row[4:] for row in observations
                             if row[0] == 'index' and row[1] == after_path.name}
            variants.append({'mode': mode, 'selected_native_id': object_id, 'selected_attribute': attribute,
                             'original_expression': original_expression, 'replacement_expression': literal,
                             'before_carrier': carrier(before, owner, reference), 'after_path': str(after_path),
                             'after_sha256': digest(after), 'after_bytes': len(after),
                             'literal_model_path': str(literal_path), 'literal_model_sha256': digest(literal_after),
                             'complete_model_matches': literal_after == after,
                             'model_differences': len(mismatch), 'first_model_differences': mismatch[:16],
                             'changed_bytes': len(differences), 'changed_pages': sorted({index // PAGE for index in differences}),
                             'carrier': carrier(after, owner, reference),
                             'unselected_raw_records_unchanged': other_records_unchanged,
                             'all_index_signature_pages_unchanged': all(before[page * PAGE:(page + 1) * PAGE] == after[page * PAGE:(page + 1) * PAGE] for page in index_pages),
                             'sdk_ordered_index_membership_unchanged': before_indexes == after_indexes})
        cases.append({'case': case, 'before_path': str(original_path), 'before_sha256': digest(before),
                      'before_bytes': len(before), 'query_count': 3 if case == 'multi-page' else 2,
                      'fragment_count': len(original_records), 'query_definition_page': owner,
                      'query_data_pages': sorted({page for page, _ in original_records}),
                      'index_signature_pages': index_pages,
                      'before_carrier': carrier(before, owner, selected(before, owner)[0]), 'variants': variants})
    report = {'schema': 1, 'date': '2026-11-08',
              'scope': 'Inert SDK metadata saves on fresh local2/3-query sources; not a TableCraft writer or Microsoft Access save comparison.',
              'recipe': {'path': str(ROOT / 'recipe/ExistingQuerySaveFixture.java'),
                         'sha256': digest((ROOT / 'recipe/ExistingQuerySaveFixture.java').read_bytes())},
              'model': {'path': str(ROOT / 'recipe/observe.py'),
                        'sha256': digest(Path(__file__).read_bytes())},
              'sdk': {'path': '.cache/tablecraft-fixtures/jackcess-5.0.4.jar',
                      'sha256': digest(Path('.cache/tablecraft-fixtures/jackcess-5.0.4.jar').read_bytes())},
              'sdk_stdout': {'path': str(ROOT / 'sdk-observations.tsv'),
                             'sha256': digest((ROOT / 'sdk-observations.tsv').read_bytes())},
              'sdk_where_stdout': {'path': str(ROOT / 'sdk-where-observations.tsv'),
                                   'sha256': digest((ROOT / 'sdk-where-observations.tsv').read_bytes())},
              'sdk_stderr': (ROOT / 'sdk-stderr.log').read_text(),
              'sdk_where_stderr': (ROOT / 'sdk-where-stderr.log').read_text(),
              'first_run': {'report': str(ROOT / 'first-run/observations.json'),
                            'sha256': digest((ROOT / 'first-run/observations.json').read_bytes())},
              'originals_unchanged_since_first_run': all(digest(Path(case['before_path']).read_bytes()) == case['before_sha256'] for case in json.loads((ROOT / 'first-run/observations.json').read_text())['cases']),
              'cases': cases}
    (ROOT / 'observations.json').write_text(json.dumps(report, indent=2) + '\n')
    print([(case['case'], [(variant['mode'], variant['complete_model_matches'], variant['model_differences']) for variant in case['variants']]) for case in cases])


if __name__ == '__main__':
    main()
