"""Read-only used/unused regions of fresh pages and unchanged predecessor keys."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
REPORT = Path('.cache/ace16-parent-split-actions-initial-20261027/report.json')
OUTPUT = Path('.cache/ace16-parent-split-actions-next-20261027/fresh-framing.json')
EXPECTED_SHA256 = 'fb970dbb6263bc3146d2852549dfdbc8b6119d456fe8c42cd9f2fca72e54566c'
PAGE_BYTES = 4096
NEXT_OFFSET = 16
NEXT_BYTES = 4


def pin(path):
    path = (ROOT / path).resolve()
    value = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(value),
            'sha256': hashlib.sha256(value).hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, default=REPORT)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verify-existing', action='store_true')
    args = parser.parse_args()
    assert pin(args.report)['sha256'] == EXPECTED_SHA256
    source = json.loads((ROOT / args.report).read_text())
    report = {'scope': 'Separate post-action read-only preservation observations; no native/JVM calls',
              'source': pin(__file__), 'input': pin(args.report), 'cases': []}
    for case in source['cases']:
        before = (ROOT / case['before']['file']['path']).read_bytes()
        after = (ROOT / case['after']['file']['path']).read_bytes()
        comparison = case['comparison']
        old_tree = comparison['before_tree']['nodes']
        new_tree = comparison['after_tree']['nodes']
        fresh = []
        for number in comparison['added_index_pages']:
            page = new_tree[str(number)]
            end = PAGE_BYTES - page['free']
            unused = after[number * PAGE_BYTES + end:(number + 1) * PAGE_BYTES]
            fresh.append({'page': number, 'height': page['height'], 'prefix': page['prefix'],
                          'entries': len(page['entries_hex']), 'active_end': end,
                          'unused_nonzero_bytes': sum(byte != 0 for byte in unused),
                          'unused_sha256': hashlib.sha256(unused).hexdigest()})
        leaf = case['selector']['target_leaf']
        predecessor = old_tree[str(leaf)]['previous']
        assert old_tree[str(predecessor)]['entries_hex'] == new_tree[str(predecessor)]['entries_hex']
        delta = [offset for offset in range(PAGE_BYTES)
                 if before[predecessor * PAGE_BYTES + offset] != after[predecessor * PAGE_BYTES + offset]]
        unused_end = PAGE_BYTES - new_tree[str(predecessor)]['free']
        outside_link = [offset for offset in delta if not NEXT_OFFSET <= offset < NEXT_OFFSET + NEXT_BYTES]
        assert all(offset >= unused_end for offset in outside_link)
        report['cases'].append({'profile': case['profile'], 'fresh_pages': fresh,
                                'predecessor': predecessor, 'predecessor_active_end': unused_end,
                                'predecessor_total_changed_bytes': len(delta),
                                'predecessor_changed_bytes_outside_next_link': len(outside_link),
                                'all_outside_link_changes_are_unused_residue': True,
                                'predecessor_complete_key_row_unchanged': True})
    output = (ROOT / args.output).resolve()
    output.relative_to(ROOT)
    if args.verify_existing:
        expected = json.loads(output.read_text())
        expected.pop('source')
        report.pop('source')
        assert report == expected
        print('Saved framing observations verified; no native/JVM calls.')
    else:
        with output.open('x') as destination:
            json.dump(report, destination, indent=2)
            destination.write('\n')
        print(json.dumps(pin(output)))


if __name__ == '__main__':
    main()
