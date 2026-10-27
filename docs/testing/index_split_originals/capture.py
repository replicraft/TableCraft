"""Prepared two-profile split discriminator; no calls without explicit execution."""
import argparse
import hashlib
import json
import os
import subprocess
import sys
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common
import capture_delete_trees as trees
import delete_reference_maps as maps

PLAN_SHA256 = 'bffbf7831ae1911fdfb3b3b5b583f27c59bd08757e8b48bce0c6b99585a25006'
JAR = Path('.cache/tablecraft-fixtures/jackcess-5.0.4.jar')
PUBLIC_GETTER = Path('docs/testing/BatchDeleteFixture.java')
PAGE_BYTES = 4096
SLOT_START = 14
SLOT_BYTES = 2
PHYSICAL_OFFSET_MASK = 0x1fff
REFERENCE_BYTES = 4
CHILD_BYTES = 4
INDEX_NODE = 3
INDEX_LEAF = 4
INDEX_MASK_OFFSET = 27
INDEX_MASK_BYTES = 453
COLUMN_COUNT_BYTES = 2
INT32_BYTES = 4
EXPECTED_ROWS = 500
REPLACEMENT = 2147483647


def pin(path):
    return common.pin(path)


def verify_pin(expected):
    assert pin(expected['path']) == expected


def parse(text):
    result = {}
    relationships = []
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag == 'RELATION':
            relationships.append(fields[1:])
        elif tag == 'TABLE':
            keys = ['live_rows', 'definition_page', 'page_bytes', 'count_offset',
                    'row_start', 'mask_offset', 'mask_bytes']
            result[fields[1]] = {
                'geometry': dict(zip(keys, map(int, fields[2:]), strict=True)),
                'columns': [], 'rows': {}, 'locations': {}, 'indexes': {},
            }
        elif tag == 'COLUMN':
            result[fields[1]]['columns'].append(fields[2:])
        elif tag == 'LOCATION':
            result[fields[1]]['locations'][int(fields[2])] = list(map(int, fields[3:]))
        elif tag == 'ROW':
            result[fields[1]]['rows'][int(fields[2])] = fields[3:]
        elif tag == 'INDEX':
            result[fields[1]]['indexes'][fields[2]] = {
                'physical': int(fields[3]), 'historical_count': int(fields[4]),
                'counter_offset': int(fields[5]), 'entry_count': int(fields[6]),
                'owned_pages': int(fields[7]), 'unique': fields[8],
                'ignore_nulls': fields[9], 'columns': [], 'members': [],
            }
        elif tag == 'INDEX_COLUMN':
            result[fields[1]]['indexes'][fields[2]]['columns'].append(fields[3:])
        elif tag == 'ENTRY':
            result[fields[1]]['indexes'][fields[2]]['members'].append(int(fields[3]))
        else:
            raise AssertionError('Unknown getter record: ' + tag)
    return result, relationships


def body(raw, page, slot):
    start = page * PAGE_BYTES
    word = int.from_bytes(raw[start + SLOT_START + SLOT_BYTES * slot:
                              start + SLOT_START + SLOT_BYTES * (slot + 1)], 'little')
    low = start + (word & PHYSICAL_OFFSET_MASK)
    if slot:
        prior = int.from_bytes(raw[start + SLOT_START + SLOT_BYTES * (slot - 1):
                                   start + SLOT_START + SLOT_BYTES * slot], 'little')
        high = start + (prior & PHYSICAL_OFFSET_MASK)
    else:
        high = start + PAGE_BYTES
    return {'page': page, 'slot': slot, 'word': word, 'start': low, 'end': high,
            'hex': raw[low:high].hex()}


def geometry(raw, tables, map_info):
    output = {}
    for name, table in tables.items():
        decoded = trees.pages(raw, table)
        identities = {(page << 8) | slot: identity
                      for identity, (page, slot) in table['locations'].items()}
        owner = {'definition': table['geometry']['definition_page'], 'trees': {}}
        for label, index in table['indexes'].items():
            root = map_info['indexes'][name + '/' + label]['root']
            if str(root) in owner['trees']:
                owner['trees'][str(root)]['logical_names'].append(label)
                continue
            visited = set()
            partitions = []

            def walk(number, depth):
                assert number not in visited
                visited.add(number)
                page = decoded[number]
                entries = [bytes.fromhex(value) for value in page['entries_hex']]
                assert page['height'] + depth == decoded[root]['height']
                if page['kind'] == INDEX_LEAF:
                    partitions.append({'page': number, 'entries': len(entries)})
                    return entries
                assert page['kind'] == INDEX_NODE
                result = []
                for entry in entries:
                    child = int.from_bytes(entry[-CHILD_BYTES:], 'big')
                    nested = walk(child, depth + 1)
                    assert nested and entry[:-CHILD_BYTES] == nested[-1]
                    result.extend(nested)
                if page['tail']:
                    result.extend(walk(page['tail'], depth + 1))
                return result

            entries = walk(root, 0)
            order = [identities[int.from_bytes(entry[-REFERENCE_BYTES:], 'big')] for entry in entries]
            assert order == index['members']
            owner['trees'][str(root)] = {
                'logical_names': [label], 'height': decoded[root]['height'],
                'pages': sorted(visited), 'leaf_partitions': partitions,
                'entry_hex': [entry.hex() for entry in entries],
                'full_key_row_sha256': hashlib.sha256(b''.join(entries)).hexdigest(),
                'page_headers': {
                    str(number): raw[number * PAGE_BYTES:number * PAGE_BYTES + INDEX_MASK_OFFSET].hex()
                    for number in sorted(visited)
                },
            }
        output[name] = owner
    return output


def checked_call(commands, argv, name):
    command = commands.run(argv, name)
    assert command['status'] == 0 and not command['timed_out'], name
    assert not common.path(command['stderr']['path']).read_bytes(), name
    return command


def capture(path, label, commands, jar):
    before = pin(path)
    java = ['java', '-Dstdout.encoding=UTF-8', '-cp', str(jar), str(PUBLIC_GETTER)]
    getter = checked_call(commands, java + ['oracle', str(path)], label + '-getter')
    probe = checked_call(commands, java + ['maps', str(path)], label + '-maps')
    metadata, relations = parse(common.path(getter['stdout']['path']).read_text())
    assert relations == [['Parents_Children', 'Parents', 'Children', 'true', 'false', 'false', 'false']]
    raw = common.path(path).read_bytes()
    map_info = maps.parse(common.path(probe['stdout']['path']).read_text())
    decoded_maps = {
        name: maps.decode_map(raw, item, PAGE_BYTES, len(raw) // PAGE_BYTES)
        for name, item in map_info['maps'].items()
    }
    rows = {
        name: {str(identity): body(raw, *location) for identity, location in table['locations'].items()}
        for name, table in metadata.items()
    }
    result = {'file': before, 'getter': getter, 'map_probe': probe, 'metadata': metadata,
              'relations': relations, 'map_info': map_info, 'maps': decoded_maps,
              'geometry': geometry(raw, metadata, map_info), 'raw_rows': rows}
    assert pin(path) == before
    return raw, result


def compare(before, after):
    old, b = before
    new, a = after
    assert b['relations'] == a['relations']
    expected_rows = dict(b['metadata']['Children']['rows'])
    expected_rows[REPLACEMENT] = expected_rows.pop(1)
    assert a['metadata']['Children']['rows'] == expected_rows
    assert a['metadata']['Children']['geometry']['live_rows'] == EXPECTED_ROWS
    for name in ['Parents', 'Sentinel']:
        assert b['metadata'][name] == a['metadata'][name]
    old_selected = b['raw_rows']['Children']['1']
    selected = a['raw_rows']['Children'][str(REPLACEMENT)]
    assert (old_selected['page'], old_selected['slot']) == (selected['page'], selected['slot'])
    old_body = bytes.fromhex(old_selected['hex'])
    expected_body = (old_body[:COLUMN_COUNT_BYTES] + REPLACEMENT.to_bytes(INT32_BYTES, 'little')
                     + old_body[COLUMN_COUNT_BYTES + INT32_BYTES:])
    assert bytes.fromhex(selected['hex']) == expected_body
    for name, rows in b['raw_rows'].items():
        for identity, row in rows.items():
            if name == 'Children' and identity == '1':
                continue
            assert a['raw_rows'][name][identity] == row
    primary_changes = []
    unchanged_pages = set()
    public_rewritten_pages = []
    for name, owner in b['geometry'].items():
        for old_root, tree in owner['trees'].items():
            label = tree['logical_names'][0]
            next_root = a['map_info']['indexes'][name + '/' + label]['root']
            updated = a['geometry'][name]['trees'][str(next_root)]
            if name == 'Children' and label == 'PrimaryID':
                expected_entries = [bytes.fromhex(entry) for entry in tree['entry_hex']]
                removed = expected_entries.pop(0)
                assert removed[:5] == bytes.fromhex('7f80000001')
                expected_entries.append(bytes.fromhex('7fffffffff') + removed[-REFERENCE_BYTES:])
                assert updated['entry_hex'] == [entry.hex() for entry in expected_entries]
                primary_changes.append({'old_root': int(old_root), 'new_root': next_root,
                                        'before': tree, 'after': updated})
            else:
                assert updated['entry_hex'] == tree['entry_hex']
                assert updated['pages'] == tree['pages']
                for page in tree['pages']:
                    before_page = old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                    after_page = new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                    if before_page == after_page:
                        unchanged_pages.add(page)
                    else:
                        public_rewritten_pages.append({
                            'table': name, 'physical_root': int(old_root),
                            'logical_names': tree['logical_names'], 'page': page,
                            'before_sha256': hashlib.sha256(before_page).hexdigest(),
                            'after_sha256': hashlib.sha256(after_page).hexdigest(),
                        })
    changed_maps = {}
    for name, prior in b['maps'].items():
        current = a['maps'][name]
        cells = dict(prior['bit_locations'])
        cells.update(current['bit_locations'])
        offsets = sorted({cell['offset'] for cell in cells.values() if cell['offset'] < len(old)})
        changed_maps[name] = {
            'added_file_members': sorted(set(current['file_members']) - set(prior['file_members'])),
            'removed_file_members': sorted(set(prior['file_members']) - set(current['file_members'])),
            'changed_existing_bytes': [
                {'offset': offset, 'before': old[offset], 'after': new[offset]}
                for offset in offsets if old[offset] != new[offset]
            ],
        }
    counters = []
    for name, table in b['metadata'].items():
        for label, index in table['indexes'].items():
            later = a['metadata'][name]['indexes'][label]
            counters.append({'table': name, 'index': label, 'physical': index['physical'],
                             'before': index['historical_count'], 'after': later['historical_count']})
    return {'file_bytes_before': len(old), 'file_bytes_after': len(new),
            'selected_fixed_record_patch_exact': True, 'all_other_row_bodies_unchanged': True,
            'all_other_index_key_refs_unchanged': True,
            'other_index_pages_byte_identical': sorted(unchanged_pages),
            'public_rewritten_pages_with_unchanged_full_key_row_streams': public_rewritten_pages,
            'primary_replacement': primary_changes, 'map_changes': changed_maps,
            'historical_counters_observed_not_assumed': counters,
            'existing_changed_pages': [page for page in range(len(old) // PAGE_BYTES)
                                       if old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                                       != new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]]}


def verify_build(args):
    receipt = json.loads(common.path(args.build_receipt).read_text())
    assert receipt['compiled_revision'] == args.revision and receipt['exit_code'] == 0
    artifact = pin(args.artifact)
    assert artifact == receipt['frozen_example'] and artifact['sha256'] == args.artifact_sha256
    for source in receipt['source_pins']:
        value = subprocess.run(['git', 'show', args.revision + ':' + source['path']], cwd=ROOT,
                               check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
        assert len(value) == source['bytes']
        assert hashlib.sha256(value).hexdigest() == source['sha256']
    return {'receipt': pin(args.build_receipt), 'artifact': artifact,
            'compiled_revision': args.revision, 'selected_source_pins': receipt['source_pins']}


def execute(args, plan):
    build = verify_build(args)
    assert pin(args.jar)['sha256'] == common.JAR_SHA256
    directory = common.fresh_work(args.workdir)
    commands = common.Capture(directory)
    immutable = [pin(__file__), pin(args.plan), pin(args.jar), pin(args.reference_helper),
                 pin(PUBLIC_GETTER), pin('docs/testing/capture_delete_trees.py'),
                 pin('docs/testing/delete_reference_maps.py'), pin('docs/testing/slot_preview_originals/common.py')]
    immutable.extend(case['original'] for case in plan['cases'])
    before_path = directory / 'before-first-call.json'
    before_path.write_text(json.dumps({'build': build, 'plan': pin(args.plan), 'immutable': immutable}, indent=2) + '\n')
    report = {'scope': 'two measured old-service attempts and two fresh public UPDATE references; current wrapper records public semantic-preserving page rewrites; not the historical initial-exit-1 program',
              'before_first_call': pin(before_path), 'cases': [], 'completed': 0}
    output_path = directory / 'report.json'
    for case in plan['cases']:
        item = {'profile': case['profile'], 'planned': case, 'complete': False}
        report['cases'].append(item)
        try:
            source = common.path(case['original']['path'])
            verify_pin(case['original'])
            original = capture(source, case['profile'] + '-before', commands, args.jar)
            original_root = str(case['primary_root'])
            leaf = original[1]['geometry']['Children']['trees'][original_root]
            assert leaf['height'] == 0 and len(leaf['entry_hex']) == EXPECTED_ROWS
            candidate = directory / (case['profile'] + '-old-service-candidate.accdb')
            command = commands.run([
                str(common.path(args.artifact)), str(source), str(candidate), 'Children', 'ID',
                str(case['original_ordinal']), str(case['expected']), str(case['replacement']),
            ], case['profile'] + '-old-service')
            item['old_service'] = command
            item['old_service_candidate_exists'] = candidate.exists()
            if candidate.exists():
                item['old_service_candidate'] = pin(candidate)
            item['old_service_actual_error_text'] = common.path(command['stderr']['path']).read_text()
            item['old_service_atomic_refusal'] = command['status'] != 0 and not command['timed_out'] and not candidate.exists()
            reference = directory / (case['profile'] + '-public-reference.accdb')
            action = commands.run([
                'java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar), str(args.reference_helper),
                str(source), str(reference),
            ], case['profile'] + '-public-update')
            item['public_action'] = action
            item['public_copy'] = pin(reference) if reference.exists() else None
            assert action['status'] == 0 and not action['timed_out'], 'Public reference action failure'
            updated = capture(reference, case['profile'] + '-after', commands, args.jar)
            item['before'] = original[1]
            item['after'] = updated[1]
            item['comparison'] = compare(original, updated)
            verify_pin(case['original'])
            item['complete'] = True
            report['completed'] += 1
        except Exception as error:
            item['error'] = {'class': type(error).__name__, 'message': str(error), 'traceback': traceback.format_exc()}
        output_path.write_text(json.dumps(report, indent=2) + '\n')
    for expected in immutable:
        verify_pin(expected)
    report['immutable_inputs_unchanged'] = True
    report['commands'] = commands.commands
    report['command_count'] = len(commands.commands)
    output_path.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'report': pin(output_path), 'complete_profiles': report['completed']}))
    return 0 if report['completed'] == 2 else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--artifact-sha256')
    parser.add_argument('--revision')
    parser.add_argument('--build-receipt', type=Path)
    parser.add_argument('--jar', type=Path, default=JAR)
    parser.add_argument('--reference-helper', type=Path,
                        default=Path('docs/testing/index_split_originals/IndexSplitReference.java'))
    parser.add_argument('--workdir', type=Path)
    args = parser.parse_args()
    assert pin(args.plan)['sha256'] == PLAN_SHA256
    plan = json.loads(common.path(args.plan).read_text())
    for case in plan['cases']:
        verify_pin(case['original'])
    for field in ['wire_budget', 'future_cells', 'public_producer', 'original_geometry']:
        verify_pin(plan[field])
    if not args.execute:
        print('Frozen two-profile split action plan/input pins verified; no native or Java calls.')
        return 0
    for field in ['artifact', 'artifact_sha256', 'revision', 'build_receipt', 'workdir']:
        if getattr(args, field) is None:
            parser.error('--execute requires --' + field.replace('_', '-'))
    return execute(args, plan)


if __name__ == '__main__':
    raise SystemExit(main())
