"""Read-only old/new LVAL chain audit; no database or engine process calls."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REPORT = Path('.cache/ace16-nonroot-split-original-actions-initial-20261027/report.json')
OUTPUT = Path('.cache/ace16-nonroot-split-next-20261027/lval-chains.json')
PAGE_BYTES = 4096
SLOT_OFFSET = 14
SLOT_BYTES = 2
OFFSET_MASK = 0x1FFF
NEXT_BYTES = 4
LVAL_OWNER = b'LVAL'
DESCRIPTORS = (('Memo', 16), ('Payload', 28))


def chain(raw, reference):
    pages = []
    payload = bytearray()
    while reference:
        page, slot = divmod(reference, 256)
        assert page not in pages and page * PAGE_BYTES < len(raw)
        start = page * PAGE_BYTES
        assert raw[start:start + 2] == b'\x01\x01'
        assert raw[start + 4:start + 8] == LVAL_OWNER
        word_offset = start + SLOT_OFFSET + slot * SLOT_BYTES
        word = int.from_bytes(raw[word_offset:word_offset + SLOT_BYTES], 'little')
        low = start + (word & OFFSET_MASK)
        high = start + PAGE_BYTES
        if slot:
            previous_word = int.from_bytes(raw[word_offset - SLOT_BYTES:word_offset], 'little')
            high = start + (previous_word & OFFSET_MASK)
        assert low + NEXT_BYTES <= high
        reference = int.from_bytes(raw[low:low + NEXT_BYTES], 'little')
        payload.extend(raw[low + NEXT_BYTES:high])
        pages.append(page)
    return pages, bytes(payload)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, default=REPORT)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verify-existing', action='store_true')
    args = parser.parse_args()
    report_path = (ROOT / args.report).resolve()
    output_path = (ROOT / args.output).resolve()
    report_path.relative_to(ROOT)
    output_path.relative_to(ROOT)
    if not args.verify_existing:
        assert not output_path.exists()
    report = json.loads(report_path.read_text())
    output = {'scope': 'Separate read-only LVAL chain proof; original actions and failed verifier unchanged',
              'native_or_java_calls': 0, 'cases': []}
    for case in report['cases']:
        before, after = case['before'], case['after']
        old = (ROOT / before['file']['path']).read_bytes()
        new = (ROOT / after['file']['path']).read_bytes()
        original = bytes.fromhex(before['raw_rows']['Tree']['1000']['hex'])
        updated = bytes.fromhex(after['raw_rows']['Tree']['2147483647']['hex'])
        item = {'profile': case['profile'], 'chains': []}
        for name, offset in DESCRIPTORS:
            assert original[offset:offset + NEXT_BYTES] == updated[offset:offset + NEXT_BYTES]
            prior_ref = int.from_bytes(original[offset + NEXT_BYTES:offset + 2 * NEXT_BYTES], 'little')
            later_ref = int.from_bytes(updated[offset + NEXT_BYTES:offset + 2 * NEXT_BYTES], 'little')
            old_pages, old_payload = chain(old, prior_ref)
            new_pages, new_payload = chain(new, later_ref)
            assert old_payload == new_payload
            assert all(old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                       == new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES] for page in old_pages)
            item['chains'].append({'column': name, 'old_pages': old_pages, 'new_pages': new_pages,
                                   'payload_bytes': len(old_payload),
                                   'payload_sha256': hashlib.sha256(old_payload).hexdigest(),
                                   'old_pages_byte_identical': True, 'complete_chain_payload_equal': True})
        output['cases'].append(item)
    if args.verify_existing:
        assert output == json.loads(output_path.read_text())
        print('Existing LVAL proof reverified without native/JVM calls.')
    else:
        output_path.write_text(json.dumps(output, indent=2) + '\n')
        print(json.dumps(output))


if __name__ == '__main__':
    main()
