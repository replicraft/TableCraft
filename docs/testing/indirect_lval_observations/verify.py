"""Verify saved indirect-LVAL sources without JVM, native or database writes."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

DIRECTORY = Path(__file__).resolve().parent
sys.path.insert(0, str(DIRECTORY))
import capture

ROOT = capture.ROOT
STAGES = ('after-first', 'after-padding', 'before')


def normalized(value):
    return json.loads(json.dumps(value))


def verify(report_path):
    report = json.loads(report_path.read_text())
    assert report['completed'] and report['immutable_inputs_unchanged']
    assert len(report['commands']) == 13
    assert all(command['status'] == 0 and not command['timed_out'] for command in report['commands'])
    assert capture.common.pin(ROOT / report['freeze']['path']) == report['freeze']
    frozen = capture.validate(ROOT / report['freeze']['path'])
    assert frozen['recipe']['max_fixture_processes'] == 13

    unique_pins = {}

    def visit(value):
        if isinstance(value, dict):
            if {'path', 'bytes', 'sha256'} <= value.keys():
                key = value['path']
                assert key not in unique_pins or unique_pins[key] == value
                unique_pins[key] = value
            for item in value.values():
                visit(item)
        elif isinstance(value, list):
            for item in value:
                visit(item)

    visit(report)
    visit(frozen)
    for entry in unique_pins.values():
        assert capture.common.pin(ROOT / entry['path']) == entry

    counts = {'snapshots': 0, 'raw_row_bodies': 0, 'live_chunk_bodies': 0,
              'map_roles': 0, 'mutable_role_comparisons': 0}
    summaries = {}
    for profile in ('V2016', 'V2019'):
        previous = None
        for stage in STAGES:
            saved = report['profiles'][profile][stage]
            label = profile + '-' + stage + '-inspect'
            command = next(command for command in report['commands']
                           if Path(command['stdout']['path']).name == label + '.stdout')
            path = ROOT / saved['file']['path']
            current = capture.snapshot(path, ROOT / command['stdout']['path'], stage)
            assert normalized(current) == saved
            if previous is not None:
                capture.unchanged_previous(previous, current)
            previous = current

            raw = path.read_bytes()
            for rows in current['rows'].values():
                for row in rows.values():
                    final, _ = capture.LONG.final_frame(raw, row)
                    body = raw[final['start']:final['end']]
                    assert len(body) == row['bytes']
                    assert hashlib.sha256(body).hexdigest() == row['sha256']
                    counts['raw_row_bodies'] += 1
            for row in current['long_values'].values():
                for field in row['fields'].values():
                    counts['live_chunk_bodies'] += len(field['chunks'])

            owner = current['tables'][capture.SELECTED]['definition']
            mutable = ['GlobalFree'] + [f'Table:{owner}:{role}' for role in (
                'Owned', 'Hint', 'Column:1:Owned', 'Column:1:Hint')]
            claims = current['role_storage_claims']
            for role in mutable:
                first = claims[role]
                for other, second in claims.items():
                    if role == other:
                        continue
                    assert first['reference'] != second['reference']
                    assert max(first['row_start'], second['row_start']) >= min(first['row_end'], second['row_end'])
                    assert set(first['bitmaps']).isdisjoint({second['container']} | set(second['bitmaps']))
                    assert set(second['bitmaps']).isdisjoint({first['container']} | set(first['bitmaps']))
                    counts['mutable_role_comparisons'] += 1
            counts['snapshots'] += 1
            counts['map_roles'] += len(current['maps'])

        final = report['profiles'][profile]['before']
        geometry = report['profiles'][profile]['observed_geometry']
        assert geometry['memo_owned_type'] == 1 and geometry['memo_candidate_type'] == 0
        assert geometry['memo_owned_span'] == 716
        assert len(geometry['memo_owned_bitmap_pages']) == 1
        assert len({row['page'] for row in final['rows'][capture.PADDING].values()}) == 700
        assert {row['bytes'] for row in final['rows'][capture.PADDING].values()} == {3595}
        assert final['indexes'][capture.SELECTED]['IDAsc']['physical'] == final['indexes'][capture.SELECTED]['IDTwin']['physical']
        assert all(value['complete'] for value in final['declarations'].values())
        summaries[profile] = {'file': final['file'], 'geometry': geometry}

    return {'scope': 'saved-source verification only; no fixture processes',
            'fixture_processes_repeated': 0, 'saved_fixture_commands': 13,
            'unique_live_pins': len(unique_pins), 'counts': counts, 'profiles': summaries}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', type=Path, required=True)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.report), indent=2))


if __name__ == '__main__':
    main()
