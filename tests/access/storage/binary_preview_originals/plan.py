"""Rebuild the fixed Binary8 candidate plan without native or JVM calls."""
import argparse
import hashlib
import json
from pathlib import Path

from slot_preview_originals import common

TARGET_CAPACITY = 268
FIRST_CHECKPOINT = 12
SECOND_CHECKPOINT = 13
PAGE_BYTES = 4096


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference-report', type=Path, required=True)
    parser.add_argument('--reference-receipt', type=Path, required=True)
    parser.add_argument('--artifact', type=Path, required=True)
    parser.add_argument('--artifact-sha256', required=True)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    for field in ['reference_report', 'reference_receipt', 'artifact', 'output']:
        setattr(args, field, common.path(getattr(args, field)).relative_to(common.ROOT))
    args.output.relative_to('.cache')
    assert not args.output.exists()
    artifact = common.pin(args.artifact)
    assert artifact['sha256'] == args.artifact_sha256
    reference = json.loads(args.reference_report.read_text())
    cases = []
    for public_position, item in enumerate(reference['cases']):
        selection = item['selection']
        if not selection['action'].startswith('UPDATE'):
            continue
        assert item['complete']
        action = 'grow' if selection['action'] == 'UPDATE-grow' else 'shrink'
        profile = selection['profile']
        checkpoint = selection['checkpoint']
        assert checkpoint in [FIRST_CHECKPOINT, SECOND_CHECKPOINT]
        before = item['before']
        after = item['after']
        growth = PAGE_BYTES if action == 'grow' and checkpoint == FIRST_CHECKPOINT else 0
        assert item['comparison']['file_bytes_added'] == growth
        assert after['target']['bytes'] == TARGET_CAPACITY
        label = f'{profile}-checkpoint-{checkpoint}-{action}'
        case = {
            'profile': profile,
            'label': label,
            'action': action,
            'checkpoint': checkpoint,
            'table': 'BinaryStorage',
            'column': 'Bytes',
            'ID': 1,
            'ordinal': 0,
            'original_source': before['file'],
            'expected_getter': selection['expected_getter'],
            'replacement_hex': selection['payload_hex'],
            'root_page': before['root']['page'],
            'root_slot': 0,
            'target_page': after['target']['page'],
            'target_slot': after['target']['slot'],
            'target_capacity_bytes': TARGET_CAPACITY,
            'expected_file_growth_bytes': growth,
            'table_and_first_index_historical_count': 22 if checkpoint == FIRST_CHECKPOINT else 23,
            'expected_maps_from_public_raw': item['comparison']['map_changes'],
            'public_reference': after['file'],
            'public_reference_getter': after['getter']['stdout'],
            'public_source_getter': before['getter']['stdout'],
            'public_before_state': {
                'declared_rows': before['declared_rows'],
                'historical_index_rows': before['historical_index_rows'],
            },
            'public_after_state': {
                'declared_rows': after['declared_rows'],
                'historical_index_rows': after['historical_index_rows'],
            },
            'public_row_action_position': public_position,
            'native_source_from_case': len(cases) - 1 if action == 'shrink' else None,
        }
        assert before['declared_rows'] == case['table_and_first_index_historical_count']
        cases.append(case)
    assert len(cases) == 8
    for case in cases:
        public = reference['cases'][case['public_row_action_position']]
        case['id'] = case['label']
        case['source'] = case['original_source'] if case['action'] == 'grow' else {
            'candidate_from': cases[case['native_source_from_case']]['id'],
        }
        case['expected_kind'] = 'null' if case['action'] == 'grow' else 'binary'
        case['expected_hex'] = '' if case['action'] == 'grow' else public['before']['row_getter'][1].removeprefix('Binary:')
        case['reference_getter'] = case['public_reference_getter']
        case['reference_maps'] = public['after']['map_probe']['stdout']
    report_pin = common.pin(args.reference_report)
    plan = {
        'scope': 'Predeclared native Binary8 plan only; no candidate calls. Shrinks consume native growth candidates, not public growth copies.',
        'artifact': artifact,
        'compiled_revision': args.revision,
        'explicit_flag': '--variable-updates',
        'cases': cases,
        'expected_candidates': 8,
        'expected_refusals': 0,
        'public_report': report_pin,
        'reference_receipt': common.pin(args.reference_receipt),
        'raw_reference_report': report_pin,
    }
    args.output.write_text(json.dumps(plan, indent=2) + '\n')
    print(json.dumps({'plan': common.pin(args.output), 'native_calls': 0, 'JVM_calls': 0}))


if __name__ == '__main__':
    main()
