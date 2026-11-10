"""Freeze metadata-only ADD over preserved historical two-field records."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

import model
from engineering_paths import repository_root

PAGE = 4096
COUNTERS = [16, 20, 67]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def append_row(before, score, bonus):
    """Append a literal three-field record without touching older row bodies."""
    count = struct.unpack_from('<H', before, 8 * PAGE + 12)[0]
    end = struct.unpack_from('<H', before, 8 * PAGE + 14 + 2 * (count - 1))[0]
    record = struct.pack('<HiiiB', 3, count + 1,
                         0 if score is None else score,
                         0 if bonus is None else bonus,
                         1 | (2 if score is not None else 0) | (4 if bonus is not None else 0))
    assert len(record) == 15
    result = bytearray(before)
    start = end - len(record)
    result[8 * PAGE + start:8 * PAGE + end] = record
    struct.pack_into('<H', result, 8 * PAGE + 14 + 2 * count, start)
    struct.pack_into('<H', result, 8 * PAGE + 12, count + 1)
    struct.pack_into('<H', result, 8 * PAGE + 2, start - 14 - 2 * (count + 1))
    for at in COUNTERS:
        struct.pack_into('<I', result, 7 * PAGE + at, count + 1)
    key = bytes([127]) + (0x80000000 + count + 1).to_bytes(4, 'big')
    key += ((8 << 8) | count).to_bytes(4, 'big')
    result[9 * PAGE + 480 + 9 * count:9 * PAGE + 489 + 9 * count] = key
    entry_end = 9 * (count + 1)
    result[9 * PAGE + 27 + entry_end // 8] |= 1 << (entry_end % 8)
    struct.pack_into('<H', result, 9 * PAGE + 2, 3616 - entry_end)
    assert result[8 * PAGE + end:9 * PAGE] == before[8 * PAGE + end:9 * PAGE]
    return bytes(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--empty-before', type=Path, required=True)
    parser.add_argument('--populated-before', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    empty = args.empty_before.read_bytes()
    before = args.populated_before.read_bytes()
    assert digest(empty) == 'c88bc4cb1090d9b0c5e011010ec76cf9acc33da7ec9d58e67fa873bfaabe18ef'
    assert digest(before) == '8e85e6456b412204afb1c21532209108f4300d25ad55151900fdd52c4c006569'
    hypothesis = bytearray(before)
    for at in COUNTERS:
        assert struct.unpack_from('<I', before, 7 * PAGE + at)[0] == 3
        struct.pack_into('<I', hypothesis, 7 * PAGE + at, 0)
    hypothesis[8 * PAGE:10 * PAGE] = empty[8 * PAGE:10 * PAGE]
    assert hypothesis == empty
    after = bytearray(model.prediction(empty, 'LONG', 'Bonus', repository_root()))
    for at in COUNTERS:
        after[7 * PAGE + at:7 * PAGE + at + 4] = before[7 * PAGE + at:7 * PAGE + at + 4]
    after[8 * PAGE:10 * PAGE] = before[8 * PAGE:10 * PAGE]
    assert all(struct.unpack_from('<H', after, 8 * PAGE + 14 + slot * 2)[0] == 4096 - 11 * (slot + 1)
               for slot in range(3))
    args.output_dir.mkdir(exist_ok=False)
    (args.output_dir / 'before.accdb').write_bytes(before)
    (args.output_dir / 'after.accdb').write_bytes(after)
    default_row = append_row(after, 0, 0)
    null_row = append_row(default_row, 0, None)
    (args.output_dir / 'default-row.accdb').write_bytes(default_row)
    (args.output_dir / 'null-row.accdb').write_bytes(null_row)
    report = {'default_row_sha256': digest(default_row), 'null_row_sha256': digest(null_row),
              'scope': 'Independent raw metadata ADD, no native writer/reader execution',
              'before_sha256': digest(before), 'after_sha256': digest(after),
              'historical_record_fields': 2, 'new_declared_fields': 3,
              'data_index_unchanged': True, 'rows_last_unique': [3, 3, 3],
              'expected_existing_rows': [[1, 42, None], [2, None, None], [3, -7, None]],
              'stored_future_default': '0'}
    (args.output_dir / 'predictions.json').write_text(json.dumps(report, indent=2) + '\n')
    assert args.empty_before.read_bytes() == empty
    assert args.populated_before.read_bytes() == before
    print(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
