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
    report = {'scope': 'Independent raw metadata ADD, no native writer/reader execution',
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
