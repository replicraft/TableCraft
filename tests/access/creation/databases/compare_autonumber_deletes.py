#!/usr/bin/env python3
"""Compare disposable AutoNumber DELETE candidates with public-writer observations."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

import capture_autonumber_deletes as reference
from encoded_delete_originals import wire

ROOT = Path(__file__).resolve().parents[2]
NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
TABLE_COUNT_OFFSET = 16
SLOT_START = 14
SLOT_WIDTH = 2
TOMBSTONE_FLAGS = 0xC000
INDEX_FREE_OFFSET = 2
INDEX_PREFIX_OFFSET = 24
WORD_WIDTH = 2
COUNTER_WIDTH = 4
COMPILED_REVISION = '1e776246313e114c7ea0d8f4b18fe3fc4162ef7f'
BINARY_SHA256 = 'aeb9a06d16d7d6356fe7ab0f2c53afd9f96f851da587ec0b407b5aff3c088f08'


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def metadata(args, path, log):
    command = ['java', '-cp', reference.os.pathsep.join(map(str, (args.jar, args.classes))),
               reference.HELPER, 'oracle', str(path)]
    return reference.parse(reference.invoke(command, log))


def preserve(before, after, original, actual, target, removed):
    assert len(before) == len(after)
    allowed = []
    changed = 0
    for name in reference.TABLES:
        old = original[name]
        new = actual[name]
        expected_rows = [row for row in old['rows'] if name != target or row[0] not in removed]
        assert new['rows'] == expected_rows
        for field in ('definition_page', 'long_counter', 'long_offset', 'complex_counter',
                      'complex_offset', 'page_size', 'columns', 'properties'):
            assert new[field] == old[field]
        assert set(new['indexes']) == set(old['indexes'])
        count_offset = old['definition_page'] * wire.PAGE_SIZE + TABLE_COUNT_OFFSET
        assert wire.u32(after, count_offset) == len(expected_rows)
        if name == target:
            allowed.append((count_offset, count_offset + COUNTER_WIDTH))
        for identifier, score, page, slot in old['rows']:
            offset = page * wire.PAGE_SIZE + SLOT_START + slot * SLOT_WIDTH
            expected = wire.u16(before, offset)
            if name == target and identifier in removed:
                expected |= TOMBSTONE_FLAGS
                allowed.append((offset, offset + SLOT_WIDTH))
            assert wire.u16(after, offset) == expected
        roots = set()
        for index, declaration in old['indexes'].items():
            current = new['indexes'][index]
            assert current[:2] == declaration[:2] and current[3:] == declaration[3:]
            assert current[2] == len(expected_rows)
            assert new['order'].get(index, []) == [row[0] for row in expected_rows]
            root = declaration[1]
            if root in roots:
                continue
            roots.add(root)
            entries, leaves, nodes = wire.decode_tree(before, root, old['definition_page'])
            actual_entries, actual_leaves, actual_nodes = wire.decode_tree(
                after, root, old['definition_page'])
            assert not nodes and not actual_nodes and len(leaves) == len(actual_leaves) == 1
            removed_locations = {(row[2], row[3]) for row in old['rows']
                                 if name == target and row[0] in removed}
            assert actual_entries == [entry for entry in entries
                                      if (entry[1], entry[2]) not in removed_locations]
            assert actual_leaves[0] == leaves[0] | {'entries': len(expected_rows)}
            if name == target:
                base = root * wire.PAGE_SIZE
                old_end = wire.PAGE_SIZE - wire.u16(before, base + INDEX_FREE_OFFSET)
                new_end = wire.PAGE_SIZE - wire.u16(after, base + INDEX_FREE_OFFSET)
                allowed.extend(((base + INDEX_FREE_OFFSET, base + INDEX_FREE_OFFSET + WORD_WIDTH),
                                (base + INDEX_PREFIX_OFFSET, base + INDEX_PREFIX_OFFSET + WORD_WIDTH),
                                (base + wire.INDEX_MASK_START, base + max(old_end, new_end))))
    for offset, (left, right) in enumerate(zip(before, after, strict=True)):
        if left != right:
            assert any(start <= offset < end for start, end in allowed), offset
            changed += 1
    return changed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--binary-sha256', required=True)
    parser.add_argument('--build-receipt', type=Path, required=True)
    parser.add_argument('--reference', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--classes', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    assert args.binary_sha256 == BINARY_SHA256 == reference.pin(args.binary)['sha256']
    assert reference.pin(args.jar)['sha256'] == reference.JAR_SHA256
    receipt = json.loads(args.build_receipt.read_text())
    assert receipt['compiled_revision'] == COMPILED_REVISION
    assert receipt['frozen_example']['sha256'] == BINARY_SHA256
    immutable_sources = []
    for item in receipt['source_pins']:
        raw = subprocess.check_output(['git', 'show', f"{COMPILED_REVISION}:{item['path']}"], timeout=30)
        assert digest(raw) == item['sha256'] and len(raw) == item['bytes']
        immutable_sources.append(item)
    captured = json.loads(args.reference.read_text())
    for pin in captured['artifacts'] + captured['originals_after']:
        assert reference.pin(ROOT / pin['path']) == pin
    args.output.mkdir(parents=True, exist_ok=False)
    assert not args.report.exists()
    inputs = [reference.pin(ROOT / pin['path']) for pin in captured['originals_after']]
    controls = []
    for profile in reference.PROFILES:
        baseline = next(item for item in captured['observations']
                        if item['profile'] == profile and item['state'] == 'before')
        source = ROOT / baseline['artifact']['path']
        original = baseline['tables']
        before = source.read_bytes()
        for table in reference.TABLES:
            for selection, removed in reference.SELECTIONS.items():
                stem = f'{profile}-{table}-{selection}'
                candidate = args.output / f'{stem}.accdb'
                command = [str(args.binary), str(source), str(candidate), table]
                for ordinal, row in enumerate(original[table]['rows']):
                    if row[0] in removed:
                        payload = wire.row(before, (row[2] << wire.REFERENCE_SLOT_BITS) | row[3])
                        command.extend(['--delete', f'{ordinal}:{payload.hex()}'])
                result = reference.invoke(command, args.output / f'{stem}-candidate.json')
                assert result == f'candidate_deleted_rows={len(removed)}\n'
                transcript = json.loads((args.output / f'{stem}-candidate.json').read_text())
                assert transcript['stderr'] == NOTICE
                actual = metadata(args, candidate, args.output / f'{stem}-getter.json')
                reference.validate(actual, table, removed)
                changed = preserve(before, candidate.read_bytes(), original, actual, table, removed)
                public = next(item for item in captured['observations']
                              if item['profile'] == profile and item.get('table') == table
                              and item['state'] == selection)
                assert actual == public['tables']
                candidate_pin = reference.pin(candidate)
                appended = args.output / f'{stem}-append.accdb'
                append_command = ['java', '-cp', reference.os.pathsep.join(map(str, (args.jar, args.classes))),
                                  reference.HELPER, 'copy', str(candidate), str(appended), table, 'append']
                response = reference.invoke(append_command, args.output / f'{stem}-append-action.json')
                assert response == f'APPEND\t{table}\t261\n'
                appended_model = metadata(args, appended, args.output / f'{stem}-append-getter.json')
                reference.validate(appended_model, table, removed, table)
                public_append = next(item for item in captured['observations']
                                     if item['profile'] == profile and item.get('table') == table
                                     and item['state'] == selection + '-append')
                # Physical append locations may reflect preserved candidate padding;
                # logical rows, columns, counters and index membership must agree.
                for name in reference.TABLES:
                    expected = public_append['tables'][name]
                    observed = appended_model[name]
                    assert [row[:2] for row in observed['rows']] == [row[:2] for row in expected['rows']]
                    for field in ('long_counter', 'complex_counter', 'columns', 'properties', 'order', 'indexes'):
                        assert observed[field] == expected[field]
                assert reference.pin(candidate) == candidate_pin
                controls.append({'profile': profile, 'table': table, 'selection': selection,
                                 'deleted_rows': len(removed), 'changed_bytes': changed,
                                 'counter_after_delete': 260, 'observed_appended_id': 261,
                                 'candidate': candidate_pin, 'append': reference.pin(appended)})
    assert inputs == [reference.pin(ROOT / pin['path']) for pin in inputs]
    report = {'schema_version': 1, 'scope': 'Ordinary LONG AutoNumber DELETE engineering evidence; no native Access save qualification',
              'compiled_revision': COMPILED_REVISION, 'controls': controls,
              'candidate_count': len(controls), 'append_count': len(controls), 'disagreements': 0,
              'immutable_compiled_sources': immutable_sources,
              'artifacts': [reference.pin(Path(__file__)), reference.pin(args.binary),
                            reference.pin(args.build_receipt), reference.pin(args.reference),
                            reference.pin(Path(reference.__file__)), reference.pin(Path(wire.__file__)),
                            *inputs, *[reference.pin(path) for path in sorted(args.output.glob('*.json'))]]}
    with args.report.open('x') as output:
        json.dump(report, output, indent=2)
        output.write('\n')
    print(f'{len(controls)} candidates and append descendants passed: {args.report}')


if __name__ == '__main__':
    main()
