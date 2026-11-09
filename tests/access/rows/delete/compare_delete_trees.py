#!/usr/bin/env python3
"""Original nonempty-leaf DELETE candidate replay; contraction stays refused."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
REFERENCE = ROOT / 'docs/testing/results/delete-tree-reference-2026-10-26.json'
SOURCE_PATHS = (
    'crates/engines/tablecraft-ace/examples/preview_ace16_delete.rs',
    'crates/engines/tablecraft-ace/src/table/mutation/delete.rs',
    'crates/engines/tablecraft-ace/src/table/mutation/delete/patches.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/delete.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/tree.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/tree/deletion.rs',
)


def load_module(name, relative):
    path = ROOT / relative
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BASE = load_module('flat_candidate_checks', 'docs/testing/compare_delete_candidates.py')
TREE = load_module('original_tree_checks', 'docs/testing/capture_delete_trees.py')


class TreeReplay(BASE.Replay):
    def getters(self, path, jar, helper):
        record = self.run([
            'java', '-Dstdout.encoding=UTF-8', '-cp', jar, helper, 'oracle', path,
        ])
        if record['status'] != 0 or record['stderr']:
            raise AssertionError('Read-only public getter failed')
        return TREE.parse(record['stdout'])


def tree_spans(before, after, metadata, actual, deleted):
    info = metadata['Tree']
    geometry = info['geometry']
    width = geometry['page_bytes']
    old_pages = TREE.pages(before, info)
    new_pages = TREE.pages(after, actual['Tree'])
    old_trees = TREE.topology(old_pages, info)
    new_trees = TREE.topology(new_pages, actual['Tree'])
    if set(old_pages) != set(new_pages):
        raise AssertionError('Index page inventory changed')
    if len(old_trees) != len(new_trees):
        raise AssertionError('Physical root inventory changed')
    for original, candidate in zip(old_trees, new_trees, strict=True):
        for field in ('root', 'logical_names', 'physical', 'height', 'owned_pages'):
            if candidate[field] != original[field]:
                raise AssertionError('Root/alias/height/ownership changed')
        for partition in original['leaf_partitions']:
            number = partition['page']
            retained = []
            encoded = old_pages[number]['entries_hex']
            for identifier, key in zip(partition['ids'], encoded, strict=True):
                if identifier not in deleted:
                    retained.append(key)
            if not retained:
                raise AssertionError('Positive batch exhausted an original leaf')
            if new_pages[number]['entries_hex'] != retained:
                raise AssertionError('Original leaf partition/key/reference bytes changed')
    spans = []
    for number, original in old_pages.items():
        candidate = new_pages[number]
        for field in ('kind', 'owner', 'stamp_hex', 'previous', 'next', 'tail', 'height'):
            if candidate[field] != original[field]:
                raise AssertionError('Index header/topology changed')
        if original['kind'] == 3:
            before_children = []
            after_children = []
            for encoded in original['entries_hex']:
                before_children.append(bytes.fromhex(encoded)[-4:])
            for encoded in candidate['entries_hex']:
                after_children.append(bytes.fromhex(encoded)[-4:])
            if after_children != before_children:
                raise AssertionError('Node child pointer/count changed')
        page_start = number * width
        mask_start = geometry['mask_offset']
        old_end = width - original['free']
        new_end = width - candidate['free']
        spans.append((page_start + 2, page_start + 4))
        spans.append((page_start + 24, page_start + 26))
        spans.append((page_start + mask_start, page_start + max(old_end, new_end)))
    return spans, {
        'physical_roots': len(old_trees),
        'index_pages': len(old_pages),
        'original_leaf_partitions_retained': True,
        'full_separator_key_and_row_maxima_checked': True,
        'root_child_tail_sibling_height_unchanged': True,
    }


def verify(before, after, metadata, actual, deleted, original_metadata):
    if len(before) != len(after):
        raise AssertionError('Image length changed')
    if set(actual) != set(metadata):
        raise AssertionError('Table inventory changed')
    # Original metadata includes records already dead in a reference-input
    # history. Their exact existing tombstones and payloads must also survive.
    slots = BASE.payloads(before, original_metadata)
    allowed = []
    for table, info in metadata.items():
        observed = actual[table]
        expected_ids = []
        for identifier in info['rows']:
            if table != 'Tree' or identifier not in deleted:
                expected_ids.append(identifier)
        if list(observed['rows']) != expected_ids:
            raise AssertionError('Sequential live population/order differs')
        if observed['columns'] != info['columns']:
            raise AssertionError('Declared columns changed')
        for field, value in info['geometry'].items():
            expected = len(expected_ids) if field == 'live_rows' else value
            if observed['geometry'][field] != expected:
                raise AssertionError('Declaration geometry/count changed')
        if set(observed['locations']) != set(expected_ids):
            raise AssertionError('Live location inventory differs')
        for identifier in expected_ids:
            if observed['rows'][identifier] != info['rows'][identifier]:
                raise AssertionError('Surviving scalar/Memo/OLE getter payload differs')
            if observed['locations'][identifier] != info['locations'][identifier]:
                raise AssertionError('Surviving row relocated')
        for identifier, slot in slots[table].items():
            offset, word, start, end = slot
            expected_word = word
            if table == 'Tree' and identifier in deleted:
                expected_word |= BASE.TOMBSTONE_FLAGS
                allowed.append((offset, offset + BASE.SLOT_BYTES))
            if BASE.STORAGE.u16(after, offset) != expected_word:
                raise AssertionError('Original slot offset/tombstone changed unexpectedly')
            if before[start:end] != after[start:end]:
                raise AssertionError('Original record body changed, including dead history')
        if set(observed['indexes']) != set(info['indexes']):
            raise AssertionError('Logical index inventory changed')
        for name, declaration in info['indexes'].items():
            candidate = observed['indexes'][name]
            expected_members = []
            for identifier in declaration['members']:
                if table != 'Tree' or identifier not in deleted:
                    expected_members.append(identifier)
            if candidate['members'] != expected_members:
                raise AssertionError('Index encounter order changed')
            if candidate['entry_count'] != len(expected_members):
                raise AssertionError('Index live membership count differs')
            for field, value in declaration.items():
                if field not in ('members', 'entry_count') and candidate[field] != value:
                    raise AssertionError('Index declaration/alias/historical counter changed')
        geometry = info['geometry']
        count = geometry['definition_page'] * geometry['page_bytes'] + geometry['count_offset']
        if BASE.STORAGE.u32(before, count) != len(info['rows']):
            raise AssertionError('Source stored live count differs')
        if BASE.STORAGE.u32(after, count) != len(expected_ids):
            raise AssertionError('Candidate stored live count differs')
        if table == 'Tree':
            allowed.append((count, count + 4))
    index_allowed, topology_proof = tree_spans(before, after, metadata, actual, deleted)
    allowed.extend(index_allowed)
    changed = 0
    for offset, (left, right) in enumerate(zip(before, after, strict=True)):
        if left == right:
            continue
        changed += 1
        if not any(start <= offset < end for start, end in allowed):
            raise AssertionError(f'Change outside slot/count/exact index spans at {offset}')
    return {
        'changed_bytes': changed,
        'outside_allowed_spans': 0,
        'authorized_spans': allowed,
        'record_bodies_including_long_values_unchanged': True,
        'maps_and_all_other_bytes_unchanged': True,
        'topology': topology_proof,
    }


def request_arguments(image, metadata, selected):
    slots = BASE.payloads(image, metadata)['Tree']
    requests = []
    for ordinal, identifier in enumerate(metadata['Tree']['rows']):
        if identifier in selected:
            _, _, start, end = slots[identifier]
            requests.append(f'{ordinal}:{image[start:end].hex()}')
    if len(requests) != len(selected):
        raise AssertionError('Selected ID is absent from current live snapshot')
    return requests


def individual_boundaries(trees, batches):
    primary = None
    for tree in trees:
        if 'PrimaryKey' in tree['logical_names']:
            primary = tree
    if primary is None or len(primary['leaf_partitions']) < 2:
        raise AssertionError('Original primary tree lacks boundary evidence')
    return {
        'individual-nontail-maximum': {primary['leaf_partitions'][0]['ids'][-1]},
        'individual-tail-maximum': {primary['leaf_partitions'][-1]['ids'][-1]},
        'individual-tie-boundary': {batches['safe-ties'][0]},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--example-sha256', required=True)
    parser.add_argument('--compiled-revision', required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--inputs', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    for name in ('example', 'jar', 'inputs', 'workdir', 'report'):
        path = (ROOT / getattr(args, name)).resolve()
        BASE.label(path)
        setattr(args, name, path)
    try:
        args.workdir.relative_to(ROOT / '.cache')
    except ValueError:
        parser.error('Full raw replay artifacts must remain under ignored .cache')
    if args.workdir.exists() or args.report.exists():
        parser.error('Work directory and report must be fresh')
    if BASE.digest(args.example.read_bytes()) != args.example_sha256:
        parser.error('Frozen example differs')
    summary = json.loads(REFERENCE.read_text())
    raw_reference = ROOT / summary['raw_capture']['path']
    raw_bytes = raw_reference.read_bytes()
    if BASE.digest(raw_bytes) != summary['raw_capture']['sha256']:
        parser.error('Pinned full original reference capture differs')
    reference = json.loads(raw_bytes)
    if reference['hashes_before'] != summary['hashes_before']:
        parser.error('Compact/full reference inventories disagree')
    if reference['hashes_before'] != reference['hashes_after']:
        parser.error('Registered reference pins disagree')
    for relative, expected in reference['hashes_before'].items():
        if BASE.digest((ROOT / relative).read_bytes()) != expected:
            parser.error(f'Registered original/helper/JAR differs: {relative}')
    helper = ROOT / 'docs/testing/DeleteTreeFixture.java'
    if BASE.label(args.jar) not in reference['hashes_before']:
        parser.error('JAR path is not the registered reviewed artifact')
    pins = [Path(__file__), Path(BASE.__file__), Path(TREE.__file__),
            BASE.OBSERVATIONS, helper, REFERENCE, raw_reference, args.example, args.jar]
    for relative in reference['hashes_before']:
        pins.append(ROOT / relative)
    before_pins = BASE.pin_files(pins)
    sources = {}
    for path in SOURCE_PATHS:
        result = subprocess.run(
            ['git', 'show', f'{args.compiled_revision}:{path}'], cwd=ROOT,
            capture_output=True, check=True, timeout=30,
        )
        sources[path] = BASE.digest(result.stdout)
    args.workdir.mkdir(parents=True)
    replay = TreeReplay(args.workdir)
    fatal = None

    def positive(source, metadata, original_metadata, name, selected, family):
        before = source.read_bytes()
        output = args.workdir / f'{source.stem}-{name}.accdb'
        requests = request_arguments(before, metadata, selected)
        outcome = replay.candidate(args.example, source, output, 'Tree', requests, name)
        outcome['coverage_family'] = family
        outcome['selected_ids'] = sorted(selected)
        if 'error' in outcome:
            return
        try:
            actual = replay.getters(output, args.jar, helper)
            outcome['preservation'] = verify(
                before, output.read_bytes(), metadata, actual, selected, original_metadata,
            )
            outcome['agrees'] = True
        except Exception as error:
            outcome['error'] = str(error)

    def refusal(source, metadata, name, selected, mode):
        output = args.workdir / f'{source.stem}-{name}.accdb'
        outcome = {
            'source': BASE.label(source),
            'output': BASE.label(output),
            'selection': name,
            'expected_refusal': True,
            'agrees': False,
        }
        replay.outcomes.append(outcome)
        try:
            requests = request_arguments(source.read_bytes(), metadata, selected)
            if mode == 'stale':
                ordinal, encoded = requests[0].split(':')
                payload = bytes.fromhex(encoded)
                payload = bytes([payload[0] ^ 1]) + payload[1:]
                requests[0] = f'{ordinal}:{payload.hex()}'
            elif mode == 'duplicate':
                requests.append(requests[0])
            elif mode == 'existing-output':
                output.write_bytes(BASE.EXISTING_OUTPUT)
            command = [args.example, source, output, 'Tree']
            for argument in requests:
                command.extend(['--delete', argument])
            record = replay.run(command)
            if mode == 'existing-output':
                unchanged = output.read_bytes() == BASE.EXISTING_OUTPUT
            else:
                unchanged = not output.exists()
            outcome['command_ordinal'] = len(replay.commands)
            if mode == 'contraction':
                expected_error = 'Unsupported(IndexedColumn)'
            elif mode in ('stale', 'duplicate'):
                expected_error = 'Conflict'
            else:
                expected_error = 'AlreadyExists'
            outcome['expected_error_class'] = expected_error
            outcome['agrees'] = (
                record['status'] not in (None, 0)
                and record['stdout'] == ''
                and expected_error in record['stderr']
                and unchanged
            )
        except Exception as error:
            outcome['error'] = str(error)
        finally:
            outcome['candidate_exists'] = output.exists()
            if output.exists():
                outcome['candidate_sha256'] = BASE.digest(output.read_bytes())

    try:
        for observation in reference['observations']:
            source = ROOT / observation['input']
            if source.parent != args.inputs:
                raise AssertionError('Input directory is not the registered corpus')
            metadata = replay.getters(source, args.jar, helper)
            raw = TREE.pages(source.read_bytes(), metadata['Tree'])
            trees = TREE.topology(raw, metadata['Tree'])
            batches = TREE.batches(trees, raw)
            selections = {
                'safe-maxima': set(batches['safe-maxima']),
                'safe-ties': set(batches['safe-ties']),
            }
            selections.update(individual_boundaries(trees, batches))
            for name, selected in selections.items():
                positive(source, metadata, metadata, name, selected, 'original-nonempty-leaf')
            for reference_copy in observation['references']:
                if reference_copy['state'] not in ('safe-maxima', 'safe-ties'):
                    continue
                history = ROOT / reference_copy['path']
                current = replay.getters(history, args.jar, helper)
                history_raw = TREE.pages(history.read_bytes(), current['Tree'])
                history_trees = TREE.topology(history_raw, current['Tree'])
                chosen = individual_boundaries(history_trees, batches)
                selected = chosen['individual-nontail-maximum']
                positive(history, current, metadata, 'history-boundary', selected,
                         'preexisting-safe-reference-tombstones')
            refusal(source, metadata, 'leaf-exhaustion',
                    set(batches['leaf-exhaustion-observation']), 'contraction')
            refusal(source, metadata, 'delete-all', set(metadata['Tree']['rows']), 'contraction')
            first = {next(iter(metadata['Tree']['rows']))}
            for mode in ('stale', 'duplicate', 'existing-output'):
                refusal(source, metadata, mode, first, mode)
    except Exception as error:
        fatal = str(error)
    after_pins = BASE.pin_files(pins)
    disagreements = 0
    for outcome in replay.outcomes:
        if not outcome['agrees']:
            disagreements += 1
    report = {
        'scope': 'Original nonempty-leaf engineering candidate replay; no native Access SQL or contraction admission',
        'compiled_revision': args.compiled_revision,
        'compiled_source_sha256': sources,
        'reference_report': BASE.label(REFERENCE),
        'hashes_before': before_pins,
        'hashes_after': after_pins,
        'pins_unchanged': before_pins == after_pins,
        'commands': replay.commands,
        'outcomes': replay.outcomes,
        'fatal_error': fatal,
        'disagreements': disagreements,
    }
    raw_report = args.workdir / 'full-report.json'
    raw_report.write_text(json.dumps(report, indent=2) + '\n')
    receipts = []
    for command in replay.commands:
        receipts.append({
            'command': command['command'],
            'status': command['status'],
            'stdout_bytes': len(command['stdout'].encode()),
            'stdout_sha256': BASE.digest(command['stdout'].encode()),
            'stderr_bytes': len(command['stderr'].encode()),
            'stderr_sha256': BASE.digest(command['stderr'].encode()),
            'failure': command.get('failure'),
        })
    public_outcomes = []
    for outcome in replay.outcomes:
        compact = dict(outcome)
        if 'preservation' in compact:
            proof = dict(compact['preservation'])
            spans = proof.pop('authorized_spans')
            proof['authorized_span_count'] = len(spans)
            proof['authorized_spans_sha256'] = BASE.digest(
                json.dumps(spans, separators=(',', ':')).encode(),
            )
            compact['preservation'] = proof
        public_outcomes.append(compact)
    public = dict(report)
    public['commands'] = receipts
    public['outcomes'] = public_outcomes
    public['raw_capture'] = {
        'path': BASE.label(raw_report),
        'sha256': BASE.digest(raw_report.read_bytes()),
        'bytes': raw_report.stat().st_size,
        'commands_jsonl': BASE.label(args.workdir / 'commands.jsonl'),
        'commands_jsonl_sha256': BASE.digest(
            (args.workdir / 'commands.jsonl').read_bytes(),
        ),
        'retained_ignored': True,
    }
    args.report.write_text(json.dumps(public, indent=2) + '\n')
    print(json.dumps({
        'controls': len(replay.outcomes),
        'disagreements': disagreements,
        'pins_unchanged': report['pins_unchanged'],
        'fatal_error': fatal,
    }))
    if fatal or disagreements or not report['pins_unchanged']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
