"""Verify two internal-parent source histories without JVM or native calls."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
sys.path.insert(0, str(Path(__file__).resolve().parent))
import certificate
import delete_reference_maps as maps
from parent_index_split_originals import wire

REPORT = '.cache/ace16-internal-parent-source-first-20261027/report.json'
REPORT_HASH = '2bcc4c81eb83e00f3ac6d49e8aec0efc3a39f68f0816aa4aeea8a2d523062c66'


def verify(pin):
    raw = (ROOT / pin['path']).read_bytes()
    assert len(raw) == pin['bytes']
    assert hashlib.sha256(raw).hexdigest() == pin['sha256']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', default=REPORT)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        raise FileExistsError(args.output)
    raw_report = (ROOT / args.report).read_bytes()
    assert hashlib.sha256(raw_report).hexdigest() == REPORT_HASH
    report = json.loads(raw_report)
    assert report['completed_profiles'] == 2 and report['process_count'] == 9
    assert report['immutable_tools_unchanged'] and 'fatal' not in report
    for command in report['commands']:
        assert command['status'] == 0 and not command['timed_out']
        verify(command['stdout'])
        verify(command['stderr'])
    for case in report['cases']:
        assert case['complete'] and 'failure' not in case
        verify(case['selection'])
        selection = wire.selection((ROOT / case['selection']['path']).read_text())
        proofs = []
        for name, sparse in [('larger_capture', False), ('sparse_capture', True)]:
            capture = case[name]
            verify(capture['file'])
            verify(capture['proof'])
            raw = (ROOT / capture['file']['path']).read_bytes()
            text = (ROOT / capture['command']['stdout']['path']).read_text()
            tables, map_text = wire.parse_description(text)
            proof = certificate.certify(raw, tables, selection, sparse)
            metadata = maps.parse(map_text)
            decoded = {}
            for role, description in metadata['maps'].items():
                decoded[role] = maps.decode_map(raw, description, wire.PAGE_BYTES, len(raw) // wire.PAGE_BYTES)
            assert sorted(decoded['IndexOwned']['file_members']) == sorted(map(int, proof['nodes']))
            order = wire.native_order(raw, decoded['TableOwned']['file_members'], tables['SparseInt32'])
            proof['selected_row']['native_ordinal'] = order[proof['selected_value']]
            expected = json.loads((ROOT / capture['proof']['path']).read_text())
            assert json.loads(json.dumps(proof)) == expected['proof']
            assert json.loads(json.dumps(decoded)) == expected['maps']
            assert metadata == expected['map_metadata']
            proofs.append(proof)
        assert certificate.compare_history(*proofs) == case['history']
        assert case['larger_file'] == case['larger_after']
        assert case['sparse_file'] == case['sparse_after']
    receipt = {'verified_profiles': 2, 'source_processes': 9,
               'all_three_height_chains_and_child_bounds_preserved': True,
               'native_or_jvm_calls': 0, 'report_sha256': REPORT_HASH}
    args.output.write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    main()
