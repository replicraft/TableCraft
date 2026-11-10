"""Freeze independently commuting populated-row and field-name transformations."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

import fitting
import model
from prepare_rows import first_number_row

PAGE = 4096
SOURCE_SHA256 = 'c88bc4cb1090d9b0c5e011010ec76cf9acc33da7ec9d58e67fa873bfaabe18ef'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def long_rows(original, values):
    assert 1 <= len(values) <= 255
    result = bytearray(first_number_row(original, 0 if values[0] is None else values[0]))
    if values[0] is None:
        result[9 * PAGE - 1] = 1
    start = PAGE - 11
    for position, value in enumerate(values[1:], 1):
        start -= 11
        record = struct.pack('<HiiB', 2, position + 1, 0 if value is None else value,
                             1 if value is None else 3)
        result[8 * PAGE + start:8 * PAGE + start + 11] = record
        struct.pack_into('<H', result, 8 * PAGE + 14 + 2 * position, start)
        key_start = 9 * PAGE + 480 + 9 * position
        key = bytes([127]) + ((position + 1) ^ 0x80000000).to_bytes(4, 'big')
        key += ((8 << 8) | position).to_bytes(4, 'big')
        result[key_start:key_start + 9] = key
        end = 9 * (position + 1)
        result[9 * PAGE + 27 + end // 8] |= 1 << (end % 8)
    count = len(values)
    struct.pack_into('<H', result, 8 * PAGE + 12, count)
    struct.pack_into('<H', result, 8 * PAGE + 2, start - 14 - 2 * count)
    struct.pack_into('<H', result, 9 * PAGE + 2, 3616 - 9 * count)
    for offset in [16, 20, 67]:
        struct.pack_into('<I', result, 7 * PAGE + offset, count)
    return bytes(result)


def rename(before, name):
    if len(name) == len('Score'):
        return model.rename(before, 'Score', name, True)[0]
    return fitting.rename(before, 'Score', name)[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--number-first-off', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    original = args.number_first_off.read_bytes()
    assert digest(original) == SOURCE_SHA256
    args.output_dir.mkdir(exist_ok=False)
    cases = []
    for label, values in [('one', [42]), ('mixed', [42, None, -7]), ('null-first', [None, 42])]:
        before = long_rows(original, values)
        (args.output_dir / (label + '-before.accdb')).write_bytes(before)
        for name in ['TaxAmount', 'X', 'Total', 'Score']:
            after = rename(before, name)
            reverse = long_rows(rename(original, name), values)
            assert after == reverse
            assert before[8 * PAGE:10 * PAGE] == after[8 * PAGE:10 * PAGE]
            assert [fitting.u32(after, 7 * PAGE + at) for at in [16, 20, 67]] == [len(values)] * 3
            (args.output_dir / (label + '-' + name + '.accdb')).write_bytes(after)
            cases.append({'case': label, 'values': values, 'new_name': name,
                          'before_sha256': digest(before), 'after_sha256': digest(after),
                          'bytes': len(before), 'row_name_operations_commute': True,
                          'active_data_index_unchanged': True, 'rows_last_unique': [len(values)] * 3})
    report = {'scope': 'Independent populated field rename images, no native writer/SDK execution',
              'source_sha256': SOURCE_SHA256,
              'model_pins': {path.name: digest(path.read_bytes()) for path in [
                  Path(__file__), Path(fitting.__file__), Path(model.__file__),
                  Path(__file__).with_name('prepare_rows.py')]},
              'cases': cases}
    (args.output_dir / 'predictions.json').write_text(json.dumps(report, indent=2) + '\n')
    assert args.number_first_off.read_bytes() == original
    print(json.dumps(cases, indent=2))


if __name__ == '__main__':
    main()
