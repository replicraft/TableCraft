#!/usr/bin/env python3
"""Original-file contraction replay using bounded request files and raw models."""
import argparse
import csv
import importlib.util
import json
from pathlib import Path
import subprocess

import capture_delete_contraction as CAP
import delete_reference_maps as MAP

ROOT = Path(__file__).resolve().parents[2]
REFERENCE = ROOT / 'docs/testing/results/delete-contraction-reference-2026-10-26.json'
NODE = CAP.INDEX_NODE_KIND
LEAF = CAP.INDEX_LEAF_KIND
REFERENCE_BYTES = CAP.ROW_REFERENCE_BYTES
INDEX_KIND_OFFSET = 0
INDEX_FREE_OFFSET = 2
INDEX_PREFIX_OFFSET = 24
INDEX_PREVIOUS_OFFSET = 12
INDEX_NEXT_OFFSET = 16
INDEX_TAIL_OFFSET = 20
INDEX_HEIGHT_OFFSET = 26
INDEX_WORD_BYTES = 2
INDEX_TAG_BYTES = 1
INVALID_PAGE_PREFIX_BYTES = 4
SOURCE_PATHS = (
    'crates/engines/tablecraft-ace/examples/preview_ace16_delete.rs',
    'crates/engines/tablecraft-ace/examples/preview_ace16_delete/requests.rs',
    'crates/engines/tablecraft-ace/src/table/mutation/delete.rs',
    'crates/engines/tablecraft-ace/src/table/mutation/delete/patches.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/delete.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/tree.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/tree/contraction.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/retirement.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/retirement/maps.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/retirement/audit.rs',
    'crates/engines/tablecraft-ace/src/format/allocation/edit.rs',
)
SPEC = importlib.util.spec_from_file_location('original_flat_checks', ROOT / 'docs/testing/compare_delete_candidates.py')
BASE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BASE)


class Replay(BASE.Replay):
    def capture(self, path, family, jar):
        helper = ROOT / f'docs/testing/{family}DeleteFixture.java'
        map_helper = ROOT / f'docs/testing/{family}UsageMapProbe.java'
        getter = self.run(['java', '-Dstdout.encoding=UTF-8', '-cp', jar, helper, 'oracle', path])
        probe = self.run(['java', '-Dstdout.encoding=UTF-8', '-cp', jar, map_helper, path])
        for receipt in (getter, probe):
            if receipt['status'] != 0 or receipt['stderr']:
                raise AssertionError('Read-only reference getter/probe failed')
        metadata = CAP.parse_rows(getter['stdout'])
        maps = MAP.parse(probe['stdout'])
        data = path.read_bytes()
        decoded = {}
        for name, item in maps['maps'].items():
            decoded[name] = MAP.decode_map(data, item, maps['page_bytes'], maps['page_count'])
        maps['decoded_maps'] = decoded
        return {
            'metadata': metadata,
            'map_metadata': maps,
            'raw_pages': CAP.TREE.pages(data, metadata['Deep']),
        }


def expected_graph(source, selected):
    pages = source['raw_pages']
    info = source['metadata']['Deep']
    locations = {}
    for identifier, (page, slot) in info['locations'].items():
        locations[(page << CAP.ROW_REFERENCE_SLOT_BITS) + slot] = identifier
    expected = {}
    retired = set()

    def visit(number, root=False):
        page = pages[number]
        if page['kind'] == LEAF:
            entries = []
            for encoded in page['entries_hex']:
                raw = bytes.fromhex(encoded)
                identifier = locations[int.from_bytes(raw[-REFERENCE_BYTES:], 'big')]
                if identifier not in selected:
                    entries.append(encoded)
            if entries or root:
                expected[number] = {
                    'kind': LEAF,
                    'height': 0,
                    'leaf_entries_hex': entries,
                    'children': [],
                }
                return bytes.fromhex(entries[-1]) if entries else None
            retired.add(number)
            return None
        children = []
        maxima = []
        # Stored bounded entries and the optional final tail are one original
        # child sequence. Pruning never redistributes or promotes children.
        for child in CAP.children(page):
            maximum = visit(child)
            if maximum is not None:
                children.append(child)
                maxima.append(maximum.hex())
        if children:
            expected[number] = {
                'kind': NODE,
                'height': page['height'],
                'children': children,
                'child_maxima_hex': maxima,
            }
            return bytes.fromhex(maxima[-1])
        if root:
            expected[number] = {
                'kind': LEAF,
                'height': 0,
                'leaf_entries_hex': [],
                'children': [],
            }
        else:
            retired.add(number)
        return None

    roots = set()
    for declaration in source['map_metadata']['indexes'].values():
        roots.add(declaration['root'])
    for number in sorted(roots):
        # Validate complete original chains at every height, not just leaves.
        original_levels = {}
        pending = [number]
        while pending:
            page_number = pending.pop()
            page = pages[page_number]
            original_levels.setdefault(page['height'], []).append(page_number)
            pending.extend(reversed(CAP.children(page)))
        for order in original_levels.values():
            for position, page_number in enumerate(order):
                previous = order[position - 1] if position else 0
                following = order[position + 1] if position + 1 < len(order) else 0
                page = pages[page_number]
                if (page['previous'], page['next']) != (previous, following):
                    raise AssertionError('Original per-height sibling chain differs')
        visit(number, root=True)
        retained_levels = {}
        pending = [number]
        while pending:
            page_number = pending.pop()
            model = expected[page_number]
            retained_levels.setdefault(model['height'], []).append(page_number)
            pending.extend(reversed(model['children']))
        for order in retained_levels.values():
            for position, page_number in enumerate(order):
                model = expected[page_number]
                model['previous'] = order[position - 1] if position else 0
                model['next'] = order[position + 1] if position + 1 < len(order) else 0
    return expected, retired


def verify_page_model(current, model):
    for field in ('kind', 'height', 'previous', 'next'):
        if current[field] != model[field]:
            raise AssertionError('Kind/height/per-height sibling chain differs')
    if model['kind'] == LEAF:
        if current['entries_hex'] != model['leaf_entries_hex'] or current['tail']:
            raise AssertionError('Original surviving leaf partition/key bytes differ')
    else:
        ordered_children = model['children']
        if CAP.children(current) != ordered_children:
            raise AssertionError('Original surviving child sequence differs')
        if current['tail']:
            if current['tail'] != ordered_children[-1]:
                raise AssertionError('Tail is not the exact last surviving child')
            bounded_children = ordered_children[:-1]
        else:
            bounded_children = ordered_children
        if len(current['entries_hex']) != len(bounded_children):
            raise AssertionError('Bounded-child entry count differs')
        for position, child in enumerate(bounded_children):
            maximum = bytes.fromhex(model['child_maxima_hex'][position])
            encoded = maximum + child.to_bytes(REFERENCE_BYTES, 'big')
            if current['entries_hex'][position] != encoded.hex():
                raise AssertionError('Full retained child key/row maximum differs')


def verify(before, after, source, actual, selected, original_metadata):
    if len(before) != len(after):
        raise AssertionError('Candidate file length changed')
    before_metadata = source['metadata']
    after_metadata = actual['metadata']
    if set(before_metadata) != set(after_metadata):
        raise AssertionError('Candidate table inventory changed')
    allowed = []
    slots = BASE.payloads(before, original_metadata)
    for table, metadata in before_metadata.items():
        observed = after_metadata[table]
        expected_ids = []
        for identifier in metadata['rows']:
            if table != 'Deep' or identifier not in selected:
                expected_ids.append(identifier)
        if list(observed['rows']) != expected_ids:
            raise AssertionError('Live row order/population differs')
        if observed['columns'] != metadata['columns']:
            raise AssertionError('Declared columns changed')
        for identifier in expected_ids:
            if observed['rows'][identifier] != metadata['rows'][identifier]:
                raise AssertionError('GUID/Binary/Memo/OLE getter value changed')
            if observed['locations'][identifier] != metadata['locations'][identifier]:
                raise AssertionError('Live row relocated')
        for name, value in metadata['geometry'].items():
            expected = len(expected_ids) if name == 'live_rows' else value
            if observed['geometry'][name] != expected:
                raise AssertionError('Declaration geometry/count changed')
        if set(observed['indexes']) != set(metadata['indexes']):
            raise AssertionError('Logical index inventory changed')
        for name, declaration in metadata['indexes'].items():
            current = observed['indexes'][name]
            members = [identifier for identifier in declaration['members'] if identifier in expected_ids]
            if current['members'] != members or current['entry_count'] != len(members):
                raise AssertionError('Index population/encounter order differs')
            for field, value in declaration.items():
                if field not in ('members', 'entry_count', 'owned_pages') and current[field] != value:
                    raise AssertionError('Index declaration/alias/historical counter changed')
            if actual['map_metadata']['indexes'][name]['root'] != source['map_metadata']['indexes'][name]['root']:
                raise AssertionError('Physical root identity changed')
        for identifier, (offset, word, start, end) in slots[table].items():
            expected_word = word
            if table == 'Deep' and identifier in selected:
                expected_word |= CAP.ROW_DELETED_FLAGS
                allowed.append((offset, offset + MAP.ROW_SLOT_BYTES))
            if MAP.unsigned(after, offset, MAP.ROW_SLOT_BYTES) != expected_word:
                raise AssertionError('Original/dead-history slot words changed unexpectedly')
            if before[start:end] != after[start:end]:
                raise AssertionError('Complete original record body changed')
        geometry = metadata['geometry']
        offset = geometry['definition_page'] * geometry['page_bytes'] + geometry['count_offset']
        if MAP.unsigned(after, offset) != len(expected_ids):
            raise AssertionError('Raw live count differs')
        if table == 'Deep':
            allowed.append((offset, offset + MAP.PAGE_POINTER_BYTES))

    expected, retired = expected_graph(source, selected)
    old_pages = source['raw_pages']
    new_pages = actual['raw_pages']
    if set(expected) != set(new_pages):
        raise AssertionError('Retained/retired raw page inventory differs')
    width = source['map_metadata']['page_bytes']
    for number, model in expected.items():
        original = old_pages[number]
        current = new_pages[number]
        verify_page_model(current, model)
        if current['owner'] != original['owner'] or current['stamp_hex'] != original['stamp_hex']:
            raise AssertionError('Retained owner/write stamp changed')
        start = number * width
        allowed.extend([
            (start + INDEX_FREE_OFFSET, start + INDEX_FREE_OFFSET + INDEX_WORD_BYTES),
            (start + INDEX_PREFIX_OFFSET, start + INDEX_PREFIX_OFFSET + INDEX_WORD_BYTES),
        ])
        used_end = max(width - original['free'], width - current['free'])
        allowed.append((start + before_metadata['Deep']['geometry']['mask_offset'], start + used_end))
        if original['kind'] != current['kind']:
            allowed.extend([
                (start + INDEX_KIND_OFFSET, start + INDEX_KIND_OFFSET + INDEX_TAG_BYTES),
                (start + INDEX_HEIGHT_OFFSET, start + INDEX_HEIGHT_OFFSET + INDEX_TAG_BYTES),
            ])
        # Exact DFS-derived chains constrain both leaf and internal links.
        allowed.extend([
            (start + INDEX_PREVIOUS_OFFSET, start + INDEX_PREVIOUS_OFFSET + MAP.PAGE_POINTER_BYTES),
            (start + INDEX_NEXT_OFFSET, start + INDEX_NEXT_OFFSET + MAP.PAGE_POINTER_BYTES),
        ])
        if current['tail'] != original['tail']:
            allowed.append((start + INDEX_TAIL_OFFSET, start + INDEX_TAIL_OFFSET + MAP.PAGE_POINTER_BYTES))

    for number in retired:
        start = number * width
        if after[start:start + INVALID_PAGE_PREFIX_BYTES] != bytes(INVALID_PAGE_PREFIX_BYTES):
            raise AssertionError('Retired page invalid header differs')
        if before[start + INVALID_PAGE_PREFIX_BYTES:start + width] != after[start + INVALID_PAGE_PREFIX_BYTES:start + width]:
            raise AssertionError('Retired page residual body was not preserved')
        allowed.append((start, start + INVALID_PAGE_PREFIX_BYTES))

    deltas = {}
    before_maps = source['map_metadata']['decoded_maps']
    after_maps = actual['map_metadata']['decoded_maps']
    for name, old in before_maps.items():
        new = after_maps[name]
        removed = sorted(set(old['file_members']) - set(new['file_members']))
        added = sorted(set(new['file_members']) - set(old['file_members']))
        deltas[name] = {'removed_members': removed, 'added_members': added}
        for page in removed + added:
            location = old['bit_locations'][page]
            allowed.append((location['offset'], location['offset'] + 1))
    map_proof = MAP.verify_transition(before, after, before_maps, after_maps, deltas, width)
    if set(map_proof['retired_index_pages']) != retired:
        raise AssertionError('Retired page set differs from independent original tree model')
    for declaration in actual['map_metadata']['indexes'].values():
        members = after_maps[f"Index{declaration['physical']}Owned"]['file_members']
        if len(members) != declaration['owned_count'] or declaration['root'] not in members:
            raise AssertionError('Owned map/root/getter counts differ')
    # The maintained raw decoder checks complete sibling chains and full node
    # maxima when nonempty. Empty root identities come from the separate probe.
    if after_metadata['Deep']['rows']:
        CAP.TREE.topology(new_pages, after_metadata['Deep'])
    else:
        for page in new_pages.values():
            if page['previous'] or page['next'] or page['tail'] or page['prefix']:
                raise AssertionError('Empty root retains links/prefix')
    changed = 0
    for offset, (left, right) in enumerate(zip(before, after, strict=True)):
        if left != right:
            changed += 1
            if not any(start <= offset < end for start, end in allowed):
                raise AssertionError(f'Changed byte outside exact preservation spans: {offset}')
    return {
        'changed_bytes': changed,
        'retired_pages': sorted(retired),
        'retired_node_count': sum(old_pages[page]['kind'] == NODE for page in retired),
        'retired_leaf_count': sum(old_pages[page]['kind'] == LEAF for page in retired),
        'retained_pages': len(expected),
        'original_leaf_partitions_and_full_separator_maxima_checked': True,
        'surviving_original_child_sequence_and_valid_tail_or_bounded_form_checked': True,
        'per_root_per_height_sibling_chains_checked': True,
        'original_root_identity_and_nonempty_height_preserved': True,
        'all_original_and_dead_history_record_bodies_unchanged': True,
        'retired_residual_bodies_unchanged': True,
        'map_checks': map_proof,
        'outside_allowed_spans': 0,
    }


def requests(image, source, selected):
    result = []
    slots = BASE.payloads(image, source['metadata'])['Deep']
    for ordinal, identifier in enumerate(source['metadata']['Deep']['rows']):
        if identifier in selected:
            _, _, start, end = slots[identifier]
            result.append((ordinal, image[start:end].hex()))
    if len(result) != len(selected):
        raise AssertionError('Request refers to an absent live ID')
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
    for name in ('example', 'jar', 'workdir', 'report'):
        path = (ROOT / getattr(args, name)).resolve()
        BASE.label(path)
        setattr(args, name, path)
    args.workdir.relative_to(ROOT / '.cache')
    if args.workdir.exists() or args.report.exists():
        parser.error('Work directory/report must be fresh')
    if BASE.digest(args.example.read_bytes()) != args.example_sha256:
        parser.error('Frozen example differs')
    summary = json.loads(REFERENCE.read_text())
    for relative, expected in summary['hashes_before'].items():
        if BASE.digest((ROOT / relative).read_bytes()) != expected:
            parser.error(f'Registered input/tool differs: {relative}')
    if BASE.label(args.jar) not in summary['hashes_before']:
        parser.error('JAR is not the registered reviewed artifact')
    pins = [Path(__file__), Path(BASE.__file__), BASE.OBSERVATIONS, REFERENCE, args.example]
    for relative in summary['hashes_before']:
        pins.append(ROOT / relative)
    raw_reports = []
    for capture in summary['raw_captures']:
        path = ROOT / capture['path']
        if BASE.digest(path.read_bytes()) != capture['sha256']:
            parser.error('Pinned original full capture differs')
        pins.append(path)
        raw_reports.append(json.loads(path.read_text()))
    before_pins = BASE.pin_files(pins)
    source_pins = {}
    for path in SOURCE_PATHS:
        result = subprocess.run(['git', 'show', f'{args.compiled_revision}:{path}'], cwd=ROOT,
                                capture_output=True, check=True, timeout=30)
        source_pins[path] = BASE.digest(result.stdout)
    args.workdir.mkdir(parents=True)
    replay = Replay(args.workdir)
    fatal = None
    request_pins = {}

    def attempt(path, family, source, anchor, name, selected, mode=None):
        image = path.read_bytes()
        output = args.workdir / f'{path.stem}-{name}.accdb'
        request_file = args.workdir / f'{path.stem}-{name}.tsv'
        rows = requests(image, source, selected)
        if mode == 'duplicate':
            rows.append(rows[0])
        elif mode == 'stale':
            ordinal, encoded = rows[0]
            payload = bytes.fromhex(encoded)
            rows[0] = (ordinal, (bytes([payload[0] ^ 1]) + payload[1:]).hex())
        if mode == 'existing-output':
            output.write_bytes(BASE.EXISTING_OUTPUT)
        with request_file.open('x', newline='') as stream:
            csv.writer(stream, delimiter='\t', lineterminator='\n').writerows(rows)
        request_pins[BASE.label(request_file)] = BASE.digest(request_file.read_bytes())
        outcome = {
            'source': BASE.label(path),
            'output': BASE.label(output),
            'selection': name,
            'request_file': BASE.label(request_file),
            'request_bytes': request_file.stat().st_size,
            'requests': len(rows),
            'expected_refusal': mode is not None,
            'agrees': False,
        }
        replay.outcomes.append(outcome)
        try:
            result = replay.run([args.example, path, output, 'Deep', '--delete-file', request_file])
            outcome['command_ordinal'] = len(replay.commands)
            if mode:
                expected_error = 'AlreadyExists' if mode == 'existing-output' else 'Conflict'
                unchanged = not output.exists()
                if mode == 'existing-output':
                    unchanged = output.read_bytes() == BASE.EXISTING_OUTPUT
                outcome['expected_error_class'] = expected_error
                outcome['agrees'] = (result['status'] not in (None, 0) and result['stdout'] == ''
                                     and expected_error in result['stderr'] and unchanged)
                return
            if result['status'] != 0 or result['stderr'] != BASE.NOTICE:
                raise AssertionError('Candidate command or qualification notice differs')
            if result['stdout'] != f'candidate_deleted_rows={len(selected)}\n':
                raise AssertionError('Candidate count publication differs')
            actual = replay.capture(output, family, args.jar)
            outcome['checks'] = verify(image, output.read_bytes(), source, actual, selected, anchor)
            outcome['agrees'] = True
        except Exception as error:
            outcome['error'] = str(error)
        finally:
            outcome['candidate_exists'] = output.exists()
            if output.exists():
                data = output.read_bytes()
                outcome['candidate_bytes'] = len(data)
                outcome['candidate_sha256'] = BASE.digest(data)

    try:
        for raw in raw_reports:
            family = raw['family']
            for original in raw['observations']:
                path = ROOT / original['path']
                source = replay.capture(path, family, args.jar)
                anchor = source['metadata']
                batches = original['selection_proofs']['selected_ids']
                for state, identifiers in batches.items():
                    attempt(path, family, source, anchor, state, set(identifiers))
                for reference in original['references']:
                    if reference['state'] not in ('whole-child-subtree', 'retain-one-leaf'):
                        continue
                    history = ROOT / reference['path']
                    current = replay.capture(history, family, args.jar)
                    attempt(history, family, current, anchor, 'history-all', set(current['metadata']['Deep']['rows']))
                first = {next(iter(source['metadata']['Deep']['rows']))}
                for mode in ('stale', 'duplicate', 'existing-output'):
                    attempt(path, family, source, anchor, mode, first, mode)
    except Exception as error:
        fatal = str(error)
    after_pins = BASE.pin_files(pins)
    request_after = {}
    for relative in request_pins:
        request_after[relative] = BASE.digest((ROOT / relative).read_bytes())
    failures = sum(not outcome['agrees'] for outcome in replay.outcomes)
    report = {
        'scope': 'Original-file engineering contraction candidate comparison; no native Access qualification',
        'compiled_revision': args.compiled_revision,
        'compiled_source_sha256': source_pins,
        'hashes_before': before_pins,
        'hashes_after': after_pins,
        'pins_unchanged': before_pins == after_pins,
        'request_hashes_before': request_pins,
        'request_hashes_after': request_after,
        'request_files_unchanged': request_pins == request_after,
        'commands': replay.commands,
        'outcomes': replay.outcomes,
        'fatal_error': fatal,
        'disagreements': failures,
    }
    full = args.workdir / 'full-report.json'
    full.write_text(json.dumps(report, indent=2) + '\n')
    public = dict(report)
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
    public['raw_capture'] = {'path': BASE.label(full), 'sha256': BASE.digest(full.read_bytes()),
                             'commands_jsonl': BASE.label(args.workdir / 'commands.jsonl'),
                             'commands_jsonl_sha256': BASE.digest((args.workdir / 'commands.jsonl').read_bytes()),
                             'retained_ignored': True}
    args.report.write_text(json.dumps(public, indent=2) + '\n')
    print(json.dumps({'controls': len(replay.outcomes), 'disagreements': failures,
                      'pins_unchanged': report['pins_unchanged'], 'requests_unchanged': report['request_files_unchanged'],
                      'fatal_error': fatal}))
    if fatal or failures or not report['pins_unchanged'] or not report['request_files_unchanged']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
