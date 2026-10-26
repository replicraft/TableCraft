"""Replay whole original DELETE selections with explicit frozen-example provenance."""

import argparse
import json
from pathlib import Path
import subprocess

from common import (NEGATIVE, POSITIVE, PROFILES, add_getter_arguments,
                    check_getter, oracle_command, pin, write_report)
import wire

CANDIDATE_TIMEOUT_SECONDS = 60
EXPECTED_RECORD_COLUMNS = 2


def snapshot(path):
    tables, payload = {}, []
    for line in path.read_text().splitlines():
        fields = line.split('\t')
        if fields[0] == 'TABLE':
            tables[fields[1]] = {
                'count': int(fields[2]), 'definition': int(fields[3]),
                'rows': [], 'indexes': {},
            }
        elif fields[0] == 'ROW':
            tables[fields[1]]['rows'].append(fields[2:])
        elif fields[0] == 'INDEX':
            tables[fields[1]]['indexes'][fields[2]] = {
                'physical': int(fields[3]), 'root': int(fields[4]),
                'kind': int(fields[5]), 'height': int(fields[6]),
                'owned_count': int(fields[7]), 'historical': int(fields[8]),
                'entries': int(fields[10]), 'order': [], 'keys': [],
            }
        elif fields[0] == 'ORDER':
            tables[fields[1]]['indexes'][fields[2]]['order'].append(int(fields[3]))
        elif fields[0] == 'KEY':
            tables[fields[1]]['indexes'][fields[2]]['keys'].append(
                (fields[5], int(fields[3]), int(fields[4])))
        elif fields[0] == 'PAYLOAD':
            payload.append(fields[1:])
    return tables, payload


def raw_rows(raw, definition, getters):
    by_ref = {(int(row[1]), int(row[2])): int(row[0]) for row in getters}
    map_offset = definition * wire.PAGE_SIZE + wire.TABLE_OWNED_MAP_OFFSET
    owned = wire.members(raw, wire.u32(raw, map_offset))
    result = []
    for number in owned:
        page = wire.page(raw, number)
        assert page[:len(wire.DATA_SIGNATURE)] == wire.DATA_SIGNATURE
        assert wire.u32(page, wire.PAGE_OWNER_OFFSET) == definition
        for slot in range(wire.u16(page, wire.DATA_ROW_COUNT_OFFSET)):
            slot_offset = wire.DATA_SLOT_START + wire.SLOT_WIDTH * slot
            word = wire.u16(page, slot_offset)
            if word & wire.DELETED_SLOT_MASK:
                continue
            assert word & wire.SLOT_FLAGS_MASK == 0
            payload = wire.row(raw, (number << wire.REFERENCE_SLOT_BITS) | slot)
            assert wire.u16(payload, 0) == EXPECTED_RECORD_COLUMNS
            result.append({
                'id': by_ref[(number, slot)], 'payload': payload,
                'word_offset': number * wire.PAGE_SIZE + slot_offset,
            })
    assert len(result) == len(getters)
    return result


def cell(raw, reference, target):
    number = reference >> wire.REFERENCE_SLOT_BITS
    slot = reference & wire.REFERENCE_SLOT_MASK
    page = wire.page(raw, number)
    slot_offset = wire.DATA_SLOT_START + wire.SLOT_WIDTH * slot
    start = wire.u16(page, slot_offset) & wire.SLOT_START_MASK
    payload = wire.row(raw, reference)
    if payload[0] == wire.INLINE_MAP:
        relative = target - wire.u32(payload, wire.MAP_BODY_OFFSET)
        byte_offset = wire.INLINE_BITS_OFFSET + relative // wire.BITS_PER_BYTE
        assert relative >= 0 and byte_offset < len(payload)
        absolute = number * wire.PAGE_SIZE + start + byte_offset
    else:
        assert payload[0] == wire.INDIRECT_MAP
        block, relative = divmod(target, wire.BITMAP_BITS)
        pointer = wire.u32(payload, wire.MAP_BODY_OFFSET + block * wire.POINTER_WIDTH)
        assert pointer
        assert wire.page(raw, pointer)[:wire.BITMAP_BODY_OFFSET] == wire.BITMAP_SIGNATURE
        absolute = (pointer * wire.PAGE_SIZE + wire.BITMAP_BODY_OFFSET
                    + relative // wire.BITS_PER_BYTE)
    return absolute, 1 << (relative % wire.BITS_PER_BYTE)


def preservation(before, after, table, deleted, old_snapshot, new_snapshot):
    assert len(before) == len(after)
    allowed = bytearray(len(before))

    def allow(start, length):
        allowed[start:start + length] = bytes([1]) * length

    definition = table['definition']
    count_offset = definition * wire.PAGE_SIZE + wire.TABLE_ROW_COUNT_OFFSET
    allow(count_offset, wire.POINTER_WIDTH)
    assert wire.u32(after, count_offset) == table['count'] - len(deleted)
    for row in deleted:
        offset = row['word_offset']
        expected = wire.u16(before, offset) | wire.TOMBSTONE_MASK
        assert wire.u16(after, offset) == expected
        allow(offset, wire.SLOT_WIDTH)

    retired, handled, expected_map_bytes = [], set(), {}
    for name, old in old_snapshot.items():
        current = new_snapshot[name]
        for field in ['physical', 'root', 'historical']:
            assert old[field] == current[field]
        if old['physical'] in handled:
            continue
        handled.add(old['physical'])
        usage, root = wire.physical(before, definition, old['physical'])
        assert (usage, root) == wire.physical(after, definition, old['physical'])
        original_pages = wire.members(before, usage)
        current_pages = wire.members(after, usage)
        assert set(current_pages).issubset(original_pages) and root in current_pages
        removed = sorted(set(original_pages) - set(current_pages))
        retired.extend(removed)
        for number in removed:
            assert number != root
            start = number * wire.PAGE_SIZE
            header_end = start + wire.RETIRED_HEADER_WIDTH
            assert after[start:header_end] == bytes(wire.RETIRED_HEADER_WIDTH)
            assert before[header_end:start + wire.PAGE_SIZE] == after[header_end:start + wire.PAGE_SIZE]
            allow(start, wire.RETIRED_HEADER_WIDTH)
            for reference, setting in [(usage, False), (wire.GLOBAL_MAP_REFERENCE, True)]:
                offset, mask = cell(before, reference, number)
                initial = before[offset]
                assert bool(initial & mask) != setting
                expected = expected_map_bytes.setdefault(offset, initial)
                expected_map_bytes[offset] = expected | mask if setting else expected & ~mask

        for number in current_pages:
            start = number * wire.PAGE_SIZE
            old_page, new_page = wire.page(before, number), wire.page(after, number)
            assert old_page[wire.PAGE_VERSION_OFFSET] == new_page[wire.PAGE_VERSION_OFFSET]
            owner_span = slice(wire.PAGE_OWNER_OFFSET, wire.INDEX_PREVIOUS_OFFSET)
            assert old_page[owner_span] == new_page[owner_span]
            if wire.u32(after, count_offset):
                assert old_page[:wire.PAGE_SIGNATURE_WIDTH] == new_page[:wire.PAGE_SIGNATURE_WIDTH]
                assert old_page[wire.INDEX_HEIGHT_OFFSET] == new_page[wire.INDEX_HEIGHT_OFFSET]
            else:
                assert number == root and new_page[0] == wire.LEAF_KIND
                assert new_page[wire.INDEX_HEIGHT_OFFSET] == 0
                link_width = wire.INDEX_PREFIX_OFFSET - wire.INDEX_PREVIOUS_OFFSET
                assert new_page[wire.INDEX_PREVIOUS_OFFSET:wire.INDEX_PREFIX_OFFSET] == bytes(link_width)
            old_end = wire.PAGE_SIZE - wire.u16(old_page, wire.INDEX_FREE_OFFSET)
            new_end = wire.PAGE_SIZE - wire.u16(new_page, wire.INDEX_FREE_OFFSET)
            assert old_page[max(old_end, new_end):] == new_page[max(old_end, new_end):]
            allow(start, wire.PAGE_KIND_WIDTH)
            allow(start + wire.INDEX_FREE_OFFSET, wire.SLOT_WIDTH)
            allow(start + wire.INDEX_PREVIOUS_OFFSET,
                  wire.INDEX_MASK_START - wire.INDEX_PREVIOUS_OFFSET)
            allow(start + wire.INDEX_MASK_START,
                  max(old_end, new_end) - wire.INDEX_MASK_START)
        map_offset = definition * wire.PAGE_SIZE + wire.TABLE_OWNED_MAP_OFFSET
        original_map = wire.u32(before, map_offset)
        assert wire.row(before, original_map) == wire.row(after, original_map)

    assert len(retired) == len(set(retired))
    for offset, value in expected_map_bytes.items():
        assert after[offset] == value
        allow(offset, 1)
    for offset, (old, new) in enumerate(zip(before, after)):
        assert allowed[offset] or old == new, ('unexpected changed byte', offset)
    return retired


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--corpus', type=Path, required=True)
    parser.add_argument('--capture-dir', type=Path, required=True)
    parser.add_argument('--observations', type=Path, required=True)
    parser.add_argument('--qualification', type=Path, required=True)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--example-sha256', required=True)
    parser.add_argument('--compiled-revision', required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--allow-internal-chain-refusal', action='store_true',
                        help='Historical d569 profile only; default expects all positive targets')
    add_getter_arguments(parser)
    args = parser.parse_args()
    check_getter(args)
    assert not args.report.exists()
    assert pin(args.example)['sha256'] == args.example_sha256
    assert len(args.compiled_revision) == 40
    assert all(c in '0123456789abcdef' for c in args.compiled_revision)
    args.workdir.mkdir(parents=True, exist_ok=False)
    commands, results = [], []
    source_files = sorted(args.corpus.glob('Encoded-*.accdb'))
    source_pins = [pin(p) for p in source_files]
    observations = json.loads(args.observations.read_text())
    assert len(source_files) == 8
    captured_hashes = {item['input']['sha256'] for item in observations['files']}
    assert {item['sha256'] for item in source_pins} == captured_hashes
    for profile in PROFILES:
        source = args.corpus / f'Encoded-{profile}-before.accdb'
        before = source.read_bytes()
        original, payloads = snapshot(args.capture_dir / f'Encoded-{profile}-before.getters.tsv')
        for name in POSITIVE + NEGATIVE:
            table = original[name]
            roots = raw_rows(before, table['definition'], table['rows'])
            states = (['delete-tie', 'delete-null', 'delete-all']
                      if name in POSITIVE else ['negative-delete'])
            for state in states:
                index = len(results)
                label = f'{index:02}-{profile}-{name}-{state}'
                selected = [(ordinal, r) for ordinal, r in enumerate(roots)
                            if state == 'delete-all' or r['id'] == (4 if state == 'delete-tie' else 1)]
                assert selected
                request_file = args.workdir / (label + '.tsv')
                request_file.write_text(''.join(f"{ordinal}\t{r['payload'].hex()}\n" for ordinal, r in selected))
                target = args.workdir / (label + '.accdb')
                argv = [str(args.example), str(source), str(target), name, '--delete-file', str(request_file)]
                stdout, stderr = args.workdir / (label + '.stdout'), args.workdir / (label + '.stderr')
                with stdout.open('xb') as out, stderr.open('xb') as err:
                    completed = subprocess.run(argv, stdout=out, stderr=err, timeout=CANDIDATE_TIMEOUT_SECONDS)
                command = {
                    'argv': argv, 'exit_code': completed.returncode,
                    'stdout': pin(stdout), 'stderr': pin(stderr),
                }
                command_index = len(commands)
                commands.append(command)
                known_internal_refusal = (args.allow_internal_chain_refusal
                                          and name == 'TextModernNode'
                                          and state == 'delete-all')
                expected_refusal = name in NEGATIVE or known_internal_refusal
                result = {
                    'profile': profile, 'table': name, 'state': state,
                    'expected_refusal': expected_refusal, 'selected_rows': len(selected),
                    'command': command_index, 'request_file': pin(request_file),
                    'refusal_class': ('unsupported_profile' if name in NEGATIVE else
                                      'known_internal_chain' if known_internal_refusal else None),
                }
                if expected_refusal:
                    agreed = (completed.returncode != 0 and not target.exists()
                              and stdout.read_bytes() == b''
                              and b'unsupported' in stderr.read_bytes().lower())
                    result.update({
                        'agreed': agreed, 'candidate_exists': target.exists(),
                        'diagnostic': stderr.read_text(),
                    })
                elif completed.returncode == 0:
                    assert target.exists()
                    assert stdout.read_text().strip() == f'candidate_deleted_rows={len(selected)}'
                    after = target.read_bytes()
                    output = args.workdir / (label + '.getters.tsv')
                    errors = args.workdir / (label + '.getters.stderr')
                    oracle = oracle_command(args, target)
                    with output.open('xb') as out, errors.open('xb') as err:
                        getter = subprocess.run(oracle, stdout=out, stderr=err, timeout=CANDIDATE_TIMEOUT_SECONDS)
                    commands.append({
                        'argv': oracle, 'exit_code': getter.returncode,
                        'stdout': pin(output), 'stderr': pin(errors),
                    })
                    assert getter.returncode == 0
                    actual, actual_payload = snapshot(output)
                    reference_path = args.capture_dir / f'Encoded-{profile}-{state}.getters.tsv'
                    reference, reference_payload = snapshot(reference_path)
                    selected_ids = {r['id'] for _, r in selected}
                    assert actual[name]['rows'] == reference[name]['rows']
                    assert actual[name]['count'] == table['count'] - len(selected)
                    for field in ['order', 'keys', 'historical', 'physical', 'root']:
                        for key, old in table['indexes'].items():
                            if field == 'order':
                                expected = [identifier for identifier in old[field]
                                            if identifier not in selected_ids]
                            elif field == 'keys':
                                retained_refs = {(int(r[1]), int(r[2])) for r in actual[name]['rows']}
                                expected = [entry for entry in old[field] if (entry[1], entry[2]) in retained_refs]
                            else:
                                expected = old[field]
                            assert actual[name]['indexes'][key][field] == expected
                            if field != 'keys':
                                reference_value = reference[name]['indexes'][key][field]
                                assert actual[name]['indexes'][key][field] == reference_value
                    assert actual_payload == payloads == reference_payload
                    for other in original:
                        if other != name:
                            assert actual[other] == original[other]
                    retired = preservation(
                        before, after, table, [row for _, row in selected],
                        table['indexes'], actual[name]['indexes'],
                    )
                    for item in actual[name]['indexes'].values():
                        raw_keys, _, _ = wire.decode_tree(after, item['root'], table['definition'])
                        assert raw_keys == item['keys']
                    result.update({
                        'agreed': True, 'candidate': pin(target), 'getter': pin(output),
                        'retired_pages': retired, 'actual_rows': actual[name]['count'],
                        'exact_retained_key_and_reference_match': True,
                        'independent_preservation_proof': True,
                    })
                else:
                    result.update({'agreed': False, 'diagnostic': stderr.read_text()})
                results.append(result)
                print(label, 'agreed', result['agreed'], 'exit', completed.returncode, flush=True)
                assert [pin(path) for path in source_files] == source_pins
    report = {
        'scope': 'Public-getter DELETE replay; no native Access save/reopen qualification',
        'driver': pin(args.example), 'compiled_revision': args.compiled_revision,
        'helper': pin(args.helper_source), 'jar': pin(args.jar),
        'runner': pin(Path(__file__)), 'wire_reader': pin(Path(wire.__file__)),
        'qualification': pin(args.qualification), 'original_observations': pin(args.observations),
        'allow_internal_chain_refusal': args.allow_internal_chain_refusal,
        'sources': source_pins, 'commands': commands, 'results': results,
        'controls': len(results),
        'disagreements': sum(not result['agreed'] for result in results),
        'source_hashes_unchanged': True,
    }
    write_report(args.report, report)
    print('controls', len(results), 'disagreements', report['disagreements'], flush=True)
    if report['disagreements']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
