"""Verify retained source/failed-action/getter/corrected-action evidence read-only."""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from usage_map_growth import model


def normalized(value):
    return json.loads(json.dumps(value))


def load(path):
    return json.loads(path.read_text())


def verify(first_path, classification_path, corrected_path):
    first = load(first_path)
    classified = load(classification_path)
    corrected = load(corrected_path)
    assert not first['completed'] and not first['qualified_success']
    assert len(first['commands']) == 14
    assert first['commands'][-1]['status'] == 1
    assert not first['commands'][-1]['timed_out']
    for command in first['commands'][:-1]:
        assert command['status'] == 0 and not command['timed_out']
    assert classified['qualified_classification'] and classified['first_update_status'] == 1
    assert len(classified['commands']) == 1 and classified['commands'][0]['status'] == 0
    assert classified['protected_files_unchanged'] and classified['frozen_sources_and_classes_unchanged']
    assert corrected['qualified_success'] and corrected['completed']
    assert len(corrected['commands']) == 3 and corrected['immutable_inputs_unchanged']
    for command in corrected['commands']:
        assert command['status'] == 0 and not command['timed_out']

    pins = {}

    def collect(value):
        if isinstance(value, dict):
            if {'path', 'bytes', 'sha256'} <= value.keys():
                previous = pins.get(value['path'])
                assert previous is None or previous == value
                pins[value['path']] = value
            for child in value.values():
                collect(child)
        elif isinstance(value, list):
            for child in value:
                collect(child)

    for report in (first, classified, corrected):
        collect(report)
        if 'freeze' in report:
            collect(load(ROOT / report['freeze']['path']))
    for entry in pins.values():
        assert model.common.pin(entry['path']) == entry

    first_directory = first_path.parent
    source_states = {}
    source_raw_rows = 0
    ordinary_pointer_creations = {}
    for profile in ('V2016', 'V2019'):
        states = {}
        for stage in ('empty', 'after-target', 'before'):
            saved = first['profiles'][profile][stage]
            output = first_directory / (profile + '-' + stage + '-inspect.stdout')
            current = model.snapshot(ROOT / saved['file']['path'], output, stage, profile)
            assert normalized(current) == saved
            states[stage] = current
            for rows in current['rows'].values():
                source_raw_rows += len(rows)
        assert model.source_gate(states['before']) == first['profiles'][profile]['prerequisites']
        assert states['after-target']['rows'][model.TARGET] == states['before']['rows'][model.TARGET]
        assert states['after-target']['trees'] == states['before']['trees']
        padding_pages = {row['page'] for row in states['before']['rows'][model.PADDING].values()}
        assert len(padding_pages) == 700
        owner = states['empty']['tables'][model.TARGET]['definition']
        role = f'Table:{owner}:Owned'
        empty = states['empty']['maps'][role]
        filled = states['after-target']['maps'][role]
        assert empty['payload_type'] == filled['payload_type'] == 1
        assert empty['bitmap_pages'] == [] and len(filled['bitmap_pages']) == 1
        for field in ('map_page', 'map_slot', 'row_start', 'row_end'):
            assert empty[field] == filled[field]
        filled_raw = (ROOT / states['after-target']['file']['path']).read_bytes()
        bitmap = filled['bitmap_pages'][0]
        expected = bytearray(4096)
        expected[:4] = b'\x05\x01\x00\x00'
        data_pages = sorted({row['page'] for row in states['after-target']['rows'][model.TARGET].values()})
        assert filled['file_members'] == data_pages
        for page in data_pages:
            expected[4 + page // 8] |= 1 << (page % 8)
        assert filled_raw[bitmap * 4096:(bitmap + 1) * 4096] == expected
        ordinary_pointer_creations[profile] = {'bitmap': bitmap, 'data_pages': data_pages}
        source_states[profile] = states['before']

    partial_file = ROOT / classified['before'][0]['path']
    partial_stdout = ROOT / classified['commands'][0]['stdout']['path']
    partial = model.snapshot(partial_file, partial_stdout, 'after', 'V2016')
    partial_transition = model.transition(source_states['V2016'], partial)
    assert partial_transition == classified['raw_transition']
    corrected_stdout = ROOT / corrected['commands'][-1]['stdout']['path']
    after = model.snapshot(ROOT / corrected['output']['path'], corrected_stdout, 'after', 'V2019')
    corrected_transition = model.transition(source_states['V2019'], after)
    assert corrected_transition == corrected['transition']

    return {'scope': 'Saved artifact/raw proof verification only; no fixture commands repeated',
            'fixture_commands_repeated': 0, 'unique_live_pins': len(pins),
            'actual_fixture_commands': 18, 'source_successes': 13,
            'failed_mutating_V2016_action': 1, 'successful_readonly_partial_classification': 1,
            'corrected_V2019_commands': 3, 'source_snapshots': 6,
            'source_raw_row_comparisons': source_raw_rows,
            'post_update_raw_row_comparisons': sum(len(rows) for state in (partial, after) for rows in state['rows'].values()),
            'ordinary_pointer_creations': ordinary_pointer_creations,
            'V2016_validated_partial_mutation': partial_transition,
            'V2019_successful_reference': corrected_transition}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--first-report', type=Path, required=True)
    parser.add_argument('--partial-classification', type=Path, required=True)
    parser.add_argument('--v2019-report', type=Path, required=True)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.first_report, arguments.partial_classification, arguments.v2019_report), indent=2))


if __name__ == '__main__':
    main()
