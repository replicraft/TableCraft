"""Independent complete images for the observed ID-only Number history/rollover."""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json
import struct
import sys

PAGE = 4096
SOURCE_HASH = 'a6a9e18cd35dc2bf018e5cd554a68b5b60d56023fd78a4a0ecba627ab9041d72'


def repository_root():
    for parent in Path(__file__).resolve().parents:
        if (parent / 'docs/testing/number_column_repeat/raw.py').is_file():
            return parent
    raise RuntimeError('Run this maintained helper inside the TableCraft source tree')


ROOT = repository_root()
OWNER = ROOT / 'docs/testing/number_column_repeat'
sys.path.insert(0, str(OWNER))
spec = importlib.util.spec_from_file_location('number_repeat_model', OWNER / 'model.py')
prior_model = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prior_model)
raw = prior_model.raw


def digest(data):
    return hashlib.sha256(data).hexdigest()



def u16(data, at):
    return struct.unpack_from('<H', data, at)[0]



def u32(data, at):
    return struct.unpack_from('<I', data, at)[0]



def rows(page):
    result = []
    end = PAGE
    for slot in range(u16(page, 12)):
        word = u16(page, 14 + 2 * slot)
        start = word & 0x1fff
        result.append({'slot': slot, 'word': word, 'start': start, 'end': end,
                       'bytes': end - start, 'sha256': digest(page[start:end]),
                       'hex': page[start:end].hex()})
        end = start
    return result



def descriptor(image):
    slot, start, end, record = prior_model.physical_record(image)
    footer = len(record) - 5
    at = u16(record, footer - 18)
    stop = u16(record, footer - 20)
    assert stop - at == 12
    framed = record[at:stop]
    return {'slot': slot, 'start': start, 'end': end,
            'absolute_at': 4 * PAGE + start + at, 'hex': framed.hex(),
            'body_length': u32(framed, 0) & 0x3fffffff,
            'page': u32(framed, 4) >> 8, 'row': u32(framed, 4) & 255}



def append_same_page(before, name):
    parent = descriptor(before)
    assert parent['page'] == 10
    page = before[10 * PAGE:11 * PAGE]
    history = rows(page)
    assert parent['row'] == len(history) - 1
    current = history[-1]
    body = bytes.fromhex(current['hex'])
    assert len(body) == parent['body_length']
    assert body[:4] == b'MR2\0'
    names_end = 4 + u32(body, 4)
    at, dictionary = 10, []
    while at < names_end:
        length = u16(body, at)
        dictionary.append(body[at + 2:at + 2 + length].decode('utf-16le'))
        at += 2 + length
    assert dictionary == ['DefaultValue', 'Required']
    body += raw.number_group(name, True, dictionary)
    start = current['start'] - len(body)
    count = len(history)
    assert start >= 14 + 2 * (count + 1)
    after = bytearray(before)
    after[7 * PAGE:8 * PAGE] = raw.append_definition(before, name)
    descriptor_at = parent['absolute_at']
    struct.pack_into('<II', after, descriptor_at, 0x40000000 | len(body), (10 << 8) | count)
    assert before[10 * PAGE + start:10 * PAGE + current['start']] == bytes(len(body))
    after[10 * PAGE + start:10 * PAGE + current['start']] = body
    struct.pack_into('<H', after, 10 * PAGE + 14 + 2 * count, start)
    struct.pack_into('<H', after, 10 * PAGE + 12, count + 1)
    struct.pack_into('<H', after, 10 * PAGE + 2, start - (14 + 2 * (count + 1)))
    return bytes(after)



def row(page, slot):
    start = u16(page, 14 + 2 * slot)
    assert start & 0xe000 == 0
    end = PAGE if slot == 0 else u16(page, 14 + 2 * (slot - 1))
    return start, end, page[start:end]



def bits(record):
    assert len(record) == 13 and record[0] == 0 and u32(record, 1) == 0
    return [bit for bit in range(64) if record[5 + bit // 8] & (1 << (bit % 8))]



def rollover(before, name):
    assert len(before) == 11 * PAGE
    slot, start, end, parent = prior_model.physical_record(before)
    assert slot == 8
    at = u16(parent, len(parent) - 23)
    descriptor = parent[at:at + 12]
    assert u32(descriptor, 0) & 0xc0000000 == 0x40000000
    reference = u32(descriptor, 4)
    assert reference == (10 << 8) | 12
    old_page = before[10 * PAGE:11 * PAGE]
    old_start, old_end, body = row(old_page, 12)
    assert len(body) == u32(descriptor, 0) & 0x3fffffff == 523
    assert u16(old_page, 2) == 71
    assert body[:4] == b'MR2\0'
    body += raw.number_group(name, True, ['DefaultValue', 'Required'])
    assert len(body) == 560 and len(body) + 2 > u16(old_page, 2)

    # Literal native17 MSysObjects trailing declaration: physical field14 LvProp
    # owns map3:16 and candidate map3:17. Binding is checked in BEFORE, not inferred
    # from changes or copied out of the SDK AFTER.
    catalog_definition = before[2 * PAGE:3 * PAGE]
    assert catalog_definition[802:812] == bytes.fromhex('0e001003000011030000')
    maps = before[3 * PAGE:4 * PAGE]
    owned_start, _, owned = row(maps, 16)
    candidate_start, _, candidate = row(maps, 17)
    assert bits(owned) == [10] and bits(candidate) == [10]
    assert bits(row(maps, 1)[2]) == [], 'global-free map already marks this represented EOF position allocated'

    after = bytearray(before)
    after[7 * PAGE:8 * PAGE] = raw.append_definition(before, name)
    descriptor_at = 4 * PAGE + start + at
    struct.pack_into('<II', after, descriptor_at, 0x40000000 | len(body), (11 << 8))
    # Source-owned page10 has no room. The observed policy retains it and places
    # the next single MR2 body on the immediate EOF page, with no page retirement.
    new_page = bytearray(PAGE)
    new_page[:2] = b'\x01\x01'
    struct.pack_into('<H', new_page, 2, PAGE - 16 - len(body))
    new_page[4:8] = b'LVAL'
    struct.pack_into('<H', new_page, 12, 1)
    struct.pack_into('<H', new_page, 14, PAGE - len(body))
    new_page[PAGE - len(body):] = body
    after.extend(new_page)
    # Map polarity is role-specific: owned includes both pages; candidate drops
    # the full old page and includes the new page. Global-free bytes stay exact.
    after[3 * PAGE + owned_start + 6] = 0x0c
    after[3 * PAGE + candidate_start + 6] = 0x08
    return bytes(after)



def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk-dir', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    args = parser.parse_args()
    args.output_dir.mkdir(exist_ok=False)
    source = args.sdk_dir / 'score-pinned-before.accdb'
    assert digest(source.read_bytes()) == SOURCE_HASH
    observations = []
    for number in range(2, 15):
        name = f'N{number}'
        before = source.read_bytes()
        expected = append_same_page(before, name) if number <= 13 else rollover(before, name)
        actual_path = args.sdk_dir / f'n{number}-sdk-after.accdb'
        actual = actual_path.read_bytes()
        output = args.output_dir / f'n{number}-independent-after.accdb'
        output.write_bytes(expected)
        differences = [at for at, (a, b) in enumerate(zip(expected, actual)) if a != b]
        observations.append({
            'column': name, 'before_sha256': digest(before),
            'sdk_after_sha256': digest(actual), 'independent_after_sha256': digest(expected),
            'before_bytes': len(before), 'after_bytes': len(expected),
            'full_image_match': expected == actual, 'first_differences': differences[:32],
            'source_unchanged': source.read_bytes() == before,
        })
        print(name, expected == actual, digest(expected), flush=True)
        source = actual_path
    result = {
        'scope': 'Portable independent model replay; SDK not re-executed',
        'model_sha256': digest(Path(__file__).read_bytes()),
        'raw_owner': 'docs/testing/number_column_repeat/raw.py',
        'raw_owner_sha256': digest(Path(raw.__file__).read_bytes()),
        'observations': observations,
    }
    (args.output_dir / 'independent-comparison.json').write_text(json.dumps(result, indent=2) + '\n')
    assert all(item['full_image_match'] and item['source_unchanged'] for item in observations)


if __name__ == '__main__':
    main()
