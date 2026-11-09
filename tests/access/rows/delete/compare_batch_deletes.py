#!/usr/bin/env python3
"""Original two-owner batch deletion candidates, with independent final-state proofs."""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

import batch_delete_model as MODEL
import compare_delete_contraction as CONTRACT
import delete_reference_maps as MAP

BASE = CONTRACT.BASE
ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / 'docs/testing/BatchDeleteFixture.java'
BUILD = ROOT / 'docs/testing/results/ace16-batch-delete-example-build-2026-10-26.json'
ORIGINALS = {
    'V2016': ('.cache/batch-delete-originals-next-20261026/Batch-V2016-before.accdb',
              'ebb1f05f264dddf5e8195633a0bd5077f5ea6bb1211da0943086a1dbf713ff80'),
    'V2019': ('.cache/batch-delete-originals-next-20261026/Batch-V2019-before.accdb',
              'eb29beb6a77359ac517233e95f375642a03a4a1623540958d85bef161fad995b'),
}
TABLES = ('Parents', 'Children', 'Sentinel')
OWNERS = ('Parents', 'Children')
ROW_WORD_BYTES = 2
DELETED_MASK = 0xC000
PAGE_BYTES = 4096
INDEX_FREE = 2
INDEX_PREFIX = 24
INDEX_PREVIOUS = 12
INDEX_NEXT = 16
INDEX_TAIL = 20
INDEX_HEIGHT = 26
INDEX_KIND = 0
INDEX_TAG_BYTES = 1
INVALID_PREFIX_BYTES = 4
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'


def pin(path):
    path = Path(path)
    data = path.read_bytes()
    return {'path': BASE.label(path), 'bytes': len(data), 'sha256': BASE.digest(data)}


class Replay(BASE.Replay):
    def capture(self, path, jar):
        getter = self.run(['java', '-Dstdout.encoding=UTF-8', '-cp', jar, HELPER, 'oracle', path])
        maps = self.run(['java', '-Dstdout.encoding=UTF-8', '-cp', jar, HELPER, 'maps', path])
        for command in [getter, maps]:
            if command['status'] != 0 or command['stderr']:
                raise AssertionError('Read-only getter/probe failed')
        metadata = MODEL.parse(getter['stdout'])
        map_metadata = MAP.parse(maps['stdout'])
        raw = path.read_bytes()
        map_metadata['decoded_maps'] = {}
        for name, item in map_metadata['maps'].items():
            map_metadata['decoded_maps'][name] = MAP.decode_map(
                raw, item, map_metadata['page_bytes'], map_metadata['page_count'],
            )
        owners = {}
        for name in OWNERS:
            pages, trees = MODEL.geometry(raw, metadata[name], map_metadata, name)
            owners[name] = {
                'raw_pages': pages,
                'trees': trees,
            }
        result = {
            'metadata': metadata,
            'map_metadata': map_metadata,
            'owners': owners,
        }
        with (self.workdir / 'snapshots.jsonl').open('a') as output:
            output.write(json.dumps({'path': BASE.label(path), 'snapshot': result}) + '\n')
        return result


def owner_snapshot(snapshot, name):
    indexes = {}
    for label, metadata in snapshot['map_metadata']['indexes'].items():
        if label.startswith(name + '/'):
            indexes[label.split('/', 1)[1]] = metadata
    # The maintained pure tree model uses a historical generic table label.
    # Only test-model field names change here, never input/native metadata.
    return {
        'metadata': {'Deep': snapshot['metadata'][name]},
        'map_metadata': {'indexes': indexes},
        'raw_pages': snapshot['owners'][name]['raw_pages'],
    }


def selections(source):
    boundary = set()
    subtrees = set()
    proofs = {}
    for name in OWNERS:
        owner = source['owners'][name]
        wide = next(tree for tree in owner['trees'] if 'Wide' in tree['logical_names'])
        if wide['height'] < 2:
            raise AssertionError('Original wide tree must have actual height two')
        children = CONTRACT.CAP.children(owner['raw_pages'][wide['root']])
        child = children[0]
        ids = CONTRACT.CAP.subtree_ids(child, owner['raw_pages'], source['metadata'][name])
        if not ids:
            raise AssertionError('Original internal subtree must contain live roots')
        last = child
        while owner['raw_pages'][last]['kind'] == MODEL.NODE:
            last = CONTRACT.CAP.children(owner['raw_pages'][last])[-1]
        maximum = bytes.fromhex(owner['raw_pages'][last]['entries_hex'][-1])
        reference = int.from_bytes(maximum[-MODEL.REFERENCE_BYTES:], 'big')
        locations = source['metadata'][name]['locations']
        maximum_ids = [
            identity for identity, (page, slot) in locations.items()
            if ((page << CONTRACT.CAP.ROW_REFERENCE_SLOT_BITS) | slot) == reference
        ]
        if len(maximum_ids) != 1:
            raise AssertionError('Subtree full key/row maximum is ambiguous')
        boundary.add(maximum_ids[0])
        subtrees.update(ids)
        proofs[name] = {
            'root': wide['root'],
            'child': child,
            'subtree_ids': sorted(ids),
            'subtree_maximum_id': maximum_ids[0],
        }
    population = set(source['metadata']['Parents']['rows'])
    if population != set(source['metadata']['Children']['rows']):
        raise AssertionError('Original parent/child ID sets differ')
    return {
        'boundary-maxima': boundary,
        'two-original-subtrees': subtrees,
        'retain-one': population - {1},
        'delete-all': population,
    }, proofs


def verify_values(source, actual, removed):
    if set(source['metadata']) != set(actual['metadata']):
        raise AssertionError('Table inventory changed')
    for name in TABLES:
        before = source['metadata'][name]
        after = actual['metadata'][name]
        expected = {}
        for identity, row in before['rows'].items():
            if name == 'Sentinel' or identity not in removed:
                expected[identity] = row
        if after['rows'] != expected or list(after['rows']) != list(expected):
            raise AssertionError('Independent retained value/order model differs')
        if after['columns'] != before['columns']:
            raise AssertionError('Column declarations changed')
        if set(after['indexes']) != set(before['indexes']):
            raise AssertionError('Logical index inventory changed')
        for label, original in before['indexes'].items():
            current = after['indexes'][label]
            members = [identity for identity in original['members'] if identity in expected]
            if current['members'] != members or current['entry_count'] != len(expected):
                raise AssertionError('Logical key order or population differs')
            for field in ['physical', 'historical_count', 'counter_offset', 'columns', 'unique', 'ignore_nulls']:
                if current[field] != original[field]:
                    raise AssertionError('Logical/physical declaration identity changed')


def verify(before, after, source, actual, removed):
    if len(before) != len(after):
        raise AssertionError('Candidate file size changed')
    verify_values(source, actual, removed)
    slots = BASE.payloads(before, source['metadata'])
    allowed = []
    for name, table in source['metadata'].items():
        geometry = table['geometry']
        count = geometry['definition_page'] * PAGE_BYTES + geometry['count_offset']
        expected_rows = len(actual['metadata'][name]['rows'])
        if MAP.unsigned(after, count) != expected_rows:
            raise AssertionError('Raw live declaration differs')
        if name in OWNERS:
            allowed.append((count, count + MAP.PAGE_POINTER_BYTES))
        for identity, (offset, word, start, end) in slots[name].items():
            deleted = name in OWNERS and identity in removed
            expected_word = word | DELETED_MASK if deleted else word
            if int.from_bytes(after[offset:offset + ROW_WORD_BYTES], 'little') != expected_word:
                raise AssertionError('Exact original tombstone word differs')
            if deleted:
                allowed.append((offset, offset + ROW_WORD_BYTES))
            if before[start:end] != after[start:end]:
                raise AssertionError('Original record body changed')
    all_retired = set()
    owner_results = {}
    for name in OWNERS:
        original = owner_snapshot(source, name)
        expected, retired = CONTRACT.expected_graph(original, removed)
        pages = actual['owners'][name]['raw_pages']
        if set(pages) != set(expected):
            raise AssertionError('Retained physical page inventory differs')
        for number, model in expected.items():
            old = original['raw_pages'][number]
            current = pages[number]
            CONTRACT.verify_page_model(current, model)
            if current['owner'] != old['owner'] or current['stamp_hex'] != old['stamp_hex']:
                raise AssertionError('Index owner/write stamp changed')
            start = number * PAGE_BYTES
            allowed.extend([
                (start + INDEX_FREE, start + INDEX_FREE + ROW_WORD_BYTES),
                (start + INDEX_PREFIX, start + INDEX_PREFIX + ROW_WORD_BYTES),
                (start + INDEX_PREVIOUS, start + INDEX_PREVIOUS + MAP.PAGE_POINTER_BYTES),
                (start + INDEX_NEXT, start + INDEX_NEXT + MAP.PAGE_POINTER_BYTES),
            ])
            used = max(PAGE_BYTES - old['free'], PAGE_BYTES - current['free'])
            mask = source['metadata'][name]['geometry']['mask_offset']
            allowed.append((start + mask, start + used))
            if old['kind'] != current['kind']:
                allowed.extend([(start + INDEX_KIND, start + INDEX_KIND + INDEX_TAG_BYTES),
                                (start + INDEX_HEIGHT, start + INDEX_HEIGHT + INDEX_TAG_BYTES)])
            if old['tail'] != current['tail']:
                allowed.append((start + INDEX_TAIL, start + INDEX_TAIL + MAP.PAGE_POINTER_BYTES))
        for number in retired:
            start = number * PAGE_BYTES
            if after[start:start + INVALID_PREFIX_BYTES] != bytes(INVALID_PREFIX_BYTES):
                raise AssertionError('Retired invalidation prefix differs')
            if after[start + INVALID_PREFIX_BYTES:start + PAGE_BYTES] != before[start + INVALID_PREFIX_BYTES:start + PAGE_BYTES]:
                raise AssertionError('Retired residual changed')
            allowed.append((start, start + INVALID_PREFIX_BYTES))
        if retired & all_retired:
            raise AssertionError('Cross-table retired ownership overlaps')
        all_retired.update(retired)
        owner_results[name] = {
            'retired_pages': sorted(retired),
            'retained_pages': len(expected),
        }
    old_maps = source['map_metadata']['decoded_maps']
    new_maps = actual['map_metadata']['decoded_maps']
    deltas = {}
    for label, old in old_maps.items():
        new = new_maps[label]
        delta = {
            'removed_members': sorted(set(old['file_members']) - set(new['file_members'])),
            'added_members': sorted(set(new['file_members']) - set(old['file_members'])),
        }
        deltas[label] = delta
        for number in delta['removed_members'] + delta['added_members']:
            offset = old['bit_locations'][number]['offset']
            allowed.append((offset, offset + 1))
    map_proof = MAP.verify_transition(before, after, old_maps, new_maps, deltas, PAGE_BYTES)
    if set(map_proof['retired_index_pages']) != all_retired:
        raise AssertionError('Global retirement union differs')
    changed = 0
    for offset, (old, new) in enumerate(zip(before, after, strict=True)):
        if old != new:
            changed += 1
            if not any(start <= offset < end for start, end in allowed):
                raise AssertionError(f'Unexpected changed byte {offset}')
    shared_bytes = []
    global_locations = old_maps['JackcessGlobalFree']['bit_locations']
    offsets = {global_locations[page]['offset'] for page in all_retired}
    for offset in sorted(offsets):
        owners = {}
        for name, result in owner_results.items():
            members = []
            for page in result['retired_pages']:
                if global_locations[page]['offset'] == offset:
                    members.append(page)
            if members:
                owners[name] = members
        if len(owners) == 2:
            shared_bytes.append({
                'offset': offset,
                'before': before[offset],
                'after': after[offset],
                'owners': owners,
            })
    return {
        'changed_bytes': changed,
        'owners': owner_results,
        'maps': map_proof,
        'shared_global_bytes': shared_bytes,
        'outside_allowed_spans': 0,
        'all_original_record_bodies_preserved': True,
        'original_leaf_partitions_full_key_row_maxima_and_height_chains_checked': True,
    }


def requests(before, source, name, removed):
    slots = BASE.payloads(before, source['metadata'])[name]
    result = []
    for ordinal, identity in enumerate(source['metadata'][name]['rows']):
        if identity in removed:
            _, _, start, end = slots[identity]
            result.append((ordinal, before[start:end].hex()))
    if len(result) != len(removed):
        raise AssertionError('Request ID is absent from original rows')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--example-sha256', required=True)
    parser.add_argument('--compiled-revision', required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    for field in ['example', 'jar', 'workdir', 'report']:
        value = (ROOT / getattr(args, field)).resolve()
        BASE.label(value)
        setattr(args, field, value)
    args.workdir.relative_to(ROOT / '.cache')
    if args.workdir.exists() or args.report.exists():
        parser.error('Work directory and report must be fresh')
    build = json.loads(BUILD.read_text())
    if args.compiled_revision != build['compiled_revision'] or pin(args.example) != build['frozen_example']:
        parser.error('Frozen artifact/revision does not match build receipt')
    if BASE.digest(args.example.read_bytes()) != args.example_sha256:
        parser.error('Frozen artifact hash mismatch')
    if BASE.digest(args.jar.read_bytes()) != JAR_SHA256:
        parser.error('Reviewed JAR hash mismatch')
    for item in build['source_pins']:
        data = subprocess.run(['git', 'show', args.compiled_revision + ':' + item['path']],
                              capture_output=True, check=True).stdout
        if len(data) != item['bytes'] or BASE.digest(data) != item['sha256']:
            parser.error('Compiled-revision source pin differs')
    inputs = []
    for profile, (label, expected) in ORIGINALS.items():
        path = ROOT / label
        if BASE.digest(path.read_bytes()) != expected:
            parser.error('Original identity changed')
        inputs.append(path)
    tools = [
        args.example,
        args.jar,
        HELPER,
        Path(__file__),
        ROOT / 'docs/testing/batch_delete_model.py',
        ROOT / 'docs/testing/compare_delete_contraction.py',
        ROOT / 'docs/testing/compare_delete_candidates.py',
        ROOT / 'docs/testing/capture_delete_trees.py',
        ROOT / 'docs/testing/capture_delete_contraction.py',
        ROOT / 'docs/testing/delete_reference_maps.py',
        BUILD,
        ROOT / '.cache/batch-delete-originals-next-20261026/initial-commands.json',
        ROOT / '.cache/batch-delete-originals-next-20261026/original-geometry.json',
        ROOT / '.cache/batch-delete-originals-next-20261026/selection-model-before-actions.json',
    ]
    pins_before = [pin(path) for path in inputs + tools]
    args.workdir.mkdir()
    replay = Replay(args.workdir)
    outcomes = []
    references = []
    request_pins = []
    fatal = None
    try:
        for profile, (label, _) in ORIGINALS.items():
            source_path = ROOT / label
            before = source_path.read_bytes()
            source = replay.capture(source_path, args.jar)
            for name in OWNERS:
                if set(source['metadata'][name]['rows']) != set(range(1, 501)):
                    raise AssertionError('Fresh original identity inventory differs')
            batches, selection_proofs = selections(source)
            reference_snapshots = {}
            for state, removed in batches.items():
                reference = args.workdir / f'{profile}-{state}-reference.accdb'
                selector = ','.join(map(str, sorted(removed)))
                command = replay.run(['java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                                      HELPER, 'delete', source_path, reference, selector])
                item = {
                    'profile': profile,
                    'state': state,
                    'selected_ids': sorted(removed),
                    'command': len(replay.commands) - 1,
                    'path': BASE.label(reference),
                }
                references.append(item)
                if reference.exists():
                    item['file'] = pin(reference)
                if command['status'] != 0 or command['stderr']:
                    raise AssertionError('Public child-before-parent reference action failed')
                actual = replay.capture(reference, args.jar)
                verify_values(source, actual, removed)
                reference_snapshots[state] = actual
                item['final_values_and_index_orders_match'] = True
                item['selection_proofs'] = selection_proofs
            baseline_outputs = {}
            modes = []
            for state, removed in batches.items():
                modes.extend([(state, removed, False, None), (state, removed, True, None)])
            first = {next(iter(source['metadata']['Parents']['rows']))}
            for failure in ['parent-only', 'surviving-child', 'stale', 'duplicate-group', 'duplicate-request', 'existing-output']:
                modes.append((failure, first, False, failure))
            for number, (state, removed, reverse, failure) in enumerate(modes):
                output = args.workdir / f'{profile}-{number:02}-{state}.accdb'
                group_files = {}
                for name in ['Children', 'Parents']:
                    selected = removed
                    if failure == 'surviving-child' and name == 'Children':
                        selected = set()
                    rows = requests(before, source, name, selected)
                    if failure == 'stale' and name == 'Children':
                        ordinal, text = rows[0]
                        rows[0] = (ordinal, 'ff' + text[2:])
                    if failure == 'duplicate-request' and name == 'Children':
                        rows.append(rows[0])
                    if not rows:
                        # A different valid child request keeps this file nonempty
                        # while deliberately leaving the selected dependent alive.
                        other = {identity for identity in source['metadata'][name]['rows'] if identity not in removed}
                        rows = requests(before, source, name, {min(other)})
                    path = args.workdir / f'{profile}-{number:02}-{name}.tsv'
                    path.write_text(''.join(f'{row}\t{text}\n' for row, text in rows))
                    request_pins.append(pin(path))
                    group_files[name] = path
                group_names = ['Children', 'Parents']
                if reverse:
                    group_names.reverse()
                if failure == 'parent-only':
                    group_names = ['Parents']
                if failure == 'duplicate-group':
                    group_names = ['Children', 'Children']
                if failure == 'existing-output':
                    output.write_bytes(b'previous output remains unchanged')
                command = [args.example, source_path, output]
                for name in group_names:
                    command.extend(['--group', name, group_files[name]])
                result = replay.run(command)
                outcome = {
                    'profile': profile,
                    'state': state,
                    'reverse_groups': reverse,
                    'expected': 'candidate' if failure is None else 'refusal',
                    'command': len(replay.commands) - 1,
                    'agrees': False,
                }
                outcomes.append(outcome)
                if output.exists():
                    outcome['output'] = pin(output)
                try:
                    if failure:
                        assert result['status'] == 1 and result['stdout'] == ''
                        expected_error = {
                            'parent-only': 'MissingParent',
                            'surviving-child': 'MissingParent',
                            'stale': 'Conflict',
                            'duplicate-group': 'Repeated table',
                            'duplicate-request': 'Conflict',
                            'existing-output': 'AlreadyExists',
                        }[failure]
                        if expected_error not in result['stderr']:
                            raise AssertionError('Unexpected refusal class: ' + result['stderr'])
                        if failure == 'existing-output':
                            assert output.read_bytes() == b'previous output remains unchanged'
                        else:
                            assert not output.exists()
                        outcome['actual_stderr'] = result['stderr']
                    else:
                        assert result['status'] == 0
                        assert result['stdout'] == f'candidate_deleted_rows={2 * len(removed)}\n'
                        assert result['stderr'] == 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
                        actual = replay.capture(output, args.jar)
                        outcome['proof'] = verify(before, output.read_bytes(), source, actual, removed)
                        verify_values(reference_snapshots[state], actual, set())
                        if state in baseline_outputs:
                            assert output.read_bytes() == baseline_outputs[state]
                            outcome['reverse_group_bytes_identical'] = True
                        else:
                            baseline_outputs[state] = output.read_bytes()
                    outcome['agrees'] = True
                except Exception as error:
                    outcome['disagreement'] = str(error) or type(error).__name__
    except Exception as error:
        fatal = str(error) or type(error).__name__
    pins_after = [pin(path) for path in inputs + tools]
    raw = {
        'pins_before': pins_before,
        'pins_after': pins_after,
        'commands': replay.commands,
        'references': references,
        'outcomes': outcomes,
        'fatal_error': fatal,
        'request_pins_before': request_pins,
        'request_pins_after': [pin(ROOT / item['path']) for item in request_pins],
    }
    full = args.workdir / 'full-report.json'
    full.write_text(json.dumps(raw, indent=2) + '\n')
    public = dict(raw)
    public['commands'] = []
    for command in replay.commands:
        public['commands'].append({
            'command': command['command'],
            'status': command['status'],
            'stdout_bytes': len(command['stdout'].encode()),
            'stdout_sha256': BASE.digest(command['stdout'].encode()),
            'stderr_bytes': len(command['stderr'].encode()),
            'stderr_sha256': BASE.digest(command['stderr'].encode()),
        })
    public['references'] = []
    for reference in references:
        compact = {key: value for key, value in reference.items()
                   if key not in ['selected_ids', 'selection_proofs']}
        selected = reference['selected_ids']
        compact['selected_count'] = len(selected)
        compact['selected_ids_sha256'] = BASE.digest(json.dumps(selected).encode())
        compact['selected_boundary_ids'] = selected[:3] + selected[-3:]
        compact['selection_proofs'] = {}
        for name, proof in reference.get('selection_proofs', {}).items():
            identities = proof['subtree_ids']
            compact['selection_proofs'][name] = {
                'root': proof['root'],
                'child': proof['child'],
                'subtree_maximum_id': proof['subtree_maximum_id'],
                'subtree_count': len(identities),
                'subtree_ids_sha256': BASE.digest(json.dumps(identities).encode()),
            }
        public['references'].append(compact)
    public['raw_capture'] = pin(full)
    public['command_capture'] = pin(args.workdir / 'commands.jsonl')
    snapshots = args.workdir / 'snapshots.jsonl'
    if snapshots.exists():
        public['snapshots'] = pin(snapshots)
    public['compiled_build_receipt'] = pin(BUILD)
    public['compiled_source_pins'] = build['source_pins']
    public['compiled_revision'] = args.compiled_revision
    public['pins_unchanged'] = pins_before == pins_after
    public['requests_unchanged'] = raw['request_pins_before'] == raw['request_pins_after']
    public['controls'] = len(outcomes)
    public['positive_controls'] = sum(item['expected'] == 'candidate' for item in outcomes)
    public['refusal_controls'] = sum(item['expected'] == 'refusal' for item in outcomes)
    public['disagreements'] = sum(not item['agrees'] for item in outcomes)
    public['scope'] = 'Public-builder final-state engineering comparison; no native Access or SQL action-timing qualification.'
    args.report.write_text(json.dumps(public, indent=2) + '\n')
    summary = {}
    for key in ['controls', 'positive_controls', 'refusal_controls', 'disagreements',
                'fatal_error', 'pins_unchanged', 'requests_unchanged']:
        summary[key] = public[key]
    print(json.dumps(summary))
    return int(bool(fatal or public['disagreements'] or not public['pins_unchanged'] or not public['requests_unchanged']))


if __name__ == '__main__':
    raise SystemExit(main())
