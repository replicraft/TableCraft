"""Read-only diagnosis of the immutable first non-root observation report."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
REPORT = Path('.cache/ace16-nonroot-split-original-actions-initial-20261027/report.json')
REPORT_SHA256 = '36f22c7426f5e569b4f7c8a1adf8d42f002cc488034f2bda9201948b0f192bc3'
OUTPUT = Path('.cache/ace16-nonroot-split-next-20261027/diagnosis.json')
PAGE_BYTES = 4096
REFERENCE_BYTES = 4
KEY_BYTES = 5
SELECTED = '1000'
REPLACEMENT = '2147483647'
FIELD_START = 2
FIELD_END = 6
MEMO_REFERENCE = range(20, 24)
OLE_REFERENCE = range(32, 36)


def pin(path):
    path = (ROOT / path).resolve()
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


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
    assert pin(args.report)['sha256'] == REPORT_SHA256
    report = json.loads(report_path.read_text())
    result = {'source': pin(__file__), 'initial_report': pin(args.report),
              'scope': 'Post-execution read-only diagnosis; original failed verifier and actions unchanged',
              'native_or_java_calls': 0, 'cases': []}
    for case in report['cases']:
        before, after = case['before'], case['after']
        for capture in (before, after):
            assert pin(capture['file']['path']) == capture['file']
        old = (ROOT / before['file']['path']).read_bytes()
        new = (ROOT / after['file']['path']).read_bytes()
        selected_before = bytes.fromhex(before['raw_rows']['Tree'][SELECTED]['hex'])
        selected_after = bytes.fromhex(after['raw_rows']['Tree'][REPLACEMENT]['hex'])
        assert len(selected_before) == len(selected_after) == 49
        assert selected_after[FIELD_START:FIELD_END] == int(REPLACEMENT).to_bytes(4, 'little')
        allowed = set(range(FIELD_START, FIELD_END)) | set(MEMO_REFERENCE) | set(OLE_REFERENCE)
        differences = [position for position, pair in enumerate(zip(selected_before, selected_after))
                       if pair[0] != pair[1]]
        assert set(differences) <= allowed
        references = []
        for label, span in [('Memo', MEMO_REFERENCE), ('Payload', OLE_REFERENCE)]:
            start, end = span.start, span.stop
            prior = int.from_bytes(selected_before[start:end], 'little')
            current = int.from_bytes(selected_after[start:end], 'little')
            references.append({'column': label, 'record_byte_range': [start, end],
                               'before_page_slot': list(divmod(prior, 256)),
                               'after_page_slot': list(divmod(current, 256))})
        for name, table in before['metadata'].items():
            expected_rows = dict(table['rows'])
            if name == 'Tree':
                expected_rows[REPLACEMENT] = expected_rows.pop(SELECTED)
            assert after['metadata'][name]['rows'] == expected_rows
            for identity, row in before['raw_rows'][name].items():
                destination = REPLACEMENT if name == 'Tree' and identity == SELECTED else identity
                current = after['raw_rows'][name][destination]
                assert (row['page'], row['slot'], row['word']) == (current['page'], current['slot'], current['word'])
                if not (name == 'Tree' and identity == SELECTED):
                    assert row == current
        indexes = []
        counters = []
        for name, owner in before['geometry'].items():
            for root, tree in owner['trees'].items():
                label = tree['logical_names'][0]
                new_root = after['map_info']['indexes'][name + '/' + label]['root']
                later = after['geometry'][name]['trees'][str(new_root)]
                expected = list(tree['entry_hex'])
                if name == 'Tree' and label == 'PrimaryKey':
                    reference = case['planned']['record_reference_hex']
                    positions = [position for position, entry in enumerate(expected)
                                 if entry[-REFERENCE_BYTES * 2:] == reference]
                    assert len(positions) == 1
                    entry = expected.pop(positions[0])
                    assert entry[:KEY_BYTES * 2] == '7f800003e8'
                    expected.append('7fffffffff' + reference)
                    expected.sort()
                assert expected == later['entry_hex']
                assert int(root) == new_root
                original_pages = tree['pages']
                changed = [page for page in original_pages
                           if old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                           != new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]]
                indexes.append({'table': name, 'logical_names': tree['logical_names'],
                                'root': int(root), 'before': tree, 'after': later,
                                'rewritten_original_pages': changed,
                                'full_key_row_stream_verified': True})
            for label, index in before['metadata'][name]['indexes'].items():
                later = after['metadata'][name]['indexes'][label]
                counters.append({'table': name, 'index': label,
                                 'before': index['historical_count'],
                                 'after': later['historical_count']})
        map_changes = {}
        for name, initial in before['maps'].items():
            later = after['maps'][name]
            cells = dict(initial['bit_locations'])
            cells.update(later['bit_locations'])
            offsets = sorted({cell['offset'] for cell in cells.values() if cell['offset'] < len(old)})
            map_changes[name] = {
                'added_members': sorted(set(later['file_members']) - set(initial['file_members'])),
                'removed_members': sorted(set(initial['file_members']) - set(later['file_members'])),
                'existing_byte_changes': [{'offset': offset, 'before': old[offset], 'after': new[offset]}
                                          for offset in offsets if old[offset] != new[offset]],
            }
        result['cases'].append({
            'profile': case['profile'],
            'atomic_native_refusal': case['atomic_refusal'],
            'native_error': case['actual_error'],
            'public_action_status': case['public_action']['status'],
            'typed_rows_memo_ole_lengths_and_digests_verified': True,
            'all_other_raw_row_bodies_unchanged': True,
            'selected_record_changed_positions': differences,
            'selected_record_changes_bounded_to_ID_and_LVAL_references': True,
            'selected_lval_references': references,
            'before_bytes': len(old), 'after_bytes': len(new),
            'new_page_headers': [{'page': page, 'header_hex': new[page * PAGE_BYTES:page * PAGE_BYTES + 27].hex()}
                                 for page in range(len(old) // PAGE_BYTES, len(new) // PAGE_BYTES)],
            'indexes': indexes, 'map_changes': map_changes,
            'historical_counters': counters,
            'existing_changed_pages': [page for page in range(len(old) // PAGE_BYTES)
                                       if old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                                       != new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]],
        })
    if args.verify_existing:
        existing = json.loads(output_path.read_text())
        result.pop('source')
        existing.pop('source')
        assert result == existing
        print('Existing diagnosis reverified without native/JVM calls; source pin differs by public promotion.')
    else:
        output_path.write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(pin(output_path)))


if __name__ == '__main__':
    main()
