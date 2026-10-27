"""Recompute actual parent UPDATE comparisons without native/JVM calls."""
import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from parent_index_split_originals import wire
from parent_index_split_originals.actions import model
from slot_preview_originals import common
import delete_reference_maps as maps

DEFAULT_REPORT = Path('.cache/ace16-parent-split-actions-initial-20261027/report.json')
REPORT_SHA256 = 'fb970dbb6263bc3146d2852549dfdbc8b6119d456fe8c42cd9f2fca72e54566c'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, default=DEFAULT_REPORT)
    args = parser.parse_args()
    assert common.pin(args.report)['sha256'] == REPORT_SHA256
    report = json.loads(common.path(args.report).read_text())
    assert report['completed_profiles'] == 2 and report['process_count'] == 9
    assert report['atomic_native_refusals'] == 2 and report['observed_root_growths'] == 2
    for command in report['commands']:
        for name in ('stdout', 'stderr'):
            assert common.pin(command[name]['path']) == command[name]
    for case in report['cases']:
        captures = []
        for name in ('before', 'after'):
            saved = case[name]
            assert common.pin(saved['file']['path']) == saved['file']
            raw = common.path(saved['file']['path']).read_bytes()
            tables, text = wire.parse_description(common.path(saved['command']['stdout']['path']).read_text())
            metadata = maps.parse(text)
            decoded = {label: maps.decode_map(raw, item, wire.PAGE_BYTES, len(raw) // wire.PAGE_BYTES)
                       for label, item in metadata['maps'].items()}
            assert json.loads(json.dumps(decoded)) == saved['maps']
            captures.append((raw, tables, decoded))
        before, after = captures
        compared = model.compare(before[0], after[0], before[1], after[1], before[2], after[2], wire)
        assert compared == case['comparison']
        assert case['native_atomic_refusal']
        assert case['native_actual_error'] == 'Error: Unsupported(IndexedColumn)\n'
    print('Reverified both actual root-growth histories and native atomic refusals; no native/JVM calls.')


if __name__ == '__main__':
    main()
