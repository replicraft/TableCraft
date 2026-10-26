#!/usr/bin/env python3
"""Original deep/indirect DELETE references; expanded captures stay ignored."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

import capture_delete_trees as TREE
import delete_reference_maps as MAP

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
STATES = ('whole-child-subtree', 'retain-one-leaf', 'delete-all')
ROW_REFERENCE_BYTES = 4
ROW_REFERENCE_SLOT_BITS = 8
INDEX_NODE_KIND = 3
INDEX_LEAF_KIND = 4
INLINE_GLOBAL_CAPACITY_PAGES = 512
ROW_DELETED_FLAGS = 0xC000



def digest(data):
    return hashlib.sha256(data).hexdigest()


def label(path):
    return path.resolve().relative_to(ROOT).as_posix()


def parse_rows(text):
    result = {}
    for line in text.splitlines():
        cells = line.split('\t')
        tag, name = cells[:2]
        if tag == 'TABLE':
            names = (
                'live_rows', 'definition_page', 'page_bytes', 'count_offset',
                'row_start', 'mask_offset', 'mask_bytes',
            )
            if name in result:
                raise AssertionError('Duplicate table')
            result[name] = {
                'geometry': dict(zip(names, map(int, cells[2:]), strict=True)),
                'columns': [],
                'rows': {},
                'locations': {},
                'indexes': {},
            }
        elif tag == 'COLUMN':
            result[name]['columns'].append(cells[2:])
        elif tag == 'ROW':
            result[name]['rows'][int(cells[2])] = cells[3:]
        elif tag == 'LOCATION':
            result[name]['locations'][int(cells[2])] = tuple(map(int, cells[3:]))
        elif tag == 'INDEX':
            result[name]['indexes'][cells[2]] = {
                'physical': int(cells[3]),
                'historical_count': int(cells[4]),
                'counter_offset': int(cells[5]),
                'entry_count': int(cells[6]),
                'owned_pages': int(cells[7]),
                'unique': cells[8],
                'ignore_nulls': cells[9],
                'columns': [],
                'members': [],
            }
        elif tag == 'INDEX_COLUMN':
            result[name]['indexes'][cells[2]]['columns'].append(cells[3:])
        elif tag == 'ENTRY':
            result[name]['indexes'][cells[2]]['members'].append(int(cells[3]))
        else:
            raise AssertionError('Unexpected getter record')
    if set(result) != {'Deep', 'Preserved'}:
        raise AssertionError('Unexpected tables')
    if set(result['Deep']['indexes']) != {'PrimaryKey', 'Wide', 'WideAlias'}:
        raise AssertionError('Unexpected logical indexes')
    for item in result.values():
        if set(item['rows']) != set(item['locations']):
            raise AssertionError('Row/location inventories differ')
        if len(item['rows']) != item['geometry']['live_rows']:
            raise AssertionError('Getter live count differs')
        for index in item['indexes'].values():
            if index['ignore_nulls'] != 'false':
                raise AssertionError('Unexpected ignored-NULL index')
            if sorted(index['members']) != sorted(item['rows']):
                raise AssertionError('Getter index membership is incomplete')
    return result


def children(page):
    result = []
    if page['kind'] == INDEX_NODE_KIND:
        for encoded in page['entries_hex']:
            result.append(int.from_bytes(bytes.fromhex(encoded)[-ROW_REFERENCE_BYTES:], 'big'))
        if page['tail']:
            result.append(page['tail'])
    return result


def subtree_ids(number, raw_pages, info):
    locations = {}
    for identifier, (page, slot) in info['locations'].items():
        locations[(page << ROW_REFERENCE_SLOT_BITS) + slot] = identifier
    identifiers = []
    pending = [number]
    while pending:
        page = raw_pages[pending.pop()]
        if page['kind'] == INDEX_LEAF_KIND:
            for encoded in page['entries_hex']:
                reference = int.from_bytes(bytes.fromhex(encoded)[-ROW_REFERENCE_BYTES:], 'big')
                identifiers.append(locations[reference])
        else:
            pending.extend(children(page))
    return identifiers


def selections(baseline):
    info = baseline['metadata']['Deep']
    raw = baseline['raw_pages']
    wide = next(tree for tree in baseline['trees'] if 'Wide' in tree['logical_names'])
    if wide['height'] < 2:
        raise AssertionError('Actual public-writer tree is not deep enough')
    branch = children(raw[wide['root']])[0]
    if raw[branch]['kind'] != INDEX_NODE_KIND:
        raise AssertionError('Selected root child is not an internal subtree')
    leaf = min(wide['leaf_partitions'], key=lambda item: len(item['ids']))
    populations = set(info['rows'])
    return {
        'whole-child-subtree': set(subtree_ids(branch, raw, info)),
        'retain-one-leaf': populations - set(leaf['ids']),
        'delete-all': populations,
    }, {'child_subtree_page': branch, 'retained_original_leaf': leaf}


def verify_getters(baseline, actual, removed):
    for table, before in baseline['metadata'].items():
        expected = {}
        for identifier, row in before['rows'].items():
            if table != 'Deep' or identifier not in removed:
                expected[identifier] = row
        after = actual['metadata'][table]
        if after['rows'] != expected or list(after['rows']) != list(expected):
            raise AssertionError('Surviving population/payload/order differs')
        if after['columns'] != before['columns']:
            raise AssertionError('Declared columns differ')
        if set(after['indexes']) != set(before['indexes']):
            raise AssertionError('Logical index inventory differs')
        for name, index in before['indexes'].items():
            expected_members = []
            for identifier in index['members']:
                if table != 'Deep' or identifier not in removed:
                    expected_members.append(identifier)
            observed = after['indexes'][name]
            if observed['members'] != expected_members:
                raise AssertionError('Index encounter order differs')
            if observed['historical_count'] != index['historical_count']:
                raise AssertionError('Historical index count changed')


def verify_records(before, after, metadata, removed):
    records = 0
    for table, info in metadata.items():
        geometry = info['geometry']
        width = geometry['page_bytes']
        for identifier, (page, slot) in info['locations'].items():
            offset = page * width + geometry['row_start'] + slot * MAP.ROW_SLOT_BYTES
            word = MAP.unsigned(before, offset, MAP.ROW_SLOT_BYTES)
            start = page * width + (word & MAP.ROW_PAYLOAD_OFFSET_MASK)
            if slot == 0:
                end = (page + 1) * width
            else:
                previous = MAP.unsigned(before, offset - MAP.ROW_SLOT_BYTES, MAP.ROW_SLOT_BYTES)
                end = page * width + (previous & MAP.ROW_PAYLOAD_OFFSET_MASK)
            expected_word = word
            if table == 'Deep' and identifier in removed:
                expected_word |= ROW_DELETED_FLAGS
            if MAP.unsigned(after, offset, MAP.ROW_SLOT_BYTES) != expected_word:
                raise AssertionError('Slot flag/offset observation differs')
            if before[start:end] != after[start:end]:
                raise AssertionError('Original record body changed')
            records += 1
    return {'complete_original_records_checked': records, 'record_bodies_unchanged': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--family', choices=('Deep', 'Indirect'), required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--existing-references', action='store_true')
    args = parser.parse_args()
    for name in ('jar', 'inputs', 'report'):
        path = (ROOT / getattr(args, name)).resolve()
        label(path)
        setattr(args, name, path)
    try:
        args.report.relative_to(ROOT / '.cache')
    except ValueError:
        parser.error('Expanded captures must remain under ignored .cache')
    if args.report.exists():
        parser.error('Report must be fresh')
    if digest(args.jar.read_bytes()) != JAR_SHA256:
        parser.error('Reviewed JAR differs')
    helper = ROOT / f'docs/testing/{args.family}DeleteFixture.java'
    map_helper = ROOT / f'docs/testing/{args.family}UsageMapProbe.java'
    originals = []
    pins = [helper, map_helper, Path(__file__), args.jar, Path(TREE.__file__), Path(MAP.__file__)]
    for profile in ('V2016', 'V2019'):
        original = args.inputs / f'{args.family}-{profile}-before.accdb'
        originals.append(original)
        pins.append(original)
        if args.existing_references:
            for state in STATES:
                pins.append(args.inputs / f'{original.stem}-{state}.accdb')
    before_pins = {}
    for path in pins:
        before_pins[label(path)] = digest(path.read_bytes())
    commands = []
    observations = []
    failure = None
    log_path = args.report.with_suffix('.commands.jsonl')
    if log_path.exists():
        parser.error('Command capture must be fresh')

    def run(command):
        normalized = []
        for argument in command:
            normalized.append(label(argument) if isinstance(argument, Path) else argument)
        completed = subprocess.run(
            normalized, cwd=ROOT, capture_output=True, text=True, timeout=120,
        )
        record = {
            'command': normalized,
            'status': completed.returncode,
            'stdout': completed.stdout,
            'stderr': completed.stderr,
        }
        commands.append(record)
        with log_path.open('a') as output:
            output.write(json.dumps(record) + '\n')
        if completed.returncode or completed.stderr:
            raise AssertionError('Public reference helper failed')
        return completed.stdout

    def capture(path):
        metadata = parse_rows(run([
            'java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, helper, 'oracle', path,
        ]))
        maps = MAP.parse(run([
            'java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, map_helper, path,
        ]))
        data = path.read_bytes()
        decoded = {}
        for name, item in maps['maps'].items():
            decoded[name] = MAP.decode_map(data, item, maps['page_bytes'], maps['page_count'])
        maps['decoded_maps'] = decoded
        return {
            'path': label(path),
            'sha256': digest(data),
            'bytes': len(data),
            'metadata': metadata,
            'map_metadata': maps,
            'raw_pages': TREE.pages(data, metadata['Deep']),
        }

    try:
        for source in originals:
            baseline = capture(source)
            baseline['trees'] = TREE.topology(baseline['raw_pages'], baseline['metadata']['Deep'])
            if args.family == 'Indirect':
                global_map = baseline['map_metadata']['decoded_maps']['JackcessGlobalFree']
                if (
                    global_map['payload_type'] != MAP.MAP_INDIRECT
                    or baseline['map_metadata']['page_count'] <= INLINE_GLOBAL_CAPACITY_PAGES
                ):
                    raise AssertionError('Original does not have the requested existing-indirect profile')
            batches, proof = selections(baseline)
            proof['selected_ids'] = {}
            for state, identifiers in batches.items():
                proof['selected_ids'][state] = sorted(identifiers)
            baseline['selection_proofs'] = proof
            baseline['references'] = []
            observations.append(baseline)
            for state, removed in batches.items():
                output = args.inputs / f'{source.stem}-{state}.accdb'
                reference = {'path': label(output), 'state': state}
                baseline['references'].append(reference)
                if args.existing_references:
                    if not output.is_file():
                        raise AssertionError('Expected existing reference')
                else:
                    if output.exists():
                        raise AssertionError('Reference output must be fresh')
                    argument = 'all' if state == 'delete-all' else ','.join(map(str, sorted(removed)))
                    run(['java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, helper,
                         'delete', source, output, argument])
                reference.update(capture(output))
                reference['removed_rows'] = len(removed)
                verify_getters(baseline, reference, removed)
                reference['tree_observation_error'] = None
                try:
                    reference['trees'] = TREE.topology(
                        reference['raw_pages'], reference['metadata']['Deep'],
                    )
                except (AssertionError, KeyError) as error:
                    reference['tree_observation_error'] = str(error)
                reference['map_deltas'] = {}
                before_maps = baseline['map_metadata']['decoded_maps']
                after_maps = reference['map_metadata']['decoded_maps']
                for name, before in before_maps.items():
                    after = after_maps[name]
                    reference['map_deltas'][name] = {
                        'removed_members': sorted(set(before['file_members']) - set(after['file_members'])),
                        'added_members': sorted(set(after['file_members']) - set(before['file_members'])),
                    }
                before_bytes = source.read_bytes()
                after_bytes = output.read_bytes()
                reference['map_checks'] = MAP.verify_transition(
                    before_bytes, after_bytes, before_maps, after_maps,
                    reference['map_deltas'], baseline['map_metadata']['page_bytes'],
                )
                reference['record_checks'] = verify_records(
                    before_bytes, after_bytes, baseline['metadata'], removed,
                )
    except Exception as error:
        failure = str(error)
    after_pins = {}
    for path in pins:
        after_pins[label(path)] = digest(path.read_bytes())
    report = {
        'scope': 'Original public-builder/delete/getter observations; no native Access or candidate execution',
        'family': args.family,
        'existing_references_read_only': args.existing_references,
        'hashes_before': before_pins,
        'hashes_after': after_pins,
        'pins_unchanged': before_pins == after_pins,
        'commands': commands,
        'observations': observations,
        'failure': failure,
    }
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({
        'profiles': len(observations),
        'commands': len(commands),
        'pins_unchanged': report['pins_unchanged'],
        'failure': failure,
        'report_sha256': digest(args.report.read_bytes()),
    }))
    if failure or not report['pins_unchanged']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
