#!/usr/bin/env python3
"""Replay physical DELETE candidates against pinned original rows and getters."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[2]
OBSERVATIONS = ROOT / 'docs/testing/compare_delete_storage.py'
SPEC = importlib.util.spec_from_file_location('storage_observations', OBSERVATIONS)
STORAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(STORAGE)
TABLES = ('Indexed', 'CodecKeys', 'FloatingKeys', 'Unindexed', 'Sentinel')
NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
REFERENCE = ROOT / 'docs/testing/results/delete-candidate-reference-2026-10-26.json'
SOURCE_PATHS = (
    'crates/engines/tablecraft-ace/examples/preview_ace16_delete.rs',
    'crates/engines/tablecraft-ace/src/table/mutation/delete.rs',
    'crates/engines/tablecraft-ace/src/table/mutation/delete/patches.rs',
    'crates/engines/tablecraft-ace/src/table/indexes/mutation/delete.rs',
)
SLOT_BYTES = 2
SLOT_OFFSET_MASK = 0x1FFF
TOMBSTONE_FLAGS = 0xC000
INDEX_FREE_OFFSET = 2
INDEX_PREFIX_OFFSET = 24
EXISTING_OUTPUT = b'preexisting output must remain unchanged'


def digest(data):
    return STORAGE.digest(data)


def label(path):
    return Path(path).resolve().relative_to(ROOT).as_posix()


def pin_files(paths):
    result = {}
    for path in paths:
        result[label(path)] = digest(path.read_bytes())
    return result


def parse(text):
    result = {}
    for line in text.splitlines():
        cells = line.split('\t')
        if len(cells) < 2 or cells[1] not in TABLES:
            raise AssertionError('Unexpected getter table or record')
        tag, table = cells[:2]
        if tag == 'TABLE':
            if table in result:
                raise AssertionError('Duplicate table declaration')
            names = (
                'live_rows', 'definition_page', 'page_bytes', 'count_offset',
                'row_start', 'mask_offset', 'mask_bytes',
            )
            geometry = {}
            for name, value in zip(names, cells[2:], strict=True):
                geometry[name] = int(value)
            result[table] = {
                'geometry': geometry,
                'columns': [],
                'rows': {},
                'locations': {},
                'indexes': {},
            }
            continue
        if table not in result:
            raise AssertionError('Record precedes table declaration')
        item = result[table]
        if tag == 'LOCATION':
            identifier = int(cells[2])
            if identifier in item['locations']:
                raise AssertionError('Duplicate row location')
            item['locations'][identifier] = tuple(map(int, cells[3:]))
        elif tag == 'ROW':
            identifier = int(cells[2])
            if identifier in item['rows']:
                raise AssertionError('Duplicate row payload')
            item['rows'][identifier] = cells[3:]
        elif tag == 'COLUMN':
            item['columns'].append(cells[2:])
        elif tag == 'INDEX':
            name = cells[2]
            if name in item['indexes']:
                raise AssertionError('Duplicate logical index')
            item['indexes'][name] = {
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
            item['indexes'][cells[2]]['columns'].append(cells[3:])
        elif tag == 'ENTRY':
            item['indexes'][cells[2]]['members'].append(int(cells[3]))
        else:
            raise AssertionError('Unexpected getter record')
    if set(result) != set(TABLES):
        raise AssertionError('Exact table inventory differs')
    return result


def payloads(data, metadata):
    result = {}
    for table, info in metadata.items():
        geometry = info['geometry']
        width = geometry['page_bytes']
        result[table] = {}
        for identifier, location in info['locations'].items():
            page, slot = location
            offset = page * width + geometry['row_start'] + slot * SLOT_BYTES
            word = STORAGE.u16(data, offset)
            start = page * width + (word & SLOT_OFFSET_MASK)
            if slot == 0:
                end = (page + 1) * width
            else:
                preceding = STORAGE.u16(data, offset - SLOT_BYTES)
                end = page * width + (preceding & SLOT_OFFSET_MASK)
            if not page * width <= start < end <= (page + 1) * width:
                raise AssertionError('Invalid original slot geometry')
            result[table][identifier] = (offset, word, start, end)
    return result


def decoded_leaf_entries(image, number, geometry):
    width = geometry['page_bytes']
    page = image[number * width:(number + 1) * width]
    mask_start = geometry['mask_offset']
    entry_start = mask_start + geometry['mask_bytes']
    boundaries = []
    for byte_index, value in enumerate(page[mask_start:entry_start]):
        for bit in range(8):
            if value & (1 << bit):
                boundaries.append(byte_index * 8 + bit)
    prefix = STORAGE.u16(page, INDEX_PREFIX_OFFSET)
    entries = []
    first = b''
    previous = 0
    for end in boundaries:
        if not previous < end <= width - entry_start:
            raise AssertionError('Invalid independent entry boundary')
        encoded = page[entry_start + previous:entry_start + end]
        if previous == 0:
            first = encoded
        else:
            if prefix > len(first):
                raise AssertionError('Invalid independent prefix width')
            encoded = first[:prefix] + encoded
        if len(encoded) < 4:
            raise AssertionError('Entry lacks complete row reference')
        entries.append(encoded)
        previous = end
    if width - entry_start - previous != STORAGE.u16(page, INDEX_FREE_OFFSET):
        raise AssertionError('Independent entry/free-space extent differs')
    return entries


def index_spans(before, after, info, expected_ids):
    geometry = info['geometry']
    width = geometry['page_bytes']
    old_leaves = STORAGE.raw_index_pages(before, info)
    new_leaves = STORAGE.raw_index_pages(after, info)
    old_pages = {}
    new_pages = {}
    for leaf in old_leaves:
        old_pages[leaf['page']] = leaf
    for leaf in new_leaves:
        new_pages[leaf['page']] = leaf
    if set(old_pages) != set(new_pages):
        raise AssertionError('Root-leaf page inventory changed')
    physical = set()
    for declaration in info['indexes'].values():
        physical.add(declaration['physical'])
    if len(old_pages) != len(physical):
        raise AssertionError('Physical root inventory differs from declarations')
    spans = []
    for number, original in old_pages.items():
        current = new_pages[number]
        expected = []
        for identifier in original['member_ids']:
            if identifier in expected_ids:
                expected.append(identifier)
        if current['member_ids'] != expected:
            raise AssertionError('Raw leaf reference encounter order differs')
        old_entries = decoded_leaf_entries(before, number, geometry)
        new_entries = decoded_leaf_entries(after, number, geometry)
        expected_entries = []
        for identifier, encoded in zip(original['member_ids'], old_entries, strict=True):
            if identifier in expected_ids:
                expected_entries.append(encoded)
        if new_entries != expected_entries:
            raise AssertionError('Surviving physical key/reference bytes changed')
        if not expected and current['prefix_bytes'] != 0:
            raise AssertionError('Empty root retains nonzero prefix width')
        page_start = number * width
        mask_start = geometry['mask_offset']
        entry_start = mask_start + geometry['mask_bytes']
        old_end = width - original['free_bytes']
        new_end = width - current['free_bytes']
        if not entry_start <= old_end <= width or not entry_start <= new_end <= width:
            raise AssertionError('Invalid entry/free-space geometry')
        # Only the observed free/prefix words, entry bitmap and used entry data
        # can change. Signature, owner, links, tail and remaining padding cannot.
        spans.append((page_start + INDEX_FREE_OFFSET, page_start + INDEX_FREE_OFFSET + 2))
        spans.append((page_start + INDEX_PREFIX_OFFSET, page_start + INDEX_PREFIX_OFFSET + 2))
        spans.append((page_start + mask_start, page_start + max(old_end, new_end)))
    return spans


def verify(before, after, metadata, actual, target, deleted, slot_metadata=None):
    if len(before) != len(after):
        raise AssertionError('Candidate changed file length')
    if slot_metadata is None:
        slot_metadata = metadata
    slots = payloads(before, slot_metadata)
    allowed = []
    if set(actual) != set(metadata):
        raise AssertionError('Exact table inventory differs')
    for table, info in metadata.items():
        expected_ids = []
        for identifier in info['rows']:
            if table != target or identifier not in deleted:
                expected_ids.append(identifier)
        observed = actual[table]
        if list(observed['rows']) != expected_ids:
            raise AssertionError('Sequential population/order changed')
        if observed['columns'] != info['columns']:
            raise AssertionError('Declared column inventory/type/position changed')
        for name, value in info['geometry'].items():
            expected = len(expected_ids) if name == 'live_rows' else value
            if observed['geometry'][name] != expected:
                raise AssertionError('Declaration geometry/live count differs')
        if set(observed['locations']) != set(expected_ids):
            raise AssertionError('Row location inventory differs')
        for identifier in expected_ids:
            if observed['rows'][identifier] != info['rows'][identifier]:
                raise AssertionError('Surviving getter payload changed')
            if observed['locations'][identifier] != info['locations'][identifier]:
                raise AssertionError('Surviving row relocated')
        for identifier, slot in slots[table].items():
            offset, word, start, end = slot
            expected_word = word
            if table == target and identifier in deleted:
                expected_word |= TOMBSTONE_FLAGS
                allowed.append((offset, offset + SLOT_BYTES))
            if STORAGE.u16(after, offset) != expected_word:
                raise AssertionError('Slot flags/offset changed unexpectedly')
            if before[start:end] != after[start:end]:
                raise AssertionError('Original record payload changed')
        if set(observed['indexes']) != set(info['indexes']):
            raise AssertionError('Exact logical index inventory differs')
        for name, original in info['indexes'].items():
            current = observed['indexes'][name]
            expected_members = []
            for identifier in original['members']:
                if identifier in expected_ids:
                    expected_members.append(identifier)
            if current['members'] != expected_members:
                raise AssertionError('Index membership/order differs')
            if current['entry_count'] != len(expected_members):
                raise AssertionError('Index live entry count differs')
            for field, value in original.items():
                if field not in ('members', 'entry_count') and current[field] != value:
                    raise AssertionError('Index declaration/alias/historical counter changed')
        geometry = info['geometry']
        count = geometry['definition_page'] * geometry['page_bytes'] + geometry['count_offset']
        if STORAGE.u32(before, count) != len(info['rows']):
            raise AssertionError('Original raw live count differs')
        if STORAGE.u32(after, count) != len(expected_ids):
            raise AssertionError('Candidate raw live count differs')
        if table == target:
            allowed.append((count, count + 4))
            allowed.extend(index_spans(before, after, info, set(expected_ids)))
    changed = 0
    for offset, (left, right) in enumerate(zip(before, after, strict=True)):
        if left == right:
            continue
        changed += 1
        if not any(start <= offset < end for start, end in allowed):
            raise AssertionError(f'Changed byte outside exact allowed spans: {offset}')
    return {
        'changed_bytes': changed,
        'outside_allowed_spans': 0,
        'authorized_spans': allowed,
        'original_record_payloads_unchanged': True,
    }


class Replay:
    def __init__(self, workdir):
        self.workdir = workdir
        self.commands = []
        self.outcomes = []

    def run(self, command):
        normalized = []
        for argument in command:
            normalized.append(label(argument) if isinstance(argument, Path) else str(argument))
        try:
            completed = subprocess.run(
                normalized, cwd=ROOT, capture_output=True, text=True, timeout=120,
            )
            record = {
                'command': normalized,
                'status': completed.returncode,
                'stdout': completed.stdout,
                'stderr': completed.stderr,
            }
        except subprocess.TimeoutExpired as error:
            stdout = error.stdout or b''
            stderr = error.stderr or b''
            if isinstance(stdout, bytes):
                stdout = stdout.decode('utf-8', errors='replace')
            if isinstance(stderr, bytes):
                stderr = stderr.decode('utf-8', errors='replace')
            record = {
                'command': normalized,
                'status': None,
                'stdout': stdout,
                'stderr': stderr,
                'failure': 'timeout after 120 seconds',
            }
        except OSError as error:
            record = {
                'command': normalized,
                'status': None,
                'stdout': '',
                'stderr': str(error),
                'failure': type(error).__name__,
            }
        self.commands.append(record)
        with (self.workdir / 'commands.jsonl').open('a') as output:
            output.write(json.dumps(record) + '\n')
        return record

    def getters(self, path, jar, helper):
        record = self.run([
            'java', '-Dstdout.encoding=UTF-8', '-cp', jar, helper, 'oracle', path,
        ])
        if record['status'] != 0 or record['stderr']:
            raise AssertionError('Public read-only getter failed')
        return parse(record['stdout'])

    def candidate(self, example, source, output, table, requests, check):
        outcome = {
            'source': label(source),
            'output': label(output),
            'table': table,
            'selection': check,
            'agrees': False,
        }
        # Register before invocation, so a later getter/parse failure cannot
        # make a created candidate disappear from the terminal report.
        self.outcomes.append(outcome)
        try:
            command = [example, source, output, table]
            for argument in requests:
                command.extend(['--delete', argument])
            record = self.run(command)
            outcome['command_ordinal'] = len(self.commands)
            if record['status'] != 0:
                raise AssertionError('Candidate command failed')
            if record['stdout'] != f'candidate_deleted_rows={len(requests)}\n':
                raise AssertionError('Candidate count publication differs')
            if record['stderr'] != NOTICE:
                raise AssertionError('Candidate qualification notice differs')
            return outcome
        except Exception as error:
            outcome['error'] = str(error)
            return outcome
        finally:
            outcome['candidate_exists'] = output.exists()
            if output.exists():
                data = output.read_bytes()
                outcome['candidate_bytes'] = len(data)
                outcome['candidate_sha256'] = digest(data)


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
        label(path)
        setattr(args, name, path)
    if args.report.exists() or args.workdir.exists():
        parser.error('Work directory and report must be fresh')
    if digest(args.example.read_bytes()) != args.example_sha256:
        parser.error('Frozen example differs')

    reference = json.loads(REFERENCE.read_text())
    if reference['hashes_before'] != reference['hashes_after']:
        parser.error('Committed reference has inconsistent pins')
    helper = ROOT / 'docs/testing/DeleteCandidateFixture.java'
    originals = []
    inputs = []
    for profile in ('V2016', 'V2019'):
        for state in ('before', 'delete-one', 'delete-null', 'delete-all'):
            path = args.inputs / f'Candidate-{profile}-{state}.accdb'
            inputs.append(path)
            if state == 'before':
                originals.append(path)
    required = [helper, args.jar, *inputs]
    for path in required:
        expected = reference['hashes_before'].get(label(path))
        if expected is None or digest(path.read_bytes()) != expected:
            parser.error(f'Input does not match committed reference: {label(path)}')
    pins = [Path(__file__), helper, OBSERVATIONS, REFERENCE, args.example, args.jar, *inputs]
    before_pins = pin_files(pins)
    sources = {}
    for path in SOURCE_PATHS:
        completed = subprocess.run(
            ['git', 'show', f'{args.compiled_revision}:{path}'], cwd=ROOT,
            capture_output=True, check=True, timeout=30,
        )
        sources[path] = digest(completed.stdout)
    args.workdir.mkdir(parents=True)
    replay = Replay(args.workdir)
    fatal = None
    try:
        for source in originals:
            metadata = replay.getters(source, args.jar, helper)
            before = source.read_bytes()
            slots = payloads(before, metadata)
            for table in TABLES[:-1]:
                identifiers = list(metadata[table]['rows'])
                selections = {
                    'first': {identifiers[0]},
                    'middle': {identifiers[len(identifiers) // 2]},
                    'last': {identifiers[-1]},
                    'null': {3},
                    'subset': {1, 3},
                    'all': set(identifiers),
                }
                for name, deleted in selections.items():
                    output = args.workdir / f'{source.stem}-{table}-{name}.accdb'
                    requests = []
                    for ordinal, identifier in enumerate(identifiers):
                        if identifier in deleted:
                            _, _, start, end = slots[table][identifier]
                            requests.append(f'{ordinal}:{before[start:end].hex()}')
                    outcome = replay.candidate(
                        args.example, source, output, table, requests, name,
                    )
                    if 'error' in outcome:
                        continue
                    try:
                        actual = replay.getters(output, args.jar, helper)
                        outcome['preservation'] = verify(
                            before, output.read_bytes(), metadata, actual, table, deleted,
                        )
                        outcome['agrees'] = True
                    except Exception as error:
                        outcome['error'] = str(error)
            target = 'Indexed'
            _, _, start, end = slots[target][1]
            expected = before[start:end]
            for mode in ('stale', 'duplicate', 'existing-output'):
                output = args.workdir / f'{source.stem}-{mode}.accdb'
                outcome = {
                    'source': label(source),
                    'output': label(output),
                    'selection': mode,
                    'expected_refusal': True,
                    'agrees': False,
                }
                replay.outcomes.append(outcome)
                requests = [f'0:{expected.hex()}']
                if mode == 'stale':
                    corrupted = bytes([expected[0] ^ 1]) + expected[1:]
                    requests[0] = f'0:{corrupted.hex()}'
                elif mode == 'duplicate':
                    requests.append(requests[0])
                else:
                    output.write_bytes(EXISTING_OUTPUT)
                command = [args.example, source, output, target]
                for request in requests:
                    command.extend(['--delete', request])
                record = replay.run(command)
                if mode == 'existing-output':
                    unchanged = output.read_bytes() == EXISTING_OUTPUT
                else:
                    unchanged = not output.exists()
                outcome['candidate_exists'] = output.exists()
                if output.exists():
                    outcome['candidate_sha256'] = digest(output.read_bytes())
                outcome['command_ordinal'] = len(replay.commands)
                outcome['agrees'] = (
                    record['status'] not in (None, 0)
                    and record['stdout'] == ''
                    and unchanged
                )
        # Continue from independently written reference deletions. These inputs
        # already contain tombstones; current getter order defines each ordinal.
        for profile in ('V2016', 'V2019'):
            original_path = args.inputs / f'Candidate-{profile}-before.accdb'
            original_metadata = replay.getters(original_path, args.jar, helper)
            for state in ('delete-one', 'delete-null'):
                source = args.inputs / f'Candidate-{profile}-{state}.accdb'
                metadata = replay.getters(source, args.jar, helper)
                before = source.read_bytes()
                slots = payloads(before, metadata)
                for table in TABLES[:-1]:
                    identifiers = list(metadata[table]['rows'])
                    ordinal = len(identifiers) // 2
                    identifier = identifiers[ordinal]
                    _, _, start, end = slots[table][identifier]
                    requests = [f'{ordinal}:{before[start:end].hex()}']
                    output = args.workdir / f'{source.stem}-{table}-existing-tombstones.accdb'
                    outcome = replay.candidate(
                        args.example, source, output, table, requests, 'existing-tombstones',
                    )
                    outcome['coverage_family'] = 'reference-input-live-ordinal'
                    outcome['selected_live_ordinal'] = ordinal
                    outcome['selected_id'] = identifier
                    if 'error' in outcome:
                        continue
                    try:
                        actual = replay.getters(output, args.jar, helper)
                        outcome['preservation'] = verify(
                            before, output.read_bytes(), metadata, actual, table,
                            {identifier}, slot_metadata=original_metadata,
                        )
                        outcome['agrees'] = True
                    except Exception as error:
                        outcome['error'] = str(error)
    except Exception as error:
        fatal = str(error)
    after_pins = pin_files(pins)
    disagreements = 0
    for item in replay.outcomes:
        if not item['agrees']:
            disagreements += 1
    report = {
        'scope': 'Engineering DELETE candidate; original getters and independent physical preservation; no native Access SQL',
        'compiled_revision': args.compiled_revision,
        'compiled_source_sha256': sources,
        'reference_report': label(REFERENCE),
        'hashes_before': before_pins,
        'hashes_after': after_pins,
        'pins_unchanged': before_pins == after_pins,
        'outcomes': replay.outcomes,
        'commands': replay.commands,
        'fatal_error': fatal,
        'disagreements': disagreements,
    }
    args.report.write_text(json.dumps(report, indent=2) + '\n')
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
