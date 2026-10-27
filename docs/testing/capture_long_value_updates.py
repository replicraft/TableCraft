"""Execute the fixed60 long-value observations after a verified metadata gate.

This parameterized action phase preserves the executed private runner separately.
It does not run implicitly during imports or rewrite accepted originals.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common, facts
import delete_reference_maps as MAP

RUN = None
PAGE_BYTES = 4096
OFFSET_MASK = 0x1FFF
FORWARDED = 0x4000
DELETED = 0x8000
SLOT_START = 14
SLOT_BYTES = 2
ROW_COUNT_OFFSET = 12
FREE_OFFSET = 2
TABLE = 'Sentinel'
IDENTITY = 91
TABLE_ROWS_OFFSET = 16
HISTORY_OFFSET = 67


def number(data, offset, width=2):
    return int.from_bytes(data[offset:offset + width], 'little')


def frame(raw, page, slot):
    base = page * PAGE_BYTES
    assert slot < number(raw, base + ROW_COUNT_OFFSET)
    word = number(raw, base + SLOT_START + SLOT_BYTES * slot)
    start = base + (word & OFFSET_MASK)
    end = base + PAGE_BYTES if slot == 0 else base + (number(raw, base + SLOT_START + SLOT_BYTES * (slot - 1)) & OFFSET_MASK)
    assert start <= end <= len(raw)
    body = raw[start:end]
    return {'page': page, 'slot': slot, 'word': word, 'start': start, 'end': end,
            'body_bytes': len(body), 'body_sha256': hashlib.sha256(body).hexdigest(), 'body_hex': body.hex()}


def final_frame(raw, row):
    page, slot = row['page'], row['slot']
    seen = set()
    path = []
    while True:
        assert (page, slot) not in seen and len(seen) < 32
        seen.add((page, slot))
        value = frame(raw, page, slot)
        path.append(value)
        if not value['word'] & FORWARDED:
            return value, path
        reference = number(raw, value['start'], 4)
        page, slot = reference >> 8, reference & 255


def long_value(raw, body, column):
    count = number(body, 0)
    mask = len(body) - (count + 7) // 8
    variables = number(body, mask - 2)
    variable = column['variable']
    assert variable < variables
    start = number(body, mask - 4 - variable * 2)
    end = number(body, mask - 6 - variable * 2)
    assert start <= end <= mask - 2 * (variables + 2)
    present = bool(body[mask + column['number'] // 8] & (1 << (column['number'] % 8)))
    descriptor = body[start:end]
    result = {'present': present, 'row_range': [start, end], 'wire_hex': descriptor.hex(), 'chunks': []}
    if not present or not descriptor:
        result['kind'] = 'NULL'
        result['encoded_bytes'] = 0
        return result
    assert len(descriptor) >= 12
    tagged = number(descriptor, 0, 4)
    kind, length = tagged & 0xC0000000, tagged & 0x3FFFFFFF
    result.update(encoded_bytes=length, tag=kind, reference=number(descriptor, 4, 4), reserved_hex=descriptor[8:12].hex())
    payload = bytearray()
    if kind == 0x80000000:
        result['kind'] = 'inline'
        assert len(descriptor) == 12 + length
        payload.extend(descriptor[12:])
    else:
        assert kind in (0, 0x40000000) and len(descriptor) == 12 and length > 0
        result['kind'] = 'single' if kind else 'chain'
        reference = result['reference']
        seen = set()
        while reference:
            assert reference not in seen and len(seen) < 1024
            seen.add(reference)
            value = frame(raw, reference >> 8, reference & 255)
            assert value['word'] & (FORWARDED | DELETED) == 0
            page = value['page']
            assert raw[page * PAGE_BYTES:page * PAGE_BYTES + 2] == b'\x01\x01'
            assert raw[page * PAGE_BYTES + 4:page * PAGE_BYTES + 8] == b'LVAL'
            chunk = raw[value['start']:value['end']]
            next_reference = number(chunk, 0, 4) if kind == 0 else 0
            payload.extend(chunk[4:] if kind == 0 else chunk)
            result['chunks'].append(dict(value, reference=reference, next=next_reference))
            reference = next_reference
    assert len(payload) == length
    result['encoded_sha256'] = hashlib.sha256(payload).hexdigest()
    # Both selected source Memo declarations are uncompressed. Verify actual
    # UTF16 getters rather than infer field intent from descriptor tags.
    logical = bytes(payload).decode('utf-16le').encode('utf-8') if column['native_type'] == 12 else bytes(payload)
    result['getter_bytes'] = len(logical)
    result['getter_sha256'] = hashlib.sha256(logical).hexdigest()
    return result


def parse_metadata(text, raw):
    maps, values = {}, {}
    format_seen = False
    for line in text.splitlines():
        fields = line.split('\t')
        if fields[0] == 'FORMAT':
            assert not format_seen and list(map(int, fields[1:])) == [4096, 64, 4076, 12]
            format_seen = True
        elif fields[0] == 'MAP':
            assert fields[1] not in maps
            maps[fields[1]] = {'table_page': int(fields[2]), 'row_start': int(fields[3]),
                               'row_end': int(fields[4]), 'reported_count': int(fields[5]), 'all_members': []}
        elif fields[0] == 'MEMBER':
            maps[fields[1]]['all_members'].append(int(fields[2]))
        elif fields[0] == 'VALUE':
            values[fields[1]] = fields[2:]
        else:
            assert fields[0] == 'ROW'
    assert format_seen and set(maps) == {'GlobalFree', 'TableOwned', 'TableHint', 'MemoOwned', 'MemoHint', 'OLEOwned', 'OLEHint'}
    decoded = {}
    for name, description in maps.items():
        description['file_members'] = [value for value in description['all_members'] if value < len(raw) // PAGE_BYTES]
        value = MAP.decode_map(raw, description, PAGE_BYTES, len(raw) // PAGE_BYTES)
        value['all_members'] = description['all_members']
        value['row_wire_hex'] = raw[value['row_start']:value['row_end']].hex()
        value['bitmap_hashes'] = {str(page): hashlib.sha256(raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]).hexdigest() for page in value['bitmap_pages']}
        decoded[name] = value
    return decoded, values


def snapshot(path, getter, metadata):
    raw = path.read_bytes()
    tables = facts.parse(getter.read_text())
    table = tables[TABLE]
    maps, values = parse_metadata(metadata.read_text(), raw)
    final, chain = final_frame(raw, table['rows'][IDENTITY])
    body = raw[final['start']:final['end']]
    assert body.hex() == table['raw_rows'][IDENTITY]
    selected = {}
    for label in ['Memo', 'OLE']:
        selected[label] = long_value(raw, body, table['columns'][label])
        actual = selected[label]
        if actual['kind'] == 'NULL':
            assert values[label][0] == 'NULL'
        else:
            assert values[label] == ['PRESENT', str(actual['getter_bytes']), actual['getter_sha256']]
        assert set(chunk['page'] for chunk in actual['chunks']) <= set(maps[label + 'Owned']['file_members'])
    definition = table['definition'] * PAGE_BYTES
    state = {'file': common.pin(path), 'getter': common.pin(getter), 'metadata': common.pin(metadata),
             'root_path': chain, 'fields': selected, 'maps': maps,
             'live_count': number(raw, definition + TABLE_ROWS_OFFSET, 4),
             'historical_index_count': number(raw, definition + HISTORY_OFFSET, 4),
             'definition_sha256': hashlib.sha256(raw[definition:definition + PAGE_BYTES]).hexdigest(),
             'map_page_headers': {}}
    pages = set()
    for role, value in maps.items():
        pages.add(value['map_page'])
        pages.update(value['bitmap_pages'])
        if role != 'GlobalFree':
            pages.update(value['file_members'])
    for page in sorted(pages):
        data = raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
        state['map_page_headers'][str(page)] = {'prefix_hex': data[:16].hex(), 'free': number(data, 2),
                                               'slots': number(data, 12), 'sha256': hashlib.sha256(data).hexdigest()}
    return raw, tables, state


def compare(before, after, selection):
    old_raw, old_tables, old = before
    raw, tables, new = after
    column = selection['column']
    payload = Path(selection['payload']['path']).read_bytes()
    expected = 'NULL' if selection['kind'] == 'null' else ('MemoSHA:' + hashlib.sha256(payload).hexdigest()) if column == 'Memo' else f'OLESHA:{len(payload)}:' + hashlib.sha256(payload).hexdigest()
    position = 1 if column == 'Memo' else 2
    assert tables[TABLE]['rows'][IDENTITY]['values'][position] == expected
    assert set(old_tables) == set(tables)
    for name, table in old_tables.items():
        actual = tables[name]
        for key in ['count', 'definition', 'columns_count', 'max_columns', 'columns', 'properties', 'indexes']:
            assert table[key] == actual[key], (name, key)
        assert set(table['rows']) == set(actual['rows'])
        for identity, row in table['rows'].items():
            if name == TABLE and identity == IDENTITY:
                assert (row['page'], row['slot']) == (actual['rows'][identity]['page'], actual['rows'][identity]['slot'])
                for other, value in enumerate(row['values']):
                    if other != position:
                        assert value == actual['rows'][identity]['values'][other]
                continue
            assert row == actual['rows'][identity]
            assert table['raw_rows'][identity] == actual['raw_rows'][identity]
    other = 'OLE' if column == 'Memo' else 'Memo'
    assert old['fields'][other]['wire_hex'] == new['fields'][other]['wire_hex']
    assert old['live_count'] == new['live_count'] and old['historical_index_count'] == new['historical_index_count']
    old_chain_bodies = []
    for field, description in old['fields'].items():
        for chunk in description['chunks']:
            after_body = raw[chunk['start']:chunk['end']]
            base = chunk['page'] * PAGE_BYTES
            word_offset = base + SLOT_START + SLOT_BYTES * chunk['slot']
            old_chain_bodies.append({'field': field, 'reference': chunk['reference'],
                                    'before_sha256': chunk['body_sha256'],
                                    'after_sha256': hashlib.sha256(after_body).hexdigest(),
                                    'body_unchanged': after_body == old_raw[chunk['start']:chunk['end']],
                                    'slot_word_before': chunk['word'], 'slot_word_after': number(raw, word_offset),
                                    'page_prefix_after': raw[base:base + 16].hex()})
    deltas = {}
    for name, prior in old['maps'].items():
        current = new['maps'][name]
        deltas[name] = {'added_members': sorted(set(current['all_members']) - set(prior['all_members'])),
                        'removed_members': sorted(set(prior['all_members']) - set(current['all_members'])),
                        'before_type': prior['payload_type'], 'after_type': current['payload_type'],
                        'before_row_wire_hex': prior['row_wire_hex'], 'after_row_wire_hex': current['row_wire_hex']}
    pages = []
    for page in range(len(raw) // PAGE_BYTES):
        previous = old_raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
        current = raw[page * PAGE_BYTES:(page + 1) * PAGE_BYTES]
        if previous != current:
            pages.append({'page': page, 'new': not previous, 'before_sha256': hashlib.sha256(previous).hexdigest(),
                          'after_sha256': hashlib.sha256(current).hexdigest(), 'header_before': previous[:16].hex(),
                          'header_after': current[:16].hex(), 'changed_bytes': sum(left != right for left, right in zip(previous, current)) if previous else len(current)})
    return {'requested_value_exact': True, 'all_other_values_rows_indexes_preserved': True,
            'other_long_descriptor_preserved': True, 'live_and_history_counts_preserved': True,
            'old_chunk_bodies': old_chain_bodies, 'map_deltas': deltas, 'changed_pages': pages,
            'before_kind': old['fields'][column]['kind'], 'after_kind': new['fields'][column]['kind'],
            'file_growth_bytes': len(raw) - len(old_raw)}


def main():
    global RUN
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--metadata-gate', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--getter-classes', type=Path, required=True)
    parser.add_argument('--probe-classes', type=Path, required=True)
    parser.add_argument('--helper', type=Path, default=Path(__file__).with_name('LongValueUpdateProbe.java'))
    arguments = parser.parse_args()
    plan = json.loads(arguments.plan.read_text())
    assert json.loads(arguments.metadata_gate.read_text())['completed']
    RUN = Path(common.fresh_work(arguments.work).relative_to(ROOT))
    destinations = {case['output']: str(RUN / (common.safe_label(case['id']) + '.accdb')) for case in plan['cases']}
    assert len(destinations) == len(plan['cases']) == 60
    for case in plan['cases']:
        case['source'] = destinations.get(case['source'], case['source'])
        case['output'] = destinations[case['output']]
        assert case['table'] == TABLE and case['ID'] == IDENTITY
        assert case['column'] in ('Memo', 'OLE') and case['kind'] in ('null', 'text', 'binary')
    assert not (RUN / 'actions-report.json').exists()
    assert common.pin(arguments.helper)['sha256'] == plan['helper']['sha256']
    assert common.pin(Path(plan['jar']['path'])) == plan['jar']
    for selection in plan['cases']:
        assert common.pin(Path(selection['payload']['path'])) == selection['payload']
    original_pins = [source['source'] for source in plan['sources']]
    for item in original_pins:
        assert common.pin(item['path']) == item
    report = {'scope': 'Actual public-writer observations; no engine candidates/native Access execution.',
              'plan': common.pin(arguments.plan), 'runner': common.pin(Path(__file__)),
              'helper': plan['helper'], 'jar': plan['jar'], 'originals_before': original_pins,
              'executed_class': common.pin(arguments.probe_classes / 'LongValueUpdateProbe.class'),
              'getter_class': common.pin(arguments.getter_classes / 'SlotUpdateFixture.class'),
              'cases': [], 'completed': False}
    commands = common.Capture(RUN)
    metadata_gate = json.loads(arguments.metadata_gate.read_text())
    known = {}
    for source, captured in zip(plan['sources'], metadata_gate['calls'], strict=True):
        known[source['source']['path']] = snapshot(Path(source['source']['path']), Path(source['cached_getter']['path']), Path(captured['stdout']['path']))
    java = ['java', '-Dstdout.encoding=UTF-8', '-cp', os.pathsep.join([str(arguments.probe_classes), plan['jar']['path']]), 'LongValueUpdateProbe']
    getter_java = ['java', '-Dstdout.encoding=UTF-8', '-cp', os.pathsep.join([str(arguments.getter_classes), plan['jar']['path']]), 'SlotUpdateFixture']
    for selection in plan['cases']:
        source, output = Path(selection['source']), Path(selection['output'])
        before_pin = common.pin(source)
        assert selection['source'] in known and not output.exists()
        shutil.copyfile(source, output)
        assert common.pin(output)['sha256'] == before_pin['sha256']
        action = {'selection': selection, 'source_before': before_pin}
        report['cases'].append(action)
        try:
            public = commands.run(java + ['update', str(output), selection['column'], selection['kind'], selection['payload']['path']], selection['id'] + '-update')
            action['public_update'] = public
            assert public['status'] == 0 and not public['timed_out']
            assert not Path(public['stderr']['path']).read_bytes()
            getter = commands.run(getter_java + ['oracle', str(output)], selection['id'] + '-getter')
            action['getter'] = getter
            assert getter['status'] == 0 and not getter['timed_out'] and not Path(getter['stderr']['path']).read_bytes()
            metadata = commands.run(java + ['metadata', str(output)], selection['id'] + '-metadata')
            action['metadata'] = metadata
            assert metadata['status'] == 0 and not metadata['timed_out'] and not Path(metadata['stderr']['path']).read_bytes()
            after = snapshot(output, Path(getter['stdout']['path']), Path(metadata['stdout']['path']))
            action['before'] = known[selection['source']][2]
            action['after'] = after[2]
            action['comparison'] = compare(known[selection['source']], after, selection)
            known[str(output)] = after
            action['source_after'] = common.pin(source)
            assert action['source_after'] == before_pin
            for item in original_pins:
                assert common.pin(item['path']) == item
            action['accepted_observation'] = True
        finally:
            report['command_count_after_metadata_gate'] = len(commands.commands)
            (RUN / 'actions-report.json').write_text(json.dumps(report, indent=2) + '\n')
    report['originals_after'] = [common.pin(item['path']) for item in original_pins]
    assert report['originals_after'] == original_pins
    report['completed'] = True
    report['public_updates'] = len(report['cases'])
    report['action_phase_processes'] = len(commands.commands)
    assert report['public_updates'] == 60 and report['action_phase_processes'] == 180
    (RUN / 'actions-report.json').write_text(json.dumps(report, indent=2) + '\n')
    print({'public_updates': 60, 'commands': report['action_phase_processes'], 'completed': True})


if __name__ == '__main__':
    main()
