"""Separate saved-copy framing classification; no fixture process or mutation."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PAGE = 4096
ENTRY_START = 480
KIND = 0
FREE = 2
TAIL = 20
PREFIX = 24
HEIGHT = 26
MASK_START = 27
U16_BYTES = 2
U32_BYTES = 4


def pin(path):
    data = path.read_bytes()
    label = path.resolve().relative_to(ROOT).as_posix()
    return {'path': label, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def framing(report_path):
    before_report = pin(report_path)
    report = json.loads(report_path.read_text())
    cases = []
    for case in report['cases']:
        source = Path(case['source']['path'])
        reference = Path(case['reference']['path'])
        before = source.read_bytes()
        after = reference.read_bytes()
        physical = {}
        for label, effect in case['analysis']['selected_index_effects'].items():
            if effect['physical'] in physical:
                continue
            root = effect['after_root']
            old = before[root * PAGE:(root + 1) * PAGE]
            new = after[root * PAGE:(root + 1) * PAGE]
            end = PAGE - int.from_bytes(new[FREE:FREE + U16_BYTES], 'little')
            suffix_changes = sum(a != b for a, b in zip(old[end:], new[end:]))
            unauthorized = []
            for position in range(PAGE):
                allowed = position in (KIND, HEIGHT)
                allowed |= FREE <= position < FREE + U16_BYTES
                allowed |= TAIL <= position < TAIL + U32_BYTES
                allowed |= PREFIX <= position < PREFIX + U16_BYTES
                allowed |= MASK_START <= position < end
                if not allowed and old[position] != new[position]:
                    unauthorized.append(position)
            fresh = []
            for page in effect['owned_added']:
                body = after[page * PAGE:(page + 1) * PAGE]
                active_end = PAGE - int.from_bytes(body[FREE:FREE + U16_BYTES], 'little')
                fresh.append({'page': page, 'active_end': active_end,
                              'unused_bytes': PAGE - active_end,
                              'unused_nonzero_bytes': sum(value != 0 for value in body[active_end:])})
            physical[effect['physical']] = {'root': root, 'root_active_end': end,
                                            'old_root_suffix_changed_bytes': suffix_changes,
                                            'changes_outside_engineering_root_spans': unauthorized,
                                            'fresh_pages': fresh}
        cases.append({'profile': case['profile'], 'target': case['target'],
                      'source': pin(source), 'reference': pin(reference),
                      'physical': physical})
    assert pin(report_path) == before_report
    return {'read_only_saved_copy_analysis': True, 'fixture_processes_repeated': 0,
            'raw_report_unchanged': True, 'raw_report': before_report, 'cases': cases}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    assert not args.output.exists()
    result = framing(args.report)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'cases': len(result['cases']), 'raw_report_unchanged': True,
                      'fixture_processes_repeated': 0}, indent=2))


if __name__ == '__main__':
    main()
