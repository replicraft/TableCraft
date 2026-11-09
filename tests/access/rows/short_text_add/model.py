"""Complete independent Short Text AFTER model; no production or SDK encoder."""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import struct

RAW_SOURCE = Path('docs/testing/number_column_repeat/raw.py')
PAGE = 4096
CATALOG_PAGE = 4
DEFINITION_PAGE = 7
PROPERTY_PAGE = 10
DIRECTORY = 14
ROOT_SLOT = 7
OFFSET_MASK = 0x1FFF


def digest(data):
    return hashlib.sha256(data).hexdigest()


def u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]


def u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]


def put16(data, offset, value):
    struct.pack_into('<H', data, offset, value)


def put32(data, offset, value):
    struct.pack_into('<I', data, offset, value)


def prior_raw():
    spec = importlib.util.spec_from_file_location('owned_number_raw', RAW_SOURCE)
    raw = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(raw)
    return raw


def catalog_record(before):
    page = before[CATALOG_PAGE * PAGE:(CATALOG_PAGE + 1) * PAGE]
    word = u16(page, DIRECTORY + ROOT_SLOT * 2)
    slot = ROOT_SLOT
    if word & 0x4000:
        root_start = word & OFFSET_MASK
        reference = u32(page, root_start)
        assert reference >> 8 == CATALOG_PAGE
        slot = reference & 255
    start = u16(page, DIRECTORY + slot * 2) & OFFSET_MASK
    end = u16(page, DIRECTORY + (slot - 1) * 2) & OFFSET_MASK
    row = page[start:end]
    footer = len(row) - 5
    begin = u16(row, footer - 18)
    finish = u16(row, footer - 20)
    return page, slot, start, end, row, begin, finish


def body_before(before, row, begin, finish):
    if begin == finish:
        return b''
    descriptor = row[begin:finish]
    tagged = u32(descriptor, 0)
    length = tagged & 0x3FFFFFFF
    if tagged & 0xC0000000 == 0x80000000:
        assert len(descriptor) == 12 + length
        return descriptor[12:]
    assert tagged & 0xC0000000 == 0x40000000
    reference = u32(descriptor, 4)
    assert reference >> 8 == PROPERTY_PAGE
    page = before[PROPERTY_PAGE * PAGE:(PROPERTY_PAGE + 1) * PAGE]
    selected = reference & 255
    assert selected == u16(page, 12) - 1
    start = u16(page, DIRECTORY + selected * 2)
    end = PAGE if selected == 0 else u16(page, DIRECTORY + (selected - 1) * 2)
    assert end - start == length
    return page[start:end]


def dictionary(body):
    if not body:
        return [], b''
    assert body[:4] == b'MR2\0'
    end = 4 + u32(body, 4)
    names = []
    position = 10
    while position < end:
        length = u16(body, position)
        names.append(body[position + 2:position + 2 + length].decode('utf-16le'))
        position += 2 + length
    assert position == end
    return names, body[end:]


def text_body(original, name):
    names, groups = dictionary(original)
    for key in ['Required', 'AllowZeroLength']:
        if key not in names:
            names.append(key)
    entries = b''
    for key in names:
        if key in ['Required', 'AllowZeroLength']:
            value = 0 if key == 'Required' else 255
            entries += struct.pack('<HBBHHB', 9, 1, 1, names.index(key), 1, value)
    label = name.encode('utf-16le')
    group = struct.pack('<IHIH', 12 + len(label) + len(entries), 1,
                        6 + len(label), len(label)) + label + entries
    encoded_names = b''.join(struct.pack('<H', len(key.encode('utf-16le')))
                             + key.encode('utf-16le') for key in names)
    return b'MR2\0' + struct.pack('<IH', 6 + len(encoded_names), 128) + encoded_names + groups + group


def parent_page(before, body, reference):
    page, slot, start, end, original, begin, finish = catalog_record(before)
    descriptor = struct.pack('<III', 0x40000000 | len(body), reference, 0)
    length = max(len(original), len(original) - (finish - begin) + len(descriptor))
    row = bytearray(length)
    row[:begin] = original[:begin]
    row[begin:begin + len(descriptor)] = descriptor
    footer = original[finish:]
    row[length - len(footer):] = footer
    count = length - 5
    for index in range(9, 12):
        put16(row, count - 2 * (index + 1), begin + len(descriptor))
    row[-2] |= 0x40
    result = bytearray(page)
    if len(row) == len(original):
        result[start:end] = row
    else:
        assert slot == ROOT_SLOT and u16(page, 12) == 8
        target = start - len(row)
        result[target:start] = row
        result[start:end] = bytes(end - start)
        put32(result, start, (CATALOG_PAGE << 8) | 8)
        put16(result, DIRECTORY + ROOT_SLOT * 2, start | 0x4000)
        put16(result, DIRECTORY + 8 * 2, target | 0x8000)
        put16(result, 12, 9)
        put16(result, 2, target - (DIRECTORY + 9 * 2))
    return result


def append_body(before, after, body):
    if len(before) == PROPERTY_PAGE * PAGE:
        page = bytearray(PAGE)
        page[:2] = b'\x01\x01'
        page[4:8] = b'LVAL'
        count = 0
        end = PAGE
        for slot in [16, 17]:
            maps = 3 * PAGE
            record = maps + u16(after, maps + DIRECTORY + 2 * slot)
            assert after[record:record + 13] == bytes(13)
            after[record + 6] |= 4
        after.extend(page)
    else:
        assert len(before) == (PROPERTY_PAGE + 1) * PAGE
        page = bytearray(before[PROPERTY_PAGE * PAGE:(PROPERTY_PAGE + 1) * PAGE])
        count = u16(page, 12)
        end = u16(page, DIRECTORY + 2 * (count - 1))
    start = end - len(body)
    assert start >= DIRECTORY + 2 * (count + 1)
    assert page[start:end] == bytes(len(body))
    page[start:end] = body
    put16(page, DIRECTORY + 2 * count, start)
    put16(page, 12, count + 1)
    put16(page, 2, start - (DIRECTORY + 2 * (count + 1)))
    after[PROPERTY_PAGE * PAGE:(PROPERTY_PAGE + 1) * PAGE] = page
    return (PROPERTY_PAGE << 8) | count


def short_text(before, name, units, compressed, raw):
    after = bytearray(before)
    original = before[DEFINITION_PAGE * PAGE:(DEFINITION_PAGE + 1) * PAGE]
    count = u16(original, 45)
    definition = raw.append_definition(before, name)
    field = 75 + 25 * count
    definition[field] = 10
    definition[field + 11:field + 15] = bytes([9, 4, 0, 1])
    definition[field + 15] = 2
    definition[field + 16] = 1 if compressed else 0
    put16(definition, field + 21, 0)
    put16(definition, field + 23, units * 2)
    put16(definition, 43, u16(original, 43) + 1)
    after[DEFINITION_PAGE * PAGE:(DEFINITION_PAGE + 1) * PAGE] = definition
    _, _, _, _, row, begin, finish = catalog_record(before)
    body = text_body(body_before(before, row, begin, finish), name)
    reference = append_body(before, after, body)
    after[CATALOG_PAGE * PAGE:(CATALOG_PAGE + 1) * PAGE] = parent_page(before, body, reference)
    return after


def number_again(before, name, raw):
    after = bytearray(before)
    after[DEFINITION_PAGE * PAGE:(DEFINITION_PAGE + 1) * PAGE] = raw.append_definition(before, name)
    _, _, _, _, row, begin, finish = catalog_record(before)
    original = body_before(before, row, begin, finish)
    names, groups = dictionary(original)
    if 'DefaultValue' not in names:
        names.append('DefaultValue')
    encoded_names = b''.join(struct.pack('<H', len(key.encode('utf-16le')))
                             + key.encode('utf-16le') for key in names)
    body = (b'MR2\0' + struct.pack('<IH', 6 + len(encoded_names), 128)
            + encoded_names + groups + raw.number_group(name, True, names))
    reference = append_body(before, after, body)
    after[CATALOG_PAGE * PAGE:(CATALOG_PAGE + 1) * PAGE] = parent_page(before, body, reference)
    return after


def definition_observation(data):
    page = data[7 * PAGE:8 * PAGE]
    count = u16(page, 45)
    position = 75 + count * 25
    for _ in range(count):
        position += 2 + u16(page, position)
    length = u32(page, 8)
    return {'current_fields': count, 'maximum_fields': u16(page, 41),
            'variable_fields': u16(page, 43), 'definition_bytes': length,
            'index_tail_sha256': digest(page[position:length])}


def parent(data):
    page = data[4 * PAGE:5 * PAGE]
    root_word = u16(page, 28)
    root_start = root_word & 0x1FFF
    slot = u32(page, root_start) & 255 if root_word & 0x4000 else 7
    start = u16(page, 14 + slot * 2) & 0x1FFF
    end = u16(page, 14 + (slot - 1) * 2) & 0x1FFF
    row = page[start:end]
    begin = u16(row, len(row) - 5 - 18)
    finish = u16(row, len(row) - 5 - 20)
    descriptor = row[begin:finish]
    return {'logical_root_word': hex(root_word), 'physical_slot': slot,
            'physical_span': [start, end], 'slots': u16(page, 12),
            'free_bytes': u16(page, 2), 'property_descriptor_hex': descriptor.hex()}



def main():
    raw = prior_raw()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--sdk-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--report-name', default='sdk-observations.json')
    args = parser.parse_args()
    root = args.output_dir
    root.mkdir(exist_ok=False)
    report = json.loads((args.sdk_dir / args.report_name).read_text())
    records = []
    number_source = next(case for case in report['observations'] if case['case'] == 'two-number-uncompressed')
    original = (args.sources_dir / Path(number_source['source']).name).read_bytes()
    expected_third = number_again(original, 'Third', raw)
    third = (args.sdk_dir / 'files/third-number-sdk-after.accdb').read_bytes()
    third_match = expected_third == third
    print('third-number independent full image', third_match, digest(expected_third))
    for case in report['observations']:
        source_directory = args.sdk_dir / 'files' if case.get('source_location') == 'sdk_files' or case['case'].startswith('three-number') else args.sources_dir
        before = (source_directory / Path(case['source']).name).read_bytes()
        assert digest(before) == case['source_sha256']
        declaration = case['declaration']
        if declaration['type'] == 'LONG':
            expected = number_again(before, case['column'], raw)
        else:
            expected = short_text(before, case.get('column', 'Label'), declaration['units'], declaration['unicode_compression'], raw)
        actual = (args.sdk_dir / 'files' / Path(case['output']).name).read_bytes()
        differences = [index for index, (a, b) in enumerate(zip(expected, actual)) if a != b]
        case['before_definition'] = definition_observation(before)
        case['after_definition'] = definition_observation(actual)
        case['index_definition_tail_unchanged'] = case['before_definition']['index_tail_sha256'] == case['after_definition']['index_tail_sha256']
        case['before_catalog'] = parent(before)
        case['after_catalog'] = parent(actual)
        case['changed_pages'] = [number for number in range(len(actual) // PAGE)
                                 if before[number * PAGE:(number + 1) * PAGE] != actual[number * PAGE:(number + 1) * PAGE]]
        count = u16(actual, DEFINITION_PAGE * PAGE + 45)
        field_at = DEFINITION_PAGE * PAGE + 75 + 25 * (count - 1)
        case['field_hex'] = actual[field_at:field_at + 25].hex()
        _, _, _, _, actual_row, actual_begin, actual_end = catalog_record(actual)
        actual_body = body_before(actual, actual_row, actual_begin, actual_end)
        case['body_hex'] = actual_body.hex()
        case['dictionary'] = dictionary(actual_body)[0]
        case['old_property_rows'] = []
        if len(before) == (PROPERTY_PAGE + 1) * PAGE:
            old_page = before[PROPERTY_PAGE * PAGE:(PROPERTY_PAGE + 1) * PAGE]
            new_page = actual[PROPERTY_PAGE * PAGE:(PROPERTY_PAGE + 1) * PAGE]
            for slot in range(u16(old_page, 12)):
                start = u16(old_page, DIRECTORY + slot * 2)
                end = PAGE if slot == 0 else u16(old_page, DIRECTORY + (slot - 1) * 2)
                unchanged = old_page[start:end] == new_page[start:end] and old_page[DIRECTORY + slot * 2:DIRECTORY + slot * 2 + 2] == new_page[DIRECTORY + slot * 2:DIRECTORY + slot * 2 + 2]
                case['old_property_rows'].append({'slot': slot, 'span': [start, end], 'sha256': digest(old_page[start:end]), 'unchanged': unchanged})
        case['source'] = Path(case['source']).name
        case['output'] = Path(case['output']).name
        case.update(independent_sha256=digest(expected), complete_match=expected == actual,
                    first_differences=differences[:32])
        (root / (case['case'] + '-independent-after.accdb')).write_bytes(expected)
        print(case['case'], expected == actual, digest(expected), differences[:8])
        records.append(case)
    report.update(model='docs/testing/short_text_add/model.py', model_sha256=digest(Path(__file__).read_bytes()),
                  raw_helper=str(RAW_SOURCE), raw_helper_sha256=digest(RAW_SOURCE.read_bytes()),
                  third_independent_complete_match=third_match,
                  third_independent_sha256=digest(expected_third), observations=records)
    (root / 'independent-comparison.json').write_text(json.dumps(report, indent=2) + '\n')
    assert third_match and all(case['complete_match'] for case in records)


if __name__ == '__main__':
    main()
