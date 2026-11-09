"""Verify saved public-Memo observations without opening databases or running Java."""
import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PAGE_BYTES = 4096
MEMBERSHIP_ROLES = ('TableOwned', 'CompactMemoOwned', 'RawMemoOwned', 'OLEOwned')


def pin(path):
    path = (ROOT / path).resolve()
    path.relative_to(ROOT)
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def collect(value, result):
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            expected = {key: value[key] for key in ('path', 'bytes', 'sha256')}
            if expected['path'] in result:
                assert result[expected['path']] == expected
            result[expected['path']] = expected
        for child in value.values():
            collect(child, result)
    elif isinstance(value, list):
        for child in value:
            collect(child, result)


def verify(path):
    report = json.loads((ROOT / path).read_text())
    assert report['completed'] and report['originals_and_all_source_parents_unchanged']
    assert report['prerequisites_completed_before_updates']
    assert len(report['prerequisites']) == 2
    count = report['public_updates']
    assert count == len(report['cases']) and count in (56, 92)
    assert report['processes'] == len(report['commands'])
    assert report['processes'] == {56: 173, 92: 283}[count]
    assert all(command['status'] == 0 and not command['timed_out'] for command in report['commands'])

    pins = {}
    collect(report, pins)
    for expected in pins.values():
        assert pin(expected['path']) == expected

    sources = {'original:' + item['profile']: item['state'] for item in report['prerequisites']}
    forms = Counter()
    growth = Counter()
    transitions = Counter()
    additions = Counter()
    removals = Counter()
    old_chunks = []
    ancestor_chunks = []
    mismatches = []
    matrix = []
    map_proofs = 0
    eof_pages = 0
    existing_lval_reuse_cases = 0
    for action in report['cases']:
        assert action['completed']
        selection = action['selection']
        before = sources[selection['source_ref']]
        after = action['after']
        comparison = action['comparison']
        field = after['fields'][selection['column']]
        assert comparison['all_other_values_rows_indexes_preserved']
        assert comparison['protected_metadata_indexes_preserved']
        forms[field['kind']] += 1
        growth[comparison['file_growth_bytes']] += 1
        transitions[comparison['before_kind'] + '->' + field['kind']] += 1
        old_chunks.extend(comparison['old_chunk_bodies'])
        ancestor_chunks.extend(action['cumulative_old_chunks'])

        allocated = []
        for role, delta in comparison['map_deltas'].items():
            additions[role] += len(delta['added'])
            removals[role] += len(delta['removed'])
            if role in MEMBERSHIP_ROLES:
                allocated.extend(delta['added'])
                assert not delta['removed']
        eof = list(range(before['file']['bytes'] // PAGE_BYTES, after['file']['bytes'] // PAGE_BYTES))
        global_delta = comparison['map_deltas']['GlobalFree']
        assert sorted(allocated) == eof
        assert sorted(global_delta['removed']) == eof and not global_delta['added']
        eof_pages += len(eof)
        for proof in comparison['map_wire_proofs'].values():
            assert not proof['representation_changed'] and proof['exact_bit_only_claim']
            map_proofs += 1
        prior_pages = before['maps'][selection['column'] + 'Owned']['file_members']
        if any(chunk['page'] in prior_pages for chunk in field['chunks']):
            existing_lval_reuse_cases += 1

        if not comparison['requested_value_equal']:
            assert selection['normalization_probe']
            requested = bytes.fromhex(comparison['requested_utf8_hex']).decode('utf-8')
            actual = bytes.fromhex(comparison['actual_utf8_hex']).decode('utf-8')
            mismatches.append({'id': selection['id'],
                               'requested_utf16_units': len(requested.encode('utf-16le')) // 2,
                               'actual_utf16_units': len(actual.encode('utf-16le')) // 2,
                               'requested_sha256': hashlib.sha256(requested.encode('utf-8')).hexdigest(),
                               'actual_sha256': hashlib.sha256(actual.encode('utf-8')).hexdigest()})
        if selection['phase'] == 'direct' and selection['profile'] == 'V2016':
            wire = bytes.fromhex(field['encoded_hex'])
            matrix.append({'column': selection['column'], 'input': selection['payload_label'],
                           'encoded_bytes': field['encoded_bytes'], 'storage': field['kind'],
                           'marker_prefix': wire.startswith(b'\xff\xfe'),
                           'requested_value_equal': comparison['requested_value_equal']})
        sources[selection['id']] = after

    assert all(chunk['body_unchanged'] and chunk['slot_word_before'] == chunk['slot_word_after']
               for chunk in old_chunks + ancestor_chunks)
    assert set(report['normalization_observations']) == {item['id'] for item in mismatches}
    return {
        'report': pin(path), 'recursive_live_pins_verified': len(pins),
        'counts': {'public_updates': count, 'bounded_processes': report['processes'],
                   'process_failures': 0, 'requested_value_equal': count - len(mismatches),
                   'requested_getter_mismatches': len(mismatches), 'metadata_prerequisites': 2,
                   'immediate_old_chunk_comparisons': len(old_chunks),
                   'cumulative_ancestor_chunk_comparisons': len(ancestor_chunks),
                   'changed_old_bodies_or_slot_words': 0, 'stable_map_wire_proofs': map_proofs,
                   'eof_owned_additions_equal_global_clears': eof_pages,
                   'owned_removals': 0, 'existing_lval_page_reuse_cases': existing_lval_reuse_cases,
                   'standalone_decoder_streams': sum(item['matched_streams'] for item in report['oracles'])},
        'storage_forms': dict(forms), 'growth_bytes': dict(growth), 'transitions': dict(transitions),
        'map_additions': dict(additions), 'map_removals': dict(removals),
        'mismatches': mismatches, 'originals': report['originals_before'],
        'v2016_direct_matrix': matrix,
        'scope': 'Saved-byte/hash verification only; no Java, database updates, native candidates or native Access execution.',
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    arguments = parser.parse_args()
    output = (ROOT / arguments.output).resolve()
    output.relative_to(ROOT)
    assert not output.exists(), 'Use a fresh verification output'
    summary = verify(arguments.report)
    output.write_text(json.dumps(summary, indent=2, ensure_ascii=True) + '\n')
    print(json.dumps({'verified': True, 'summary': pin(output), 'counts': summary['counts']}))


if __name__ == '__main__':
    main()
