"""Fixed 26 public Binary appends and eight UPDATEs; no native candidate calls."""
import argparse
import hashlib
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORK = Path('.cache/ace16-binary-allocation-next-20261026')
ORIGINALS = Path('.cache/ace16-slot-originals-next-20261026')
JAR = Path('.cache/tablecraft-fixtures/jackcess-5.0.4.jar')
CLASSES = WORK / 'classes'
TABLE = 'BinaryStorage'
COLUMN = 'Bytes'
FIRST_ID = 10001
APPENDS_PER_PROFILE = 13
CHECKPOINTS = (12, 13)
PAYLOAD_BYTES = 255
PATTERN_STEP = 31
GROWTH = bytes((position * PATTERN_STEP + 17) & 255 for position in range(PAYLOAD_BYTES))
SHRINK = bytes((position * PATTERN_STEP + 19) & 255 for position in range(7))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common
import capture_cross_page_updates as OBS

OBS.TABLE = TABLE
OBS.COLUMN = COLUMN
OBS.JAR = JAR
OBS.CLASSES = CLASSES


def freeze_selectors(originals):
    selectors = []
    for original in originals:
        profile = original['profile']
        current = original['input']['path']
        for step in range(1, APPENDS_PER_PROFILE + 1):
            identity = FIRST_ID + step - 1
            target = WORK / f'{profile}-append-{step:02}.accdb'
            selectors.append({
                'profile': profile,
                'action': 'INSERT-checkpoint',
                'step': step,
                'ID': identity,
                'source': current,
                'output': target.as_posix(),
                'payload_bytes': PAYLOAD_BYTES,
            })
            current = target.as_posix()
        for checkpoint in CHECKPOINTS:
            source = WORK / f'{profile}-append-{checkpoint:02}.accdb'
            grown = WORK / f'{profile}-checkpoint-{checkpoint}-grown.accdb'
            shrunk = WORK / f'{profile}-checkpoint-{checkpoint}-shrunk.accdb'
            selectors.extend([
                {'profile': profile, 'action': 'UPDATE-grow', 'checkpoint': checkpoint,
                 'ID': 1, 'source': source.as_posix(), 'output': grown.as_posix(),
                 'expected_getter': 'NULL', 'payload_hex': GROWTH.hex()},
                {'profile': profile, 'action': 'UPDATE-shrink', 'checkpoint': checkpoint,
                 'ID': 1, 'source': grown.as_posix(), 'output': shrunk.as_posix(),
                 'expected_getter': 'Binary:' + GROWTH.hex(), 'payload_hex': SHRINK.hex()},
            ])
    assert len(selectors) == 34
    path = WORK / 'selectors.json'
    assert not path.exists()
    path.write_text(json.dumps(selectors, indent=2) + '\n')
    return selectors


def append_proof(before, after, identity):
    old_raw, old_tables, old = before
    new_raw, new_tables, new = after
    assert old['root'] == new['root']
    assert old['row_getter'] == new['row_getter']
    assert new['declared_rows'] == old['declared_rows'] + 1
    assert new['historical_index_rows'] == old['historical_index_rows'] + 1
    for name, table in old_tables.items():
        current = new_tables[name]
        assert table['columns'] == current['columns']
        assert table['properties'] == current['properties']
        for row_id, row in table['rows'].items():
            assert current['rows'][row_id] == row
            assert current['raw_rows'][row_id] == table['raw_rows'][row_id]
        if name != TABLE:
            assert table == current
        else:
            assert set(current['rows']) == set(table['rows']) | {identity}
            assert current['indexes']['PrimaryID']['order'] == table['indexes']['PrimaryID']['order'] + [identity]
    row = new_tables[TABLE]['rows'][identity]
    payload = bytes((position * PATTERN_STEP + identity) & 255 for position in range(PAYLOAD_BYTES))
    assert row['values'][1] == 'Binary:' + payload.hex()
    physical = OBS.frame(new_raw, row['page'], row['slot'])
    assert physical['bytes'] == 268
    old_members = set(old['maps']['TableOwned']['file_members'])
    new_members = set(new['maps']['TableOwned']['file_members'])
    return {
        'all_prior_rows_and_raw_bodies_unchanged': True,
        'all_other_tables_unchanged': True,
        'root_ID1_unchanged': True,
        'primary_order_adds_exact_ID': True,
        'new_row_frame': physical,
        'new_owned_pages': sorted(new_members - old_members),
        'file_bytes_added': len(new_raw) - len(old_raw),
    }


def main():
    global WORK, JAR, CLASSES
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    args = parser.parse_args()
    facts_path = common.path(args.facts).relative_to(ROOT)
    inventory_path = common.path(args.inventory).relative_to(ROOT)
    WORK = common.path(args.workdir).relative_to(ROOT)
    JAR = common.path(args.jar).relative_to(ROOT)
    WORK.relative_to('.cache')
    assert common.pin(JAR)['sha256'] == common.JAR_SHA256
    assert not WORK.exists(), 'Use a fresh output directory'
    WORK.mkdir()
    CLASSES = WORK / 'classes'
    OBS.JAR = JAR
    OBS.CLASSES = CLASSES
    report_path = WORK / 'report.json'
    assert not report_path.exists()
    assert not (WORK / 'commands.jsonl').exists()
    originals = json.loads(facts_path.read_text())
    for original in originals:
        assert common.pin(original['input']['path']) == original['input']
    selectors = freeze_selectors(originals)
    immutable = [Path(original['input']['path']) for original in originals]
    immutable.extend([
        facts_path, inventory_path, JAR, Path('docs/testing/BinaryBoundaryAppend.java'),
        Path(__file__).resolve().relative_to(ROOT), WORK / 'selectors.json',
        Path('docs/testing/SlotUpdateFixture.java'), Path('docs/testing/SlotMapProbe.java'),
        Path(OBS.__file__).resolve().relative_to(ROOT),
        Path(common.__file__).resolve().relative_to(ROOT),
    ])
    before_pins = [common.pin(path) for path in immutable]
    commands = common.Capture(WORK)
    report = {
        'scope': 'New public-wrapper capture of 26 INSERT checkpoints and eight UPDATE actions; not the historical private execution. No native candidates.',
        'selectors': common.pin(WORK / 'selectors.json'),
        'pins_before': before_pins,
        'cases': [],
    }
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    CLASSES.mkdir()
    compilation = commands.run([
        'javac', '-cp', str(JAR), '-d', str(CLASSES),
        'docs/testing/BinaryBoundaryAppend.java',
        'docs/testing/SlotUpdateFixture.java', 'docs/testing/SlotMapProbe.java',
    ], 'compile')
    report['compilation'] = compilation
    assert compilation['status'] == 0 and not compilation['timed_out']
    class_pins = [common.pin(path) for path in sorted(CLASSES.glob('*.class'))]
    report['compiled_classes'] = class_pins
    snapshots = {}
    for original in originals:
        profile = original['profile']
        snapshots[original['input']['path']] = OBS.capture(
            Path(original['input']['path']), profile + '-initial', commands,
        )
    for number, selection in enumerate(selectors):
        stem = f'{number:02}-{selection["profile"]}-{selection["action"]}'
        suffix = selection.get('step', selection.get('checkpoint'))
        stem += '-' + str(suffix)
        source = Path(selection['source'])
        target = Path(selection['output'])
        assert not target.exists()
        item = {'selection': selection}
        report['cases'].append(item)
        report_path.write_text(json.dumps(report, indent=2) + '\n')
        try:
            before = snapshots[source.as_posix()]
            assert common.pin(source) == before[2]['file']
            item['before'] = before[2]
            if selection['action'] == 'INSERT-checkpoint':
                argv = ['java', '-cp', os.pathsep.join([str(JAR), str(CLASSES)]),
                        'BinaryBoundaryAppend', str(source), str(target), str(selection['ID'])]
            else:
                assert before[2]['row_getter'][1] == selection['expected_getter']
                argv = ['java', '-cp', os.pathsep.join([str(JAR), str(CLASSES)]),
                        'SlotUpdateFixture', 'update', str(source), str(target), TABLE,
                        str(selection['ID']), COLUMN, 'binary:' + selection['payload_hex']]
            item['action'] = commands.run(argv, stem + '-action')
            assert item['action']['status'] == 0 and not item['action']['timed_out']
            assert target.is_file()
            after = OBS.capture(target, stem + '-after', commands)
            item['after'] = after[2]
            if selection['action'] == 'INSERT-checkpoint':
                item['comparison'] = append_proof(before, after, selection['ID'])
            else:
                assert after[2]['row_getter'][1] == 'Binary:' + selection['payload_hex']
                item['comparison'] = OBS.compare(before, after)
            assert common.pin(source) == before[2]['file']
            snapshots[target.as_posix()] = after
            item['complete'] = True
        except Exception as error:
            item['complete'] = False
            item['diagnostic'] = type(error).__name__ + ': ' + str(error)
        finally:
            if target.exists():
                item['created_copy'] = common.pin(target)
            report_path.write_text(json.dumps(report, indent=2) + '\n')
        print(stem, item['complete'], item.get('diagnostic', ''), flush=True)
    report['pins_after'] = [common.pin(path) for path in immutable]
    report['immutable_unchanged'] = before_pins == report['pins_after']
    assert [common.pin(item['path']) for item in class_pins] == class_pins
    report['command_count'] = len(commands.commands)
    report['completed'] = sum(case.get('complete', False) for case in report['cases'])
    report['public_row_actions'] = sum('action' in case for case in report['cases'])
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    print('TERMINAL', report['completed'], report['public_row_actions'], report['command_count'], flush=True)
    return 0 if report['completed'] == 34 and report['immutable_unchanged'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
