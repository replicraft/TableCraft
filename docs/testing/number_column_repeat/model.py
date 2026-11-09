"""Independent literal second-Number AFTER model; imports only prior raw oracle."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

import raw

PAGE = 4096
OFFSET_MASK = 0x1FFF
FORWARD_FLAG = 0x4000
HIDDEN_FLAG = 0x8000
CATALOG_PAGE = 4
DEFINITION_PAGE = 7
PROPERTY_PAGE = 10
CATALOG_ROOT = 7
DIRECTORY = 14


def digest(data):
    return hashlib.sha256(data).hexdigest()


def u16(data, at):
    return struct.unpack_from('<H', data, at)[0]


def u32(data, at):
    return struct.unpack_from('<I', data, at)[0]


def put16(data, at, value):
    struct.pack_into('<H', data, at, value)


def put32(data, at, value):
    struct.pack_into('<I', data, at, value)


def physical_record(image):
    page = image[CATALOG_PAGE * PAGE:(CATALOG_PAGE + 1) * PAGE]
    word = u16(page, DIRECTORY + CATALOG_ROOT * 2)
    start = word & OFFSET_MASK
    slot = CATALOG_ROOT
    if word & FORWARD_FLAG:
        reference = u32(page, start)
        assert reference >> 8 == CATALOG_PAGE
        slot = reference & 255
        word = u16(page, DIRECTORY + slot * 2)
        assert word & HIDDEN_FLAG and not word & FORWARD_FLAG
        start = word & OFFSET_MASK
    end = u16(page, DIRECTORY + (slot - 1) * 2) & OFFSET_MASK
    return slot, start, end, page[start:end]


def candidate(before, column, oracle):
    assert len(before) == 11 * PAGE
    slot, start, end, row = physical_record(before)
    footer = len(row) - 5
    property_start = u16(row, footer - 18)
    property_end = u16(row, footer - 20)
    assert property_end - property_start == 12
    descriptor = u32(row, property_start)
    reference = u32(row, property_start + 4)
    assert descriptor & 0xC0000000 == 0x40000000
    assert reference == PROPERTY_PAGE << 8
    assert u32(row, property_start + 8) == 0

    page = before[PROPERTY_PAGE * PAGE:(PROPERTY_PAGE + 1) * PAGE]
    assert page[:2] == b'\x01\x01' and page[4:8] == b'LVAL'
    assert u16(page, 12) == 1
    old_start = u16(page, DIRECTORY)
    body = page[old_start:]
    assert descriptor & 0x3FFFFFFF == len(body)
    assert body[:4] == b'MR2\0'
    names_end = 4 + u32(body, 4)
    at = 10
    dictionary = []
    while at < names_end:
        length = u16(body, at)
        dictionary.append(body[at + 2:at + 2 + length].decode('utf-16le'))
        at += 2 + length
    assert dictionary in [['DefaultValue', 'Required'], ['Required', 'DefaultValue']]
    new_body = body + oracle.number_group(column, True, dictionary)

    after = bytearray(before)
    after[DEFINITION_PAGE * PAGE:(DEFINITION_PAGE + 1) * PAGE] = oracle.append_definition(before, column)
    descriptor_at = CATALOG_PAGE * PAGE + start + property_start
    put32(after, descriptor_at, 0x40000000 | len(new_body))
    put32(after, descriptor_at + 4, (PROPERTY_PAGE << 8) | 1)

    new_start = old_start - len(new_body)
    assert new_start >= DIRECTORY + 4
    assert before[PROPERTY_PAGE * PAGE + new_start:PROPERTY_PAGE * PAGE + old_start] == bytes(len(new_body))
    page_at = PROPERTY_PAGE * PAGE
    after[page_at + new_start:page_at + old_start] = new_body
    put16(after, page_at + DIRECTORY + 2, new_start)
    put16(after, page_at + 12, 2)
    put16(after, page_at + 2, new_start - (DIRECTORY + 4))
    return after, {
        'catalog_root_slot': CATALOG_ROOT,
        'catalog_physical_slot': slot,
        'catalog_physical_span': [start, end],
        'descriptor_before_hex': row[property_start:property_end].hex(),
        'descriptor_after_hex': after[descriptor_at:descriptor_at + 12].hex(),
        'old_lval_row': 0,
        'new_lval_row': 1,
        'old_body_bytes': len(body),
        'new_body_bytes': len(new_body),
        'old_body_sha256': digest(body),
        'new_body_sha256': digest(new_body),
        'new_body_hex': new_body.hex(),
        'old_body_preserved_exact': after[page_at + old_start:page_at + PAGE] == body,
        'dictionary_order': dictionary,
        'lval_before_free_bytes': u16(page, 2),
        'lval_after_free_bytes': u16(after, page_at + 2),
        'lval_before_slots': 1,
        'lval_after_slots': 2,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sources-dir', type=Path, required=True)
    parser.add_argument('--sdk-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    root = args.output_dir
    root.mkdir(exist_ok=False)
    report = json.loads((args.sdk_dir / 'sdk-observations.json').read_text())
    records = []
    for observation in report['observations']:
        before = (args.sources_dir / Path(observation['source']).name).read_bytes()
        assert digest(before) == observation['before_sha256']
        expected, physical = candidate(before, observation['column'], raw)
        raw_path = root / (observation['case'] + '-independent-after.accdb')
        assert not raw_path.exists()
        raw_path.write_bytes(expected)
        actual = (args.sdk_dir / Path(observation['output']).name).read_bytes()
        differences = [index for index, (a, b) in enumerate(zip(expected, actual)) if a != b]
        changed_pages = [page for page in range(len(before) // PAGE)
                         if before[page * PAGE:(page + 1) * PAGE] != expected[page * PAGE:(page + 1) * PAGE]]
        record = dict(observation)
        record['source'] = Path(record['source']).name
        record['output'] = Path(record['output']).name
        record.update(physical)
        record.update({
            'independent_after': raw_path.name,
            'independent_after_sha256': digest(expected),
            'full_image_match': expected == actual,
            'first_differences': differences[:32],
            'changed_pages': changed_pages,
            'maps_page_3_unchanged': before[3 * PAGE:4 * PAGE] == expected[3 * PAGE:4 * PAGE],
            'index_page_9_unchanged': before[9 * PAGE:10 * PAGE] == expected[9 * PAGE:10 * PAGE],
            'all_other_pages_unchanged': all(before[p * PAGE:(p + 1) * PAGE] == expected[p * PAGE:(p + 1) * PAGE]
                                             for p in range(11) if p not in [4, 7, 10]),
        })
        records.append(record)
        print(observation['case'], expected == actual, digest(expected), differences[:8])
    result = {
        'schema': 1,
        'scope': 'Independent repeated Number ADD full-image model vs unchanged first-party SDK helper; no engine/native Access execution',
        'model': 'docs/testing/number_column_repeat/model.py',
        'model_sha256': digest(Path(__file__).read_bytes()),
        'raw_helper': {'path': 'docs/testing/number_column_repeat/raw.py', 'sha256': digest(Path(raw.__file__).read_bytes()), 'production_codecs': False},
        'observations': records,
    }
    (root / 'independent-comparison.json').write_text(json.dumps(result, indent=2) + '\n')
    assert all(record['full_image_match'] for record in records)


if __name__ == '__main__':
    main()
