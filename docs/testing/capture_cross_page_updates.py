"""Eight approved public UPDATE calls on fresh allocation-boundary descendants."""
import argparse
import hashlib
import os
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORK = Path('.cache/ace16-cross-page-update-next-20261026')
BOUNDARY = Path('.cache/ace16-slot-boundary-next-20261026/report.json')
ORIGINALS = Path('.cache/ace16-slot-originals-next-20261026')
JAR = Path('.cache/tablecraft-fixtures/jackcess-5.0.4.jar')
CLASSES = ORIGINALS / 'classes'
TABLE = 'RawLegacy'
COLUMN = 'Label'
IDENTITY = 1
PAGE_BYTES = 4096
FREE_OFFSET = 2
ROW_COUNT_OFFSET = 12
SLOT_START = 14
SLOT_BYTES = 2
OFFSET_MASK = 0x1FFF
POINTER_BYTES = 4
FORWARDED = 0x4000
TABLE_ROWS_OFFSET = 16
INDEX_HISTORY_OFFSET = 67
GROWN_VALUE = 'Ā' * 255
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common, facts
import delete_reference_maps as MAP


def number(raw, offset, width=SLOT_BYTES):
    return int.from_bytes(raw[offset:offset + width], 'little')


def frame(raw, page, slot):
    base = page * PAGE_BYTES
    word = number(raw, base + SLOT_START + slot * SLOT_BYTES)
    start = base + (word & OFFSET_MASK)
    end = base + PAGE_BYTES
    if slot:
        end = base + (number(raw, base + SLOT_START + (slot - 1) * SLOT_BYTES) & OFFSET_MASK)
    body = raw[start:end]
    return {'page': page, 'slot': slot, 'word': word, 'flags': word & ~OFFSET_MASK,
            'start': start, 'end': end, 'bytes': len(body), 'body_hex': body.hex(),
            'body_sha256': hashlib.sha256(body).hexdigest()}


def capture(path, label, commands):
    before = common.pin(path)
    java = ['java', '-Dstdout.encoding=UTF-8', '-cp', os.pathsep.join([str(JAR), str(CLASSES)])]
    getter = commands.run(java + ['SlotUpdateFixture', 'oracle', str(path)], label + '-getter')
    assert getter['status'] == 0 and not getter['timed_out']
    assert not Path(getter['stderr']['path']).read_bytes()
    metadata = facts.parse(Path(getter['stdout']['path']).read_text())
    probe = commands.run(java + ['SlotMapProbe', str(path), TABLE, str(IDENTITY)], label + '-maps')
    assert probe['status'] == 0 and not probe['timed_out']
    assert not Path(probe['stderr']['path']).read_bytes()
    lines = []
    location = None
    for line in Path(probe['stdout']['path']).read_text().splitlines():
        if line.startswith('LOCATION\t'):
            location = list(map(int, line.split('\t')[1:]))
        else:
            lines.append(line)
    assert location is not None and len(location) == 4
    parsed = MAP.parse('\n'.join(lines))
    raw = path.read_bytes()
    maps = {}
    for name, description in parsed['maps'].items():
        maps[name] = MAP.decode_map(raw, description, parsed['page_bytes'], parsed['page_count'])
    root_page, root_slot, target_page, target_slot = location
    definition = metadata[TABLE]['definition'] * PAGE_BYTES
    state = {
        'file': before,
        'file_pages': len(raw) // PAGE_BYTES,
        'getter': getter,
        'map_probe': probe,
        'root': frame(raw, root_page, root_slot),
        'target': frame(raw, target_page, target_slot),
        'row_getter': metadata[TABLE]['rows'][IDENTITY]['values'],
        'declared_rows': number(raw, definition + TABLE_ROWS_OFFSET, 4),
        'historical_index_rows': number(raw, definition + INDEX_HISTORY_OFFSET, 4),
        'maps': maps,
        'owned_data_page_headers': {},
    }
    for page in maps['TableOwned']['file_members']:
        state['owned_data_page_headers'][str(page)] = {
            'free_bytes': number(raw, page * PAGE_BYTES + FREE_OFFSET),
            'physical_slots': number(raw, page * PAGE_BYTES + ROW_COUNT_OFFSET),
            'prefix_hex': raw[page * PAGE_BYTES:page * PAGE_BYTES + SLOT_START].hex(),
        }
    assert common.pin(path) == before
    return raw, metadata, state


def compare(before, after):
    old_raw, old_metadata, old = before
    new_raw, new_metadata, new = after
    assert (old['root']['page'], old['root']['slot']) == (new['root']['page'], new['root']['slot'])
    assert old['declared_rows'] == new['declared_rows']
    assert old['historical_index_rows'] == new['historical_index_rows']
    all_others = True
    index_orders = True
    for name, table in old_metadata.items():
        assert set(table['rows']) == set(new_metadata[name]['rows'])
        for identity, row in table['rows'].items():
            if name == TABLE and identity == IDENTITY:
                continue
            all_others &= row == new_metadata[name]['rows'][identity]
            all_others &= table['raw_rows'][identity] == new_metadata[name]['raw_rows'][identity]
        index_orders &= table['indexes'] == new_metadata[name]['indexes']
    assert all_others and index_orders
    changes = {}
    for name in old['maps']:
        prior = old['maps'][name]
        current = new['maps'][name]
        cells = dict(prior['bit_locations'])
        cells.update(current['bit_locations'])
        offsets = sorted({cell['offset'] for cell in cells.values()})
        changes[name] = {
            'added_file_members': sorted(set(current['file_members']) - set(prior['file_members'])),
            'removed_file_members': sorted(set(prior['file_members']) - set(current['file_members'])),
            'changed_existing_bytes': [
                {'offset': offset, 'before': old_raw[offset], 'after': new_raw[offset]}
                for offset in offsets
                if offset < len(old_raw) and old_raw[offset] != new_raw[offset]
            ],
        }
    changed_pages = []
    for page in range(len(old_raw) // PAGE_BYTES):
        span = slice(page * PAGE_BYTES, (page + 1) * PAGE_BYTES)
        if old_raw[span] != new_raw[span]:
            changed_pages.append(page)
    same_target = (old['target']['page'], old['target']['slot']) == (new['target']['page'], new['target']['slot'])
    orphan_preserved = None
    if not same_target and old['root']['flags'] & FORWARDED:
        orphan_preserved = frame(new_raw, old['target']['page'], old['target']['slot']) == old['target']
        assert orphan_preserved
    return {'all_other_rows_and_raw_bodies_unchanged': all_others,
            'all_logical_index_metadata_and_orders_unchanged': index_orders,
            'table_and_historical_index_counters_unchanged': True,
            'root_identity_unchanged': True, 'same_target': same_target,
            'prior_orphan_hidden_target_preserved': orphan_preserved,
            'old_file_changed_pages': changed_pages,
            'file_bytes_added': len(new_raw) - len(old_raw), 'map_changes': changes}


def main():
    global WORK, BOUNDARY, JAR, CLASSES
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--boundary-report', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    args = parser.parse_args()
    BOUNDARY = common.path(args.boundary_report).relative_to(ROOT)
    WORK = common.path(args.workdir).relative_to(ROOT)
    JAR = common.path(args.jar).relative_to(ROOT)
    CLASSES = common.path(args.classes).relative_to(ROOT)
    WORK.relative_to('.cache')
    assert common.pin(JAR)['sha256'] == common.JAR_SHA256
    assert not WORK.exists(), 'Use a fresh output directory'
    assert (CLASSES / 'SlotUpdateFixture.class').is_file()
    assert (CLASSES / 'SlotMapProbe.class').is_file()
    WORK.mkdir()
    report_path = WORK / 'report.json'
    selectors_path = WORK / 'selectors.json'
    assert not report_path.exists() and not selectors_path.exists()
    boundary = json.loads(BOUNDARY.read_text())
    selectors = []
    inputs = []
    for profile in boundary['profiles']:
        for kind, position in [('true-hint', 3), ('false-hint', 4)]:
            original = profile['steps'][position]['created_copy']
            assert common.pin(original['path']) == original
            inputs.append(original)
            stem = profile['profile'] + '-' + kind
            grown = WORK / (stem + '-grown.accdb')
            selectors.extend([
                {'profile': profile['profile'], 'history': kind, 'action': 'grow',
                 'source': original['path'], 'output': grown.as_posix(),
                 'table': TABLE, 'ID': IDENTITY, 'column': COLUMN,
                 'expected_getter': 'NULL', 'replacement': GROWN_VALUE},
                {'profile': profile['profile'], 'history': kind, 'action': 'shrink',
                 'source': grown.as_posix(), 'output': (WORK / (stem + '-shrunk.accdb')).as_posix(),
                 'table': TABLE, 'ID': IDENTITY, 'column': COLUMN,
                 'expected_getter': 'String16:' + GROWN_VALUE.encode('utf-16-le').hex(),
                 'replacement': 'B'},
            ])
    assert len(selectors) == 8
    selectors_path.write_text(json.dumps(selectors, indent=2) + '\n')
    immutable = [Path(item['path']) for item in inputs]
    immutable.extend([BOUNDARY, JAR, Path('docs/testing/SlotUpdateFixture.java'),
                      CLASSES / 'SlotUpdateFixture.class', Path('docs/testing/SlotMapProbe.java'),
                      CLASSES / 'SlotMapProbe.class', Path(__file__).resolve().relative_to(ROOT),
                      Path(common.__file__).resolve().relative_to(ROOT),
                      Path(MAP.__file__).resolve().relative_to(ROOT),
                      Path(facts.__file__).resolve().relative_to(ROOT), selectors_path])
    before_pins = [common.pin(path) for path in immutable]
    report = {'scope': 'New public-wrapper capture of eight bounded UPDATE calls; not the historical private execution. No INSERT, DELETE, candidate or native Access execution.',
              'selectors': common.pin(selectors_path), 'pins_before': before_pins, 'cases': []}
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    commands = common.Capture(WORK)
    prior = {}
    for number, selection in enumerate(selectors):
        stem = f'{number:02}-{selection["profile"]}-{selection["history"]}-{selection["action"]}'
        item = {'selection': selection}
        report['cases'].append(item)
        source = Path(selection['source'])
        target = Path(selection['output'])
        assert not target.exists()
        try:
            key = (selection['profile'], selection['history'])
            before = prior[key] if selection['action'] == 'shrink' else capture(source, stem + '-before', commands)
            item['before'] = before[2]
            assert before[2]['row_getter'][1] == selection['expected_getter']
            if selection['action'] == 'grow':
                root_page = before[2]['root']['page']
                root_header = before[2]['owned_data_page_headers'][str(root_page)]
                assert root_header['free_bytes'] == 139
                assert root_header['physical_slots'] == 21
                hints = before[2]['maps']['TableFreeSpace']['file_members']
                assert (root_page in hints) == (selection['history'] == 'true-hint')
            token = 'text16:' + selection['replacement'].encode('utf-16-le').hex()
            argv = ['java', '-cp', os.pathsep.join([str(JAR), str(CLASSES)]), 'SlotUpdateFixture',
                    'update', str(source), str(target), TABLE, str(IDENTITY), COLUMN, token]
            item['action'] = commands.run(argv, stem + '-action')
            assert item['action']['status'] == 0 and not item['action']['timed_out']
            assert target.is_file()
            after = capture(target, stem + '-after', commands)
            item['after'] = after[2]
            assert after[2]['row_getter'][1] == 'String16:' + selection['replacement'].encode('utf-16-le').hex()
            item['comparison'] = compare(before, after)
            assert common.pin(source) == before[2]['file']
            prior[key] = after
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
    report['immutable_inputs_unchanged'] = report['pins_after'] == before_pins
    report['public_update_calls'] = sum('action' in case for case in report['cases'])
    report['command_count'] = len(commands.commands)
    report['completed'] = sum(case.get('complete', False) for case in report['cases'])
    report_path.write_text(json.dumps(report, indent=2) + '\n')
    print('TERMINAL', report['completed'], report['public_update_calls'], report['command_count'], flush=True)
    return 0 if report['completed'] == 8 and report['immutable_inputs_unchanged'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
