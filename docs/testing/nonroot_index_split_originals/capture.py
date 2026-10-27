"""Reproduce the historical bounded non-root observation on fresh outputs.

This maintained wrapper is distinct from the archived first execution. Its exact
raw-record assertion intentionally retains the historical LVAL hypothesis failure.
Use analyze.py and lval.py for saved-output diagnosis without repeating actions.
"""
import argparse
import hashlib
import json
import subprocess
import sys
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common
from index_split_originals.capture import body, geometry
import capture_delete_trees as trees
import delete_reference_maps as maps

PLAN = Path('docs/testing/nonroot_index_split_originals/plan.json')
PLAN_SHA256 = '44f35ae7dbd339d0a1f3d4f9bc8c2b9bc6e7eaf1a1e3fbc8216a8d92e3716f20'
REVISION = '0c0dc6d904bd112236a383c74b04192ee6c5f90f'
ARTIFACT_SHA256 = '8245c35b6b533dbdc8f6dec5fed74aab0c0fdaf0da73e5df529dd3053eae4709'
GETTER = Path('docs/testing/DeleteTreeFixture.java')
REFERENCE = Path('docs/testing/nonroot_index_split_originals/NonrootSplitReference.java')
PAGE_BYTES = 4096
COLUMN_COUNT_BYTES = 2
FIELD_BYTES = 4
KEY_BYTES = 5
ROW_REFERENCE_BYTES = 4
TABLE = 'Tree'
PRIMARY = 'PrimaryKey'
TABLES = ('Tree', 'Unindexed', 'Sentinel')
SOURCE_INPUTS = (
    GETTER,
    REFERENCE,
    Path('docs/testing/capture_delete_trees.py'),
    Path('docs/testing/delete_reference_maps.py'),
    Path('docs/testing/index_split_originals/capture.py'),
    Path('docs/testing/slot_preview_originals/common.py'),
)


def check_pin(expected):
    assert common.pin(expected['path']) == expected, expected['path']


def java(args, helper, *arguments):
    return ['java', '-Dstdout.encoding=UTF-8', '-cp', str(common.path(args.jar)),
            str(common.path(helper)), *map(str, arguments)]


def checked(commands, argv, label):
    result = commands.run(argv, label)
    assert result['status'] == 0 and not result['timed_out'], label
    assert not common.path(result['stderr']['path']).read_bytes(), label
    return result


def capture(args, source, label, commands):
    original = common.pin(source)
    getter = checked(commands, java(args, GETTER, 'oracle', source), label + '-getter')
    probe = checked(commands, java(args, REFERENCE, 'maps', source), label + '-maps')
    metadata = trees.parse(common.path(getter['stdout']['path']).read_text())
    raw = common.path(source).read_bytes()
    map_info = maps.parse(common.path(probe['stdout']['path']).read_text())
    decoded_maps = {
        name: maps.decode_map(raw, item, PAGE_BYTES, len(raw) // PAGE_BYTES)
        for name, item in map_info['maps'].items()
    }
    topology = geometry(raw, metadata, map_info)
    decoded_pages = {}
    for name, table in metadata.items():
        pages = trees.pages(raw, table)
        decoded_pages[name] = pages
        for tree in topology[name]['trees'].values():
            partitions = tree['leaf_partitions']
            for position, partition in enumerate(partitions):
                page = pages[partition['page']]
                previous = partitions[position - 1]['page'] if position else 0
                following = partitions[position + 1]['page'] if position + 1 < len(partitions) else 0
                assert (page['previous'], page['next']) == (previous, following)
    rows = {
        name: {str(identity): body(raw, *location)
               for identity, location in table['locations'].items()}
        for name, table in metadata.items()
    }
    assert common.pin(source) == original
    return raw, {
        'file': original,
        'getter': getter,
        'map_probe': probe,
        'metadata': metadata,
        'map_info': map_info,
        'maps': decoded_maps,
        'geometry': topology,
        'decoded_pages': decoded_pages,
        'raw_rows': rows,
    }


def compare(case, before, after):
    old, prior = before
    new, later = after
    expected = case['expected']
    replacement = case['replacement']
    expected_rows = dict(prior['metadata'][TABLE]['rows'])
    expected_rows[replacement] = expected_rows.pop(expected)
    assert later['metadata'][TABLE]['rows'] == expected_rows
    for name in TABLES:
        assert prior['metadata'][name]['columns'] == later['metadata'][name]['columns']
        assert prior['metadata'][name]['geometry']['live_rows'] == later['metadata'][name]['geometry']['live_rows']
        if name != TABLE:
            assert prior['metadata'][name]['rows'] == later['metadata'][name]['rows']
        for identity, row in prior['raw_rows'][name].items():
            destination = str(replacement) if name == TABLE and identity == str(expected) else identity
            actual = later['raw_rows'][name][destination]
            assert (row['page'], row['slot'], row['word']) == (actual['page'], actual['slot'], actual['word'])
            expected_body = bytes.fromhex(row['hex'])
            if name == TABLE and identity == str(expected):
                expected_body = (expected_body[:COLUMN_COUNT_BYTES]
                                 + replacement.to_bytes(FIELD_BYTES, 'little', signed=True)
                                 + expected_body[COLUMN_COUNT_BYTES + FIELD_BYTES:])
            assert bytes.fromhex(actual['hex']) == expected_body
    changed_indexes = []
    counters = []
    for name in TABLES:
        for root, tree in prior['geometry'][name]['trees'].items():
            label = tree['logical_names'][0]
            new_root = later['map_info']['indexes'][name + '/' + label]['root']
            current = later['geometry'][name]['trees'][str(new_root)]
            expected_entries = list(tree['entry_hex'])
            if name == TABLE and label == PRIMARY:
                reference = bytes.fromhex(case['record_reference_hex'])
                matches = [position for position, entry in enumerate(expected_entries)
                           if bytes.fromhex(entry)[-ROW_REFERENCE_BYTES:] == reference]
                assert len(matches) == 1
                encoded = bytes.fromhex(expected_entries.pop(matches[0]))
                assert encoded[:KEY_BYTES] == b'\x7f' + (expected ^ (1 << 31)).to_bytes(FIELD_BYTES, 'big')
                expected_entries.append((b'\x7f' + (replacement ^ (1 << 31)).to_bytes(FIELD_BYTES, 'big') + reference).hex())
                expected_entries.sort()
            assert current['entry_hex'] == expected_entries
            all_pages = set(tree['pages']) | set(current['pages'])
            changed_indexes.append({
                'table': name, 'logical_names': tree['logical_names'],
                'root_before': int(root), 'root_after': new_root,
                'height_before': tree['height'], 'height_after': current['height'],
                'leaves_before': tree['leaf_partitions'], 'leaves_after': current['leaf_partitions'],
                'added_pages': sorted(set(current['pages']) - set(tree['pages'])),
                'removed_pages': sorted(set(tree['pages']) - set(current['pages'])),
                'rewritten_existing_pages': [page for page in sorted(all_pages)
                                            if page * PAGE_BYTES < len(old)
                                            and old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                                            != new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]],
                'full_key_row_stream_verified': True,
            })
        for label, index in prior['metadata'][name]['indexes'].items():
            current = later['metadata'][name]['indexes'][label]
            counters.append({'table': name, 'index': label,
                             'before': index['historical_count'],
                             'after': current['historical_count']})
    changed_maps = {}
    for name, original in prior['maps'].items():
        current = later['maps'][name]
        cells = dict(original['bit_locations'])
        cells.update(current['bit_locations'])
        offsets = sorted({cell['offset'] for cell in cells.values() if cell['offset'] < len(old)})
        changed_maps[name] = {
            'added_members': sorted(set(current['file_members']) - set(original['file_members'])),
            'removed_members': sorted(set(original['file_members']) - set(current['file_members'])),
            'existing_byte_changes': [
                {'offset': offset, 'before': old[offset], 'after': new[offset]}
                for offset in offsets if old[offset] != new[offset]
            ],
        }
    return {
        'before_bytes': len(old), 'after_bytes': len(new),
        'all_typed_rows_and_payload_digests_verified': True,
        'selected_record_exact_fixed_patch': True,
        'all_other_raw_record_bodies_unchanged': True,
        'indexes': changed_indexes, 'maps': changed_maps,
        'observed_historical_counters': counters,
        'existing_changed_pages': [page for page in range(len(old) // PAGE_BYTES)
                                   if old[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
                                   != new[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]],
        'eof_pages': list(range(len(old) // PAGE_BYTES, len(new) // PAGE_BYTES)),
        'public_page_rewrites_observed_not_kernel_byte_goldens': True,
    }


def verify_inputs(args):
    assert common.pin(args.plan)['sha256'] == PLAN_SHA256
    plan = json.loads(common.path(args.plan).read_text())
    check_pin(plan['inventory'])
    for item in plan['source_pins']:
        check_pin(item)
    for case in plan['cases']:
        check_pin(case['original'])
        assert (case['table'], case['column'], case['snapshot_live_ordinal'], case['expected'],
                case['replacement']) == (TABLE, 'ID', 27, 1000, 2147483647)
    assert common.pin(args.jar)['sha256'] == common.JAR_SHA256
    build = json.loads(common.path(args.build_receipt).read_text())
    assert args.revision == REVISION
    assert build['compiled_revision'] == args.revision and build['exit_code'] == 0
    assert common.pin(args.artifact) == build['frozen_example']
    assert args.artifact_sha256 == ARTIFACT_SHA256
    assert build['frozen_example']['sha256'] == args.artifact_sha256
    for source in build['source_pins']:
        value = subprocess.run(['git', 'show', args.revision + ':' + source['path']], cwd=ROOT,
                               check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout
        assert len(value) == source['bytes']
        assert hashlib.sha256(value).hexdigest() == source['sha256']
    return plan


def execute(args, plan):
    directory = common.fresh_work(args.workdir)
    commands = common.Capture(directory)
    immutable = [common.pin(path) for path in [__file__, args.plan, args.jar, *SOURCE_INPUTS]]
    immutable.extend(case['original'] for case in plan['cases'])
    archive = directory / 'precall-sources'
    archive.mkdir()
    archives = []
    for position, item in enumerate(immutable):
        if item['path'].endswith(('.py', '.java', '.json')):
            destination = archive / (str(position) + '-' + Path(item['path']).name)
            destination.write_bytes(common.path(item['path']).read_bytes())
            archives.append(common.pin(destination))
    precall = directory / 'before-first-call.json'
    precall.write_text(json.dumps({'immutable': immutable, 'archives': archives,
                                  'build_receipt': common.pin(args.build_receipt),
                                  'artifact': common.pin(args.artifact)}, indent=2) + '\n')
    report = {'precall': common.pin(precall), 'cases': [], 'completed': 0,
              'scope': 'Two frozen root-split attempts and two public UPDATE actions; not native Access qualification'}
    output = directory / 'report.json'
    for position, case in enumerate(plan['cases']):
        label = 'V2016' if position == 0 else 'V2019'
        item = {'profile': label, 'planned': case, 'complete': False}
        report['cases'].append(item)
        try:
            source = common.path(case['original']['path'])
            before = capture(args, source, label + '-before', commands)
            item['before'] = before[1]
            primary = before[1]['geometry'][TABLE]['trees'][str(case['root'])]
            assert primary['height'] == 1 and len(primary['entry_hex']) == 1000
            assert before[1]['metadata'][TABLE]['locations'][1000] == tuple(divmod(int(case['record_reference_hex'], 16), 256))
            candidate = directory / (label + '-current-candidate.accdb')
            attempt = commands.run([
                str(common.path(args.artifact)), str(source), str(candidate), TABLE, 'ID',
                str(case['snapshot_live_ordinal']), str(case['expected']), str(case['replacement']),
            ], label + '-current-attempt')
            item['current_service'] = attempt
            item['actual_error'] = common.path(attempt['stderr']['path']).read_text()
            item['candidate'] = common.pin(candidate) if candidate.exists() else None
            item['atomic_refusal'] = (attempt['status'] != 0 and not attempt['timed_out']
                                      and not candidate.exists()
                                      and not common.path(attempt['stdout']['path']).read_bytes())
            reference = directory / (label + '-public-reference.accdb')
            action = commands.run(java(args, REFERENCE, 'update', source, reference), label + '-public-update')
            item['public_action'] = action
            item['public_copy'] = common.pin(reference) if reference.exists() else None
            assert action['status'] == 0 and not action['timed_out'], 'Public action failed'
            after = capture(args, reference, label + '-after', commands)
            item['after'] = after[1]
            item['comparison'] = compare(case, before, after)
            check_pin(case['original'])
            item['complete'] = True
            report['completed'] += 1
        except Exception as error:
            item['failure'] = {'class': type(error).__name__, 'message': str(error),
                               'traceback': traceback.format_exc()}
        finally:
            check_pin(case['original'])
            report['commands'] = commands.commands
            output.write_text(json.dumps(report, indent=2) + '\n')
    for item in immutable:
        check_pin(item)
    report['immutable_inputs_unchanged'] = True
    report['command_count'] = len(commands.commands)
    report['atomic_refusals'] = sum(item.get('atomic_refusal', False) for item in report['cases'])
    output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'report': common.pin(output), 'complete_profiles': report['completed']}))
    return 0 if report['completed'] == 2 and report['atomic_refusals'] == 2 else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, default=PLAN)
    parser.add_argument('--jar', type=Path, default=Path('.cache/tablecraft-fixtures/jackcess-5.0.4.jar'))
    parser.add_argument('--artifact', type=Path, default=Path('.cache/tablecraft-bin/ace16-root-split-20261027'))
    parser.add_argument('--build-receipt', type=Path,
                        default=Path('docs/testing/results/ace16-root-split-example-build-2026-10-27.json'))
    parser.add_argument('--freeze', type=Path, help='Caller-prepared current source manifest for execution')
    parser.add_argument('--revision', default=REVISION)
    parser.add_argument('--artifact-sha256', default=ARTIFACT_SHA256)
    parser.add_argument('--workdir', type=Path)
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    plan = verify_inputs(args)
    if args.execute:
        if args.freeze is None:
            parser.error('--execute requires --freeze for the current wrapper sources')
        frozen = json.loads(common.path(args.freeze).read_text())
        for item in frozen['files']:
            check_pin(item)
    if not args.execute:
        print('Exact two-case inputs and historical compiled pins verified; no native/JVM calls.')
        return 0
    if args.workdir is None:
        parser.error('--execute requires --workdir')
    return execute(args, plan)


if __name__ == '__main__':
    raise SystemExit(main())
