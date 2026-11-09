"""Read-only classification of the initial two public UPDATEs and native refusals."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from index_split_originals import capture



def verify_pin(pin):
    assert capture.pin(pin['path']) == pin


def saved(state):
    verify_pin(state['file'])
    for command in [state['getter'], state['map_probe']]:
        assert command['status'] == 0 and not command['timed_out']
        verify_pin(command['stdout'])
        verify_pin(command['stderr'])
        assert not Path(command['stderr']['path']).read_bytes()
    raw = Path(state['file']['path']).read_bytes()
    metadata, relations = capture.parse(Path(state['getter']['stdout']['path']).read_text())
    map_info = capture.maps.parse(Path(state['map_probe']['stdout']['path']).read_text())
    computed = {
        'file': capture.pin(state['file']['path']), 'getter': state['getter'],
        'map_probe': state['map_probe'], 'metadata': metadata, 'relations': relations,
        'map_info': map_info,
        'maps': {name: capture.maps.decode_map(raw, description, capture.PAGE_BYTES, len(raw) // capture.PAGE_BYTES)
                 for name, description in map_info['maps'].items()},
        'geometry': capture.geometry(raw, metadata, map_info),
        'raw_rows': {name: {str(identity): capture.body(raw, *location)
                           for identity, location in table['locations'].items()}
                     for name, table in metadata.items()},
    }
    assert json.loads(json.dumps(computed)) == state
    return raw, computed


def differences(old, new):
    positions = [offset for offset, (a, b) in enumerate(zip(old, new)) if a != b]
    intervals = []
    for offset in positions:
        if intervals and intervals[-1][1] == offset:
            intervals[-1][1] += 1
        else:
            intervals.append([offset, offset + 1])
    return {'changed_bytes': len(positions), 'relative_intervals': intervals}


def classify(item):
    old, b = saved(item['before'])
    new, a = saved(item['after'])
    assert item['old_service_atomic_refusal'] and not item['old_service_candidate_exists']
    assert item['old_service_actual_error_text'] == 'Error: Unsupported(IndexedColumn)\n'
    for command in [item['old_service'], item['public_action']]:
        verify_pin(command['stdout'])
        verify_pin(command['stderr'])
        assert not command['timed_out']
    assert item['public_action']['status'] == 0
    assert Path(item['public_action']['stdout']['path']).read_text() == 'updated=1; foreignKeys=true; expressions=false; timezone=UTC\n'
    assert b['relations'] == a['relations']
    expected = dict(b['metadata']['Children']['rows'])
    expected[capture.REPLACEMENT] = expected.pop(1)
    assert a['metadata']['Children']['rows'] == expected
    assert a['metadata']['Children']['geometry']['live_rows'] == capture.EXPECTED_ROWS
    for name in ['Parents', 'Sentinel']:
        assert b['metadata'][name] == a['metadata'][name]
    for name, rows in b['raw_rows'].items():
        for identity, row in rows.items():
            if name != 'Children' or identity != '1':
                assert row == a['raw_rows'][name][identity]
    prior = b['raw_rows']['Children']['1']
    current = a['raw_rows']['Children'][str(capture.REPLACEMENT)]
    assert (prior['page'], prior['slot'], prior['word']) == (current['page'], current['slot'], current['word'])
    before_body = bytes.fromhex(prior['hex'])
    after_body = bytes.fromhex(current['hex'])
    assert after_body == before_body[:2] + capture.REPLACEMENT.to_bytes(4, 'little') + before_body[6:]
    primary = None
    other = []
    public_repartitions = []
    for name, owner in b['geometry'].items():
        before_pages = capture.trees.pages(old, b['metadata'][name])
        after_pages = capture.trees.pages(new, a['metadata'][name])
        for root, tree in owner['trees'].items():
            label = tree['logical_names'][0]
            new_root = a['map_info']['indexes'][name + '/' + label]['root']
            later = a['geometry'][name]['trees'][str(new_root)]
            if name == 'Children' and label == 'PrimaryID':
                entries = [bytes.fromhex(entry) for entry in tree['entry_hex']]
                removed = entries.pop(0)
                assert removed[:5] == bytes.fromhex('7f80000001')
                entries.append(bytes.fromhex('7fffffffff') + removed[-4:])
                assert later['entry_hex'] == [entry.hex() for entry in entries]
                assert int(root) == new_root
                assert tree['height'] == 0 and later['height'] == 1
                assert len(later['leaf_partitions']) == 2
                new_pages = sorted(set(later['pages']) - set(tree['pages']))
                assert new_pages == [len(old) // capture.PAGE_BYTES, len(old) // capture.PAGE_BYTES + 1]
                primary = {'root': new_root, 'before': tree, 'after': later,
                           'new_eof_pages': new_pages, 'full_key_row_replacement_exact': True,
                           'all_child_maxima_validated': True}
                continue
            assert later['entry_hex'] == tree['entry_hex']
            assert later['pages'] == tree['pages']
            changed = []
            for number in tree['pages']:
                low = number * capture.PAGE_BYTES
                delta = differences(old[low:low + capture.PAGE_BYTES], new[low:low + capture.PAGE_BYTES])
                if delta['changed_bytes']:
                    before_page = before_pages[number]
                    after_page = after_pages[number]
                    changed.append({'page': number, 'kind': before_page['kind'],
                                    'height': before_page['height'], 'before_entries': len(before_page['entries_hex']),
                                    'after_entries': len(after_page['entries_hex']), 'before_free': before_page['free'],
                                    'after_free': after_page['free'], 'byte_delta': delta,
                                    'active_entries_equal': before_page['entries_hex'] == after_page['entries_hex']})
            summary = {'table': name, 'root': int(root), 'aliases': tree['logical_names'],
                       'full_key_row_stream_unchanged': True, 'pages_unchanged': not changed,
                       'changed_pages': changed}
            other.append(summary)
            if changed:
                public_repartitions.append(summary)
    assert primary is not None
    current_comparison = capture.compare((old, b), (new, a))
    assert current_comparison['all_other_index_key_refs_unchanged']
    assert len(new) == len(old) + 2 * capture.PAGE_BYTES
    assert len(public_repartitions) == 1
    assert public_repartitions[0]['table'] == 'Children' and 'Wide' in public_repartitions[0]['aliases']
    map_changes = {}
    for name, before_map in b['maps'].items():
        after_map = a['maps'][name]
        cells = dict(before_map['bit_locations'])
        cells.update(after_map['bit_locations'])
        offsets = sorted({cell['offset'] for cell in cells.values() if cell['offset'] < len(old)})
        map_changes[name] = {
            'added_file_members': sorted(set(after_map['file_members']) - set(before_map['file_members'])),
            'removed_file_members': sorted(set(before_map['file_members']) - set(after_map['file_members'])),
            'existing_byte_changes': [{'offset': offset, 'before': old[offset], 'after': new[offset]}
                                      for offset in offsets if old[offset] != new[offset]],
        }
    assert map_changes['IndexChildren0Owned']['added_file_members'] == primary['new_eof_pages']
    for name, change in map_changes.items():
        if name not in ['IndexChildren0Owned', 'JackcessGlobalFree']:
            assert change == {'added_file_members': [], 'removed_file_members': [], 'existing_byte_changes': []}
    counters = []
    for name, table in b['metadata'].items():
        for label, index in table['indexes'].items():
            later = a['metadata'][name]['indexes'][label]
            assert index['historical_count'] == later['historical_count']
            assert index['entry_count'] == later['entry_count']
            counters.append({'table': name, 'index': label, 'physical': index['physical'],
                             'historical_count': index['historical_count'], 'counter_offset': index['counter_offset']})
    assert capture.pin(b['file']['path']) == item['planned']['original']
    return {'profile': item['profile'], 'before': b['file'], 'public_reference': a['file'],
            'old_service_actual': 'Unsupported(IndexedColumn), no candidate/output, source unchanged',
            'public_update_actual': 'success with enforced FK and disabled expressions',
            'full_typed_rows_relationships_and_other_raw_rows_preserved': True,
            'selected_fixed_id_patch_exact': True, 'primary_split': primary,
            'all_other_physical_key_row_streams': other,
            'historical_counters_and_live_entry_counts_preserved': counters,
            'map_changes': map_changes,
            'current_public_wrapper_comparison': current_comparison,
            'initial_hypothesis_failure': 'Unchanged Children.Wide semantic keys do NOT imply unchanged public page bytes: two leaves redistribute one entry and their parent separator changes.',
            'future_engineering_policy': 'Kernel may retain stronger untouched-index byte preservation; public repartition is not its raw-byte golden.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report_path = capture.common.path(args.report)
    output_path = capture.common.path(args.output)
    output_path.relative_to(ROOT / '.cache')
    assert not output_path.exists(), 'Use a fresh read-only diagnosis output'
    before_pin = capture.pin(report_path)
    report = json.loads(report_path.read_text())
    assert report['completed'] == 0 and report['command_count'] == 12
    for command in report['commands']:
        verify_pin(command['stdout'])
        verify_pin(command['stderr'])
    source = json.loads(Path(report['before_first_call']['path']).read_text())
    for pin in source['immutable']:
        verify_pin(pin)
    cases = [classify(item) for item in report['cases']]
    assert capture.pin(report_path) == before_pin
    output = {
        'scope': 'SEPARATE read-only diagnosis; initial-exit-1 report and hypothesis failure unchanged; no action reruns',
        'initial_report': before_pin, 'actual_native_refusals': 2, 'actual_public_successes': 2,
        'commands_in_initial_run': 12, 'cases': cases,
    }
    output_path.write_text(json.dumps(output, indent=2) + '\n')
    print(json.dumps({'diagnosis': capture.pin(output_path), 'cases': len(cases),
                      'native_or_Java_calls': 0, 'historical_execution': False}))


if __name__ == '__main__':
    main()
