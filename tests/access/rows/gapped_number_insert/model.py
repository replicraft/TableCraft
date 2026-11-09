"""Independent literal first-row hypothesis for the pinned empty DROP layout.

No production codec or writer supplies bytes. This remains an unobserved raw
prediction until an independent reader and the native candidate verify it.
"""
import argparse
import hashlib
import importlib.util
import json
import struct
from pathlib import Path

PAGE = 4096
SOURCE_SHA256 = '6c08831dfe4d305064837d93a8d1942a1a87b691ca80e4834a130c202c72923b'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def predict(before, repo):
    assert digest(before) == SOURCE_SHA256
    spec = importlib.util.spec_from_file_location('drop_raw', repo / 'tests/access/schema/saved_field_drop/model.py')
    raw = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(raw)
    definition = before[7 * PAGE:8 * PAGE]
    fields = raw.declarations(definition)[2]
    assert [(name, raw.u16(entry, 5), raw.u16(entry, 21)) for entry, _, name in fields] == [
        ('ID', 0, 0), ('Score2', 2, 8),
    ]
    assert raw.u16(definition, 41) == 3
    assert raw.u16(definition, 45) == 2
    assert raw.u32(definition, 16) == raw.u32(definition, 20) == raw.u32(definition, 67) == 0
    assert raw.u16(before, 8 * PAGE + 12) == 0
    assert before[9 * PAGE:10 * PAGE][480:] == bytes(PAGE - 480)
    after = bytearray(before)

    # Logical field count retains high-water 3; native bit1 and its fixed four
    # bytes are absent. ID1 owns native bit0, default-zero Score2 owns bit2.
    record = bytes([3, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5])
    data = bytearray(PAGE)
    data[:2] = bytes([1, 1])
    struct.pack_into('<H', data, 2, PAGE - 16 - len(record))
    struct.pack_into('<I', data, 4, 7)
    struct.pack_into('<H', data, 12, 1)
    struct.pack_into('<H', data, 14, PAGE - len(record))
    data[-len(record):] = record
    after[8 * PAGE:9 * PAGE] = data

    # Exactly the maintained independent initial-table first-row changes:
    # membership, last issued ID, physical unique history; one ASC ID key.
    for offset in [16, 20, 67]:
        struct.pack_into('<I', after, 7 * PAGE + offset, 1)
    struct.pack_into('<H', after, 9 * PAGE + 2, 3607)
    after[9 * PAGE + 28] = 2
    after[9 * PAGE + 480:9 * PAGE + 489] = bytes([127, 128, 0, 0, 1, 0, 0, 8, 0])
    changed = [page for page in range(len(before) // PAGE)
               if before[page * PAGE:(page + 1) * PAGE] != after[page * PAGE:(page + 1) * PAGE]]
    assert changed == [7, 8, 9]
    assert after[10 * PAGE:] == before[10 * PAGE:]
    return bytes(after), {
        'before_sha256': digest(before),
        'after_sha256': digest(after),
        'changed_pages': changed,
        'record_hex': record.hex(),
        'logical_row': {'ID': 1, 'Score2': 0},
        'native_numbers': [0, 2],
        'column_high_water': 3,
        'null_mask': 5,
        'status': 'raw prediction only; no SDK/native insertion observed',
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    arguments = parser.parse_args()
    assert not arguments.output.exists()
    before = arguments.source.read_bytes()
    after, record = predict(before, arguments.repo)
    arguments.output.mkdir(parents=True)
    (arguments.output / 'number-gapped-first-row.accdb').write_bytes(after)
    record['model_sha256'] = digest(Path(__file__).read_bytes())
    (arguments.output / 'prediction.json').write_text(json.dumps(record, indent=2) + '\n')
    assert digest(arguments.source.read_bytes()) == digest(before)
    print(json.dumps(record, indent=2))


if __name__ == '__main__':
    main()
