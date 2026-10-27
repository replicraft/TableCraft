"""Recompute measured internal-parent public actions without native/JVM calls."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import action_model
import delete_reference_maps as maps
from parent_index_split_originals import wire

REPORT = '.cache/ace16-internal-parent-actions-first-20261027/report.json'
REPORT_HASH = 'a7a9b561a28a6f3086d96073299ad25e64122e9332c89bc640a71a9c061ed371'


def verify(pin):
    data = (ROOT / pin['path']).read_bytes()
    assert len(data) == pin['bytes']
    assert hashlib.sha256(data).hexdigest() == pin['sha256']


def captured(capture):
    verify(capture['file'])
    raw = (ROOT / capture['file']['path']).read_bytes()
    text = (ROOT / capture['command']['stdout']['path']).read_text()
    tables, map_text = wire.parse_description(text)
    metadata = maps.parse(map_text)
    decoded = {}
    for role, description in metadata['maps'].items():
        decoded[role] = maps.decode_map(raw, description, wire.PAGE_BYTES, len(raw) // wire.PAGE_BYTES)
    assert json.loads(json.dumps(decoded)) == capture['maps']
    assert metadata == capture['map_metadata']
    return raw, tables, decoded


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', default=REPORT)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise FileExistsError(args.output)
    data = (ROOT / args.report).read_bytes()
    assert hashlib.sha256(data).hexdigest() == REPORT_HASH
    report = json.loads(data)
    assert report['completed_profiles'] == 2 and report['process_count'] == 9
    assert report['atomic_native_refusals'] == 2 and report['observed_root_growths'] == 0
    assert report['immutable_sources_and_inputs_unchanged']
    for command in report['commands']:
        assert not command['timed_out']
        verify(command['stdout'])
        verify(command['stderr'])
    for case in report['cases']:
        assert case['complete'] and 'failure' not in case
        assert case['native_actual_error'] == 'Error: Unsupported(IndexedColumn)\n'
        assert case['native_atomic_refusal'] and case['native_candidate'] is None
        verify(case['selector']['source'])
        before, before_tables, before_maps = captured(case['before'])
        after, after_tables, after_maps = captured(case['after'])
        comparison = action_model.compare(before, after, before_tables, after_tables, before_maps, after_maps, wire)
        assert comparison == case['comparison']
        assert all(row['same_root_reference'] for row in comparison['records'])
        assert all(row['expected_fixed_patch_or_unchanged_body'] for row in comparison['records'])
        assert comparison['root_height_before'] == comparison['root_height_after'] == 2
    receipt = {'verified_profiles': 2, 'record_bodies': 1926,
               'public_update_observations': 2, 'prior_service_atomic_refusals': 2,
               'native_or_jvm_calls': 0, 'report_sha256': REPORT_HASH}
    args.output.write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
