#!/usr/bin/env python3
"""Original tree DELETE references with independent raw partitions/separators."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
TABLES = ('Tree', 'Unindexed', 'Sentinel')
INDEXES = {'PrimaryKey', 'GroupAsc', 'GroupTwin', 'GroupDesc', 'Compound', 'UniqueScore'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def label(path):
    return Path(path).resolve().relative_to(ROOT).as_posix()


def unsigned(data, offset, width=4, order='little'):
    return int.from_bytes(data[offset:offset + width], order)


def parse(text):
    result = {}
    for line in text.splitlines():
        cells = line.split('\t')
        tag, table = cells[:2]
        if tag == 'TABLE':
            names = ('live_rows', 'definition_page', 'page_bytes', 'count_offset',
                     'row_start', 'mask_offset', 'mask_bytes')
            result[table] = {
                'geometry': dict(zip(names, map(int, cells[2:]), strict=True)),
                'columns': [],
                'rows': {},
                'locations': {},
                'indexes': {},
            }
        elif tag == 'COLUMN':
            result[table]['columns'].append(cells[2:])
        elif tag == 'ROW':
            result[table]['rows'][int(cells[2])] = cells[3:]
        elif tag == 'LOCATION':
            result[table]['locations'][int(cells[2])] = tuple(map(int, cells[3:]))
        elif tag == 'INDEX':
            result[table]['indexes'][cells[2]] = {
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
            result[table]['indexes'][cells[2]]['columns'].append(cells[3:])
        elif tag == 'ENTRY':
            result[table]['indexes'][cells[2]]['members'].append(int(cells[3]))
        else:
            raise AssertionError('Unexpected getter record')
    if set(result) != set(TABLES) or set(result['Tree']['indexes']) != INDEXES:
        raise AssertionError('Unexpected table/index declaration inventory')
    for table, item in result.items():
        expected = set(item['rows'])
        if set(item['locations']) != expected:
            raise AssertionError('Row location inventory differs')
        if len(expected) != item['geometry']['live_rows']:
            raise AssertionError('Stored getter count differs')
        for declaration in item['indexes'].values():
            if declaration['ignore_nulls'] != 'false':
                raise AssertionError('Unexpected ignored-NULL declaration')
            if sorted(declaration['members']) != sorted(expected):
                raise AssertionError('Incomplete getter index membership')
            if declaration['entry_count'] != len(expected):
                raise AssertionError('Getter index entry count differs')
    return result


def pages(data, info):
    geometry = info['geometry']
    width = geometry['page_bytes']
    mask_start = geometry['mask_offset']
    entry_start = mask_start + geometry['mask_bytes']
    result = {}
    for number in range(len(data) // width):
        page = data[number * width:(number + 1) * width]
        if page[0] not in (3, 4) or page[1] != 1:
            continue
        if unsigned(page, 4) != geometry['definition_page']:
            continue
        ends = []
        for byte_index, value in enumerate(page[mask_start:entry_start]):
            for bit in range(8):
                if value & (1 << bit):
                    ends.append(byte_index * 8 + bit)
        prefix = unsigned(page, 24, 2)
        previous = 0
        first = b''
        entries = []
        for end in ends:
            if not previous < end <= width - entry_start:
                raise AssertionError('Invalid raw entry boundary')
            encoded = page[entry_start + previous:entry_start + end]
            if previous == 0:
                first = encoded
            else:
                if prefix > len(first):
                    raise AssertionError('Invalid raw shared prefix')
                encoded = first[:prefix] + encoded
            minimum = 8 if page[0] == 3 else 4
            if len(encoded) < minimum:
                raise AssertionError('Incomplete raw key/reference/child')
            entries.append(encoded.hex())
            previous = end
        free = unsigned(page, 2, 2)
        if free != width - entry_start - previous:
            raise AssertionError('Raw free/entry extent differs')
        result[number] = {
            'kind': page[0],
            'owner': unsigned(page, 4),
            'stamp_hex': page[8:12].hex(),
            'previous': unsigned(page, 12),
            'next': unsigned(page, 16),
            'tail': unsigned(page, 20),
            'prefix': prefix,
            'height': page[26],
            'free': free,
            'boundaries': ends,
            'entries_hex': entries,
            'sha256': digest(page),
        }
    return result


def topology(raw_pages, info):
    locations = {}
    for identifier, (page, slot) in info['locations'].items():
        locations[page * 256 + slot] = identifier
    incoming = set()
    for page in raw_pages.values():
        if page['kind'] == 3:
            for encoded in page['entries_hex']:
                incoming.add(int.from_bytes(bytes.fromhex(encoded)[-4:], 'big'))
            if page['tail']:
                incoming.add(page['tail'])
    if not incoming <= set(raw_pages):
        raise AssertionError('Raw child missing from table-owned pages')
    roots = sorted(set(raw_pages) - incoming)
    visited = set()

    def walk(number, active):
        if number in active or number in visited:
            raise AssertionError('Raw cyclic/shared child')
        visited.add(number)
        page = raw_pages[number]
        active = active | {number}
        entries = []
        for encoded in page['entries_hex']:
            entries.append(bytes.fromhex(encoded))
        if page['kind'] == 4:
            if page['height'] != 0 or page['tail']:
                raise AssertionError('Unexpected original leaf geometry')
            identifiers = []
            for encoded in entries:
                reference = int.from_bytes(encoded[-4:], 'big')
                identifiers.append(locations[reference])
            return entries, [number], identifiers, 0
        flattened = []
        leaves = []
        identifiers = []
        child_heights = []
        for encoded in entries:
            child = int.from_bytes(encoded[-4:], 'big')
            keys, child_leaves, ids, height = walk(child, active)
            if not keys or encoded[:-4] != keys[-1]:
                raise AssertionError('Full raw node separator differs from child maximum')
            flattened.extend(keys)
            leaves.extend(child_leaves)
            identifiers.extend(ids)
            child_heights.append(height)
        if page['tail']:
            keys, child_leaves, ids, height = walk(page['tail'], active)
            if not keys:
                raise AssertionError('Original tail child is empty')
            flattened.extend(keys)
            leaves.extend(child_leaves)
            identifiers.extend(ids)
            child_heights.append(height)
        if not child_heights or len(set(child_heights)) != 1:
            raise AssertionError('Unequal original child subtree heights')
        height = child_heights[0] + 1
        if page['height'] != height:
            raise AssertionError('Raw stored level differs')
        return flattened, leaves, identifiers, height

    trees = []
    for root in roots:
        keys, leaves, identifiers, height = walk(root, set())
        names = []
        physical = set()
        for name, declaration in info['indexes'].items():
            if declaration['members'] == identifiers:
                names.append(name)
                physical.add(declaration['physical'])
        if not names or len(physical) != 1:
            raise AssertionError('Root getter-order identity is ambiguous or absent')
        owned = 0
        # A root's complete graph is independently selected from its child edges.
        pending = [root]
        root_pages = []
        while pending:
            number = pending.pop()
            root_pages.append(number)
            owned += 1
            page = raw_pages[number]
            if page['kind'] == 3:
                for encoded in page['entries_hex']:
                    pending.append(int.from_bytes(bytes.fromhex(encoded)[-4:], 'big'))
                if page['tail']:
                    pending.append(page['tail'])
        for name in names:
            if info['indexes'][name]['owned_pages'] != owned:
                raise AssertionError('Raw graph/getter owned-page inventory differs')
        partitions = []
        for position, leaf in enumerate(leaves):
            page = raw_pages[leaf]
            previous = leaves[position - 1] if position else 0
            following = leaves[position + 1] if position + 1 < len(leaves) else 0
            if page['previous'] != previous or page['next'] != following:
                raise AssertionError('Raw leaf sibling chain differs')
            ids = []
            for encoded in page['entries_hex']:
                reference = int.from_bytes(bytes.fromhex(encoded)[-4:], 'big')
                ids.append(locations[reference])
            partitions.append({'page': leaf, 'ids': ids})
        trees.append({
            'root': root,
            'logical_names': names,
            'physical': next(iter(physical)),
            'height': height,
            'owned_pages': sorted(root_pages),
            'leaf_partitions': partitions,
            'full_key_reference_sha256': digest(b''.join(keys)),
        })
    if visited != set(raw_pages):
        raise AssertionError('Unvisited original index page')
    physical = set()
    for declaration in info['indexes'].values():
        physical.add(declaration['physical'])
    if len(trees) != len(physical):
        raise AssertionError('Physical index/root inventory differs')
    return trees


def batches(trees, raw_pages):
    partitions = []
    for tree in trees:
        partitions.extend(tree['leaf_partitions'])

    def safe(selected):
        for partition in partitions:
            if not set(partition['ids']) - selected:
                return False
        return True

    maxima = set()
    for partition in partitions:
        candidate = maxima | {partition['ids'][-1]}
        if safe(candidate):
            maxima = candidate
    ties = set()
    for tree in trees:
        leaves = tree['leaf_partitions']
        for left, right in zip(leaves, leaves[1:]):
            last = bytes.fromhex(raw_pages[left['page']]['entries_hex'][-1])
            first = bytes.fromhex(raw_pages[right['page']]['entries_hex'][0])
            if last[:-4] == first[:-4]:
                candidate = ties | {left['ids'][-1]}
                if safe(candidate):
                    ties = candidate
    if not maxima or not ties:
        raise AssertionError('Original corpus lacks safe maxima/tie-boundary evidence')
    smallest = min(partitions, key=lambda partition: len(partition['ids']))
    return {
        'safe-maxima': sorted(maxima),
        'safe-ties': sorted(ties),
        'leaf-exhaustion-observation': sorted(smallest['ids']),
        'delete-all-observation': 'all',
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--existing-references', action='store_true')
    args = parser.parse_args()
    args.jar = (ROOT / args.jar).resolve()
    args.inputs = (ROOT / args.inputs).resolve()
    args.report = (ROOT / args.report).resolve()
    if args.report.exists():
        parser.error('Report must be fresh')
    if digest(args.jar.read_bytes()) != JAR_SHA256:
        parser.error('Reviewed JAR differs')
    helper = ROOT / 'docs/testing/DeleteTreeFixture.java'
    pins = [helper, Path(__file__), args.jar]
    originals = []
    for profile in ('V2016', 'V2019'):
        original = args.inputs / f'Tree-{profile}-before.accdb'
        originals.append(original)
        pins.append(original)
        if args.existing_references:
            for state in ('safe-maxima', 'safe-ties', 'leaf-exhaustion-observation',
                          'delete-all-observation'):
                pins.append(args.inputs / f'Tree-{profile}-before-{state}.accdb')
    before = {}
    for path in pins:
        before[label(path)] = digest(path.read_bytes())
    commands = []
    observations = []
    failure = None

    def run(command):
        normalized = []
        for argument in command:
            normalized.append(label(argument) if isinstance(argument, Path) else argument)
        result = subprocess.run(
            normalized, cwd=ROOT, capture_output=True, text=True, timeout=120,
        )
        commands.append({
            'command': normalized,
            'status': result.returncode,
            'stdout': result.stdout,
            'stderr': result.stderr,
        })
        if result.returncode or result.stderr:
            raise AssertionError('Public producer/getter failed')
        return result.stdout

    try:
        for original in originals:
            text = run(['java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                        helper, 'oracle', original])
            metadata = parse(text)
            raw = pages(original.read_bytes(), metadata['Tree'])
            trees = topology(raw, metadata['Tree'])
            selected = batches(trees, raw)
            item = {
                'input': label(original),
                'metadata': metadata,
                'raw_pages': raw,
                'trees': trees,
                'batches': selected,
                'references': [],
            }
            observations.append(item)
            for name, identifiers in selected.items():
                output = args.inputs / f'{original.stem}-{name}.accdb'
                if args.existing_references:
                    if not output.is_file():
                        raise AssertionError('Expected existing immutable reference')
                elif output.exists():
                    raise AssertionError('Reference copy must be fresh')
                if identifiers == 'all':
                    argument = 'all'
                    removed = set(metadata['Tree']['rows'])
                else:
                    argument = ','.join(map(str, identifiers))
                    removed = set(identifiers)
                if not args.existing_references:
                    run(['java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                         helper, 'delete', original, output, argument])
                getter = run(['java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                              helper, 'oracle', output])
                actual = parse(getter)
                for table in TABLES:
                    expected = {}
                    for identifier, row in metadata[table]['rows'].items():
                        if table != 'Tree' or identifier not in removed:
                            expected[identifier] = row
                    if actual[table]['rows'] != expected:
                        raise AssertionError('Reference surviving getter population/payload differs')
                    if list(actual[table]['rows']) != list(expected):
                        raise AssertionError('Reference sequential encounter order differs')
                    if actual[table]['columns'] != metadata[table]['columns']:
                        raise AssertionError('Reference declared columns differ')
                    if set(actual[table]['indexes']) != set(metadata[table]['indexes']):
                        raise AssertionError('Reference logical index inventory differs')
                    for index, declaration in metadata[table]['indexes'].items():
                        members = []
                        for identifier in declaration['members']:
                            if table != 'Tree' or identifier not in removed:
                                members.append(identifier)
                        if actual[table]['indexes'][index]['members'] != members:
                            raise AssertionError('Reference index encounter order differs')
                reference_raw = pages(output.read_bytes(), metadata['Tree'])
                if name.startswith('safe-'):
                    if set(reference_raw) != set(raw):
                        raise AssertionError('Safe reference changed raw page inventory')
                    for tree in trees:
                        for partition in tree['leaf_partitions']:
                            number = partition['page']
                            expected_keys = []
                            original_keys = raw[number]['entries_hex']
                            for identifier, encoded in zip(partition['ids'], original_keys, strict=True):
                                if identifier not in removed:
                                    expected_keys.append(encoded)
                            if not expected_keys or reference_raw[number]['entries_hex'] != expected_keys:
                                raise AssertionError('Safe reference changed original leaf partition/key bytes')
                    for number, original_page in raw.items():
                        for field in ('kind', 'owner', 'stamp_hex', 'previous', 'next', 'tail', 'height'):
                            if reference_raw[number][field] != original_page[field]:
                                raise AssertionError('Safe reference changed raw topology/header')
                topology_error = None
                reference_trees = None
                try:
                    reference_trees = topology(reference_raw, actual['Tree'])
                except (AssertionError, KeyError) as error:
                    topology_error = str(error)
                item['references'].append({
                    'state': name,
                    'path': label(output),
                    'sha256': digest(output.read_bytes()),
                    'metadata': actual,
                    'raw_pages': reference_raw,
                    'trees': reference_trees,
                    'topology_observation_error': topology_error,
                    'selected_to_keep_all_original_leaves_nonempty': name.startswith('safe-'),
                    'native_access_qualification': False,
                })
    except Exception as error:
        failure = str(error)
    after = {}
    for path in pins:
        after[label(path)] = digest(path.read_bytes())
    report = {
        'scope': 'Original public-builder tree partitions and public delete references; no candidate/native Access SQL',
        'hashes_before': before,
        'hashes_after': after,
        'pins_unchanged': before == after,
        'generation_commands': [],
        'existing_references_read_only': args.existing_references,
        'commands': commands,
        'observations': observations,
        'failure': failure,
    }
    for profile in ('V2016', 'V2019'):
        report['generation_commands'].append([
            'java', '-Dstdout.encoding=UTF-8', '-cp', label(args.jar), label(helper),
            'generate', label(args.inputs / f'Tree-{profile}-before.accdb'), profile,
        ])
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({
        'profiles': len(observations),
        'commands': len(commands),
        'pins_unchanged': report['pins_unchanged'],
        'failure': failure,
    }))
    if failure or not report['pins_unchanged']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
