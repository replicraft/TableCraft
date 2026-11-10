"""Pin literal original-slot deletion after populated rename, update and append."""
from pathlib import Path
import argparse
import hashlib
import json
import struct

from prepare_populated import SOURCE_SHA256, long_rows, rename

PAGE = 4096


def digest(data):
    return hashlib.sha256(data).hexdigest()


def delete_second_of_four(before):
    """Original indexed-delete model: compact active keys, retain inactive tail."""
    assert len(before) == 11 * PAGE
    assert struct.unpack_from('<II', before, 7 * PAGE + 16) == (4, 4)
    assert struct.unpack_from('<I', before, 7 * PAGE + 67)[0] == 4
    keys = [before[9 * PAGE + 480 + 9 * slot:9 * PAGE + 489 + 9 * slot]
            for slot in range(4)]
    assert keys == [bytes([127]) + (0x80000000 + slot + 1).to_bytes(4, 'big')
                    + ((8 << 8) | slot).to_bytes(4, 'big') for slot in range(4)]
    after = bytearray(before)
    struct.pack_into('<I', after, 7 * PAGE + 16, 3)
    directory = 8 * PAGE + 16
    original = struct.unpack_from('<H', before, directory)[0]
    assert original == PAGE - 22
    struct.pack_into('<H', after, directory, original | 0xc000)
    struct.pack_into('<H', after, 9 * PAGE + 2, 3616 - 27)
    bitmap = bytearray(453)
    for end in [9, 18, 27]:
        bitmap[end // 8] |= 1 << (end % 8)
    after[9 * PAGE + 27:9 * PAGE + 480] = bitmap
    after[9 * PAGE + 480:9 * PAGE + 507] = keys[0] + keys[2] + keys[3]
    assert after[9 * PAGE + 507:10 * PAGE] == before[9 * PAGE + 507:10 * PAGE]
    return bytes(after)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--number-first-off', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    source = args.number_first_off.read_bytes()
    assert digest(source) == SOURCE_SHA256
    args.output_dir.mkdir(exist_ok=False)
    cases = []
    for name in ['TaxAmount', 'X']:
        before = long_rows(rename(source, name), [99, None, -7, 0])
        after = delete_second_of_four(before)
        (args.output_dir / (name + '-before.accdb')).write_bytes(before)
        (args.output_dir / (name + '-after.accdb')).write_bytes(after)
        cases.append({'name': name, 'before_sha256': digest(before),
                      'after_sha256': digest(after), 'deleted_original_slot': 1,
                      'retained_ids': [1, 3, 4], 'rows_last_unique': [3, 4, 4]})
    report = {'scope': 'Independent raw model, no native writer or SDK execution',
              'source_sha256': SOURCE_SHA256, 'cases': cases}
    (args.output_dir / 'predictions.json').write_text(json.dumps(report, indent=2) + '\n')
    assert args.number_first_off.read_bytes() == source
    print(json.dumps(cases, indent=2))


if __name__ == '__main__':
    main()
