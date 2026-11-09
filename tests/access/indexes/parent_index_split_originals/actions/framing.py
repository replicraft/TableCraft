"""Read-only exact changed-byte classification of accepted public root growth."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
REPORT = Path('.cache/ace16-parent-split-actions-initial-20261027/report.json')
OUTPUT = Path('.cache/ace16-parent-split-actions-next-20261027/framing.json')
REPORT_SHA256 = 'fb970dbb6263bc3146d2852549dfdbc8b6119d456fe8c42cd9f2fca72e54566c'
PAGE_BYTES = 4096
ENTRY_START = 480


def pin(path):
    path = (ROOT / path).resolve()
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def changed(before, after):
    return [offset for offset, pair in enumerate(zip(before, after)) if pair[0] != pair[1]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, default=REPORT)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verify-existing', action='store_true')
    args = parser.parse_args()
    assert pin(args.report)['sha256'] == REPORT_SHA256
    report = json.loads((ROOT / args.report).read_text())
    result = {'scope': 'Post-action read-only framing observations, not native Access or kernel write parity',
              'source': pin(__file__), 'report': pin(args.report), 'native_or_java_calls': 0, 'cases': []}
    for case in report['cases']:
        old = (ROOT / case['before']['file']['path']).read_bytes()
        new = (ROOT / case['after']['file']['path']).read_bytes()
        comparison = case['comparison']
        root = comparison['root_before']
        right = case['selector']['target_leaf']
        node_root = comparison['after_tree']['nodes'][str(root)]
        node_right = comparison['after_tree']['nodes'][str(right)]
        root_end = PAGE_BYTES - node_root['free']
        right_end = PAGE_BYTES - node_right['free']
        item = {'profile': case['profile'], 'root': root, 'retained_right': right,
                'existing_page_byte_changes': {},
                'root_residual_from_new_end': root_end,
                'root_residual_changed_bytes': len(changed(old[root * PAGE_BYTES + root_end:(root + 1) * PAGE_BYTES],
                                                         new[root * PAGE_BYTES + root_end:(root + 1) * PAGE_BYTES])),
                'right_residual_from_new_end': right_end,
                'right_residual_changed_bytes': len(changed(old[right * PAGE_BYTES + right_end:(right + 1) * PAGE_BYTES],
                                                          new[right * PAGE_BYTES + right_end:(right + 1) * PAGE_BYTES]))}
        for page in comparison['existing_changed_pages']:
            delta = changed(old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES],
                            new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES])
            item['existing_page_byte_changes'][str(page)] = {'count': len(delta),
                                                            'first_offsets': delta[:16],
                                                            'last_offsets': delta[-16:]}
        selected = next(record for record in comparison['records'] if record['original_value'] == 99424)
        offset = selected['before']['offset'] + 2
        page = selected['before_reference'][0]
        delta = changed(old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES],
                        new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES])
        assert delta == list(range(offset - page * PAGE_BYTES, offset - page * PAGE_BYTES + 4))
        item['entire_selected_data_page_changes_only_four_Key_bytes'] = True
        result['cases'].append(item)
    output = (ROOT / args.output).resolve()
    output.relative_to(ROOT)
    if args.verify_existing:
        expected = json.loads(output.read_text())
        expected.pop('source')
        result.pop('source')
        assert result == expected
        print('Saved framing observations verified; no native/JVM calls.')
    else:
        with output.open('x') as destination:
            json.dump(result, destination, indent=2)
            destination.write('\n')
        print(json.dumps(pin(output)))


if __name__ == '__main__':
    main()
