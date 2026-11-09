"""Freeze literal row INSERT/UPDATE/DELETE images after independent fitted renames."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

PAGE = 4096
DEFINITION = 7
DATA = 8
INDEX = 9
BITMAP = 27
ENTRIES = 480
KEY = bytes.fromhex('7f8000000100000800')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def first_number_row(original, value):
    result = bytearray(original)
    for at in [16, 20, 67]:
        struct.pack_into('<I', result, DEFINITION * PAGE + at, 1)
    record = struct.pack('<HiiB', 2, 1, value, 3)
    assert len(record) == 11
    page = bytearray(PAGE)
    page[:2] = bytes([1, 1])
    struct.pack_into('<I', page, 4, DEFINITION)
    struct.pack_into('<H', page, 12, 1)
    start = PAGE - len(record)
    struct.pack_into('<H', page, 14, start)
    struct.pack_into('<H', page, 2, start - 16)
    page[start:] = record
    result[DATA * PAGE:(DATA + 1) * PAGE] = page
    struct.pack_into('<H', result, INDEX * PAGE + 2, PAGE - ENTRIES - len(KEY))
    result[INDEX * PAGE + BITMAP + len(KEY) // 8] = 1 << (len(KEY) % 8)
    result[INDEX * PAGE + ENTRIES:INDEX * PAGE + ENTRIES + len(KEY)] = KEY
    return bytes(result)


def delete_only_row(original):
    result = bytearray(original)
    directory = DATA * PAGE + 14
    word = struct.unpack_from('<H', original, directory)[0]
    assert word == PAGE - 11
    assert original[INDEX * PAGE + ENTRIES:INDEX * PAGE + ENTRIES + len(KEY)] == KEY
    expected_bitmap = bytearray(ENTRIES - BITMAP)
    expected_bitmap[len(KEY) // 8] = 1 << (len(KEY) % 8)
    assert original[INDEX * PAGE + BITMAP:INDEX * PAGE + ENTRIES] == expected_bitmap
    struct.pack_into('<H', result, directory, word | 0xc000)
    struct.pack_into('<I', result, DEFINITION * PAGE + 16, 0)
    struct.pack_into('<H', result, INDEX * PAGE + 2, PAGE - ENTRIES)
    result[INDEX * PAGE + BITMAP:INDEX * PAGE + ENTRIES] = bytes(ENTRIES - BITMAP)
    assert result[DATA * PAGE + word:(DATA + 1) * PAGE] == original[DATA * PAGE + word:(DATA + 1) * PAGE]
    assert result[INDEX * PAGE + ENTRIES:(INDEX + 1) * PAGE] == original[INDEX * PAGE + ENTRIES:(INDEX + 1) * PAGE]
    return bytes(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fitting-output-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    frozen = json.loads((args.fitting_output_dir / 'predictions-before-observation.json').read_text())
    args.output_dir.mkdir(exist_ok=False)
    cases = []
    for name in ['TaxAmount', 'X']:
        selected = [item for item in frozen['cases'] if item['case'] == 'number-first'
                    and item['variant'] == 'coordinated' and item['new_name'] == name]
        assert len(selected) == 1
        source = args.fitting_output_dir / selected[0]['after']
        before = source.read_bytes()
        assert digest(before) == selected[0]['after_sha256']
        inserted = first_number_row(before, 0)
        updated = first_number_row(before, 42)
        deleted = delete_only_row(updated)
        images = [('renamed', before), ('inserted', inserted), ('updated', updated), ('deleted', deleted)]
        for stage, image in images:
            (args.output_dir / (name + '-' + stage + '.accdb')).write_bytes(image)
        cases.append({'new_name': name, 'images': {stage: digest(image) for stage, image in images},
                      'bytes': len(before), 'live_after_delete': 0, 'last_issued_after_delete': 1,
                      'unique_history_after_delete': 1, 'tombstoned_record_hex': deleted[DATA * PAGE + PAGE - 11:(DATA + 1) * PAGE].hex(),
                      'inactive_key_hex': deleted[INDEX * PAGE + ENTRIES:INDEX * PAGE + ENTRIES + len(KEY)].hex()})
        assert source.read_bytes() == before
    report = {'scope': 'Independent raw row composition, no production encoder/SDK/native writer output',
              'model_sha256': digest(Path(__file__).read_bytes()),
              'delete_reference_owner': 'tests/access/indexes/indexed_delete_originals/model.py',
              'delete_reference_sha256': digest(Path('tests/access/indexes/indexed_delete_originals/model.py').read_bytes()),
              'cases': cases}
    (args.output_dir / 'predictions.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(cases, indent=2))


if __name__ == '__main__':
    main()
