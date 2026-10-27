"""Recompute the accepted sparse-source proofs without native or JVM calls."""
import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from parent_index_split_originals import wire
from slot_preview_originals import common
import delete_reference_maps as maps

REPORT_SHA256 = '11151766680c9c2f863f545de26ea680ac7b5c5f4cd4f0ac60ca54b1f82fd077'
DEFAULT_REPORT = Path('.cache/ace16-parent-split-reference-initial-20261027/report.json')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, default=DEFAULT_REPORT)
    args = parser.parse_args()
    assert common.pin(args.report)['sha256'] == REPORT_SHA256
    report = json.loads(common.path(args.report).read_text())
    assert report['completed'] == 2 and report['command_count'] == 11
    for command in report['commands']:
        assert command['status'] == 0 and not command['timed_out']
        for name in ('stdout', 'stderr'):
            assert common.pin(command[name]['path']) == command[name]
    for case in report['cases']:
        chosen = wire.selection(common.path(case['selection']['path']).read_text())
        assert common.pin(case['selection']['path']) == case['selection']
        proofs = []
        for name, sparse in [('larger_capture', False), ('sparse_capture', True)]:
            capture = case[name]
            assert common.pin(capture['proof']['path']) == capture['proof']
            assert common.pin(capture['file']['path']) == capture['file']
            saved = json.loads(common.path(capture['proof']['path']).read_text())
            raw = common.path(capture['file']['path']).read_bytes()
            text = common.path(capture['command']['stdout']['path']).read_text()
            tables, map_text = wire.parse_description(text)
            proof = wire.certify(raw, tables, chosen, sparse)
            map_metadata = maps.parse(map_text)
            map_proof = {label: maps.decode_map(raw, data, wire.PAGE_BYTES, len(raw) // wire.PAGE_BYTES)
                         for label, data in map_metadata['maps'].items()}
            order = wire.native_order(raw, map_proof['TableOwned']['file_members'], tables['SparseInt32'])
            proof['selected_row']['native_ordinal'] = order[proof['selected_value']]
            proof['selected_row']['public_ordinal'] = proof['selected_row']['ordinal']
            assert proof == saved['proof']
            # JSON stores native integer bitmap-page keys as strings. Normalize
            # representation before comparing, without changing any byte facts.
            assert json.loads(json.dumps(map_proof)) == saved['maps']
            assert map_metadata == saved['map_metadata']
            proofs.append(proof)
        assert wire.compare_history(*proofs) == case['history']
        source = common.path(case['sparse_file']['path']).read_bytes()
        noop = common.path(case['byte_identical_admission_output']['path']).read_bytes()
        assert source == noop
        for item in (case['sparse_file'], case['larger_file'], case['byte_identical_admission_output']):
            assert common.pin(item['path']) == item
        sparse_saved = json.loads(common.path(case['sparse_capture']['proof']['path']).read_text())
        for label, cells in case['future_cells_for_three_pages'].items():
            metadata = sparse_saved['map_metadata']['maps'][label]
            assert [wire.future_cell(source, metadata, cell['page']) for cell in cells] == cells
    print('Reverified both source histories, complete key/ROW/maps and byte-identical no-op admissions; zero native/JVM calls.')


if __name__ == '__main__':
    main()
