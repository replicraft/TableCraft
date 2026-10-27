"""Frozen public compressed-Memo observations; import/verify modes perform no DB calls."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import traceback

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from slot_preview_originals import common, facts
import delete_reference_maps as MAP

PAGE = 4096
SLOT_START = 14
SLOT_BYTES = 2
SLOT_MASK = 0x1FFF
DELETED = 0x8000
FORWARDED = 0x4000
LENGTH_MASK = 0x3FFFFFFF
TYPE_MASK = 0xC0000000
INLINE = 0x80000000
SINGLE = 0x40000000
DESCRIPTOR_BYTES = 12
MAX_CHUNKS = 1024
TABLE = 'CompressedMemos'
IDENTITY = 91
SENTINEL = 92
MEMOS = ('CompactMemo', 'RawMemo')
LONG_COLUMNS = MEMOS + ('OLE',)
GETTER_TAGS = {'TABLE', 'PROPERTY', 'COLUMN', 'ROW', 'RAWROW', 'INDEX', 'INDEX_COLUMN', 'ENTRY'}
EXPECTED_MAPS = {'GlobalFree', 'TableOwned', 'TableHint'} | {
    name + role for name in LONG_COLUMNS for role in ('Owned', 'Hint')
}


def pin(path):
    return common.pin(path)


def save(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=True) + '\n')


def number(data, offset, width=2):
    value = data[offset:offset + width]
    if len(value) != width:
        raise AssertionError('Truncated independent field')
    return int.from_bytes(value, 'little')


def frame(raw, page, slot):
    base = page * PAGE
    assert page < len(raw) // PAGE and 0 <= slot < number(raw, base + 12) <= 255
    word = number(raw, base + SLOT_START + slot * SLOT_BYTES)
    start = base + (word & SLOT_MASK)
    end = base + PAGE if slot == 0 else base + (number(raw, base + SLOT_START + (slot - 1) * SLOT_BYTES) & SLOT_MASK)
    assert base + SLOT_START <= start < end <= base + PAGE
    body = raw[start:end]
    return {'page': page, 'slot': slot, 'word': word, 'start': start, 'end': end,
            'body_bytes': len(body), 'body_sha256': hashlib.sha256(body).hexdigest(),
            'body_hex': body.hex()}


def final_frame(raw, location):
    page, slot = location['page'], location['slot']
    seen = set()
    path = []
    while True:
        assert (page, slot) not in seen and len(seen) < 16
        seen.add((page, slot))
        value = frame(raw, page, slot)
        path.append(value)
        if not value['word'] & FORWARDED:
            return value, path
        reference = number(raw, value['start'], 4)
        page, slot = reference >> 8, reference & 255


def decode_stream(encoded):
    if not encoded.startswith(b'\xff\xfe'):
        return encoded.decode('utf-16le', errors='strict')
    units = bytearray()
    narrow = True
    position = 2
    while position < len(encoded):
        first = encoded[position]
        if first == 0:
            narrow = not narrow
            position += 1
        elif narrow:
            units.extend((first, 0))
            position += 1
        else:
            assert position + 2 <= len(encoded)
            units.extend(encoded[position:position + 2])
            position += 2
    return units.decode('utf-16le', errors='strict')


def token_value(token):
    if token == 'NULL':
        return None
    kind, value = token.split(':', 1)
    if kind == 'String16':
        return bytes.fromhex(value).decode('utf-16le', errors='strict')
    if kind == 'Binary':
        return bytes.fromhex(value)
    assert kind == 'Integer'
    return int(value)


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
    result = {'present': present, 'range': [start, end], 'descriptor_hex': descriptor.hex(), 'chunks': []}
    if not present or not descriptor:
        result.update(kind='NULL', encoded_bytes=0, encoded_hex='', logical=None)
        return result
    assert len(descriptor) >= DESCRIPTOR_BYTES
    tagged = number(descriptor, 0, 4)
    kind, length = tagged & TYPE_MASK, tagged & LENGTH_MASK
    result.update(tag=kind, encoded_bytes=length, reference=number(descriptor, 4, 4), reserved_hex=descriptor[8:12].hex())
    encoded = bytearray()
    if kind == INLINE:
        assert len(descriptor) == DESCRIPTOR_BYTES + length
        result['kind'] = 'inline'
        encoded.extend(descriptor[DESCRIPTOR_BYTES:])
    else:
        assert kind in (0, SINGLE) and len(descriptor) == DESCRIPTOR_BYTES and length > 0
        result['kind'] = 'single' if kind else 'chain'
        reference = result['reference']
        seen = set()
        while reference:
            assert reference not in seen and len(seen) < MAX_CHUNKS
            seen.add(reference)
            row = frame(raw, reference >> 8, reference & 255)
            base = row['page'] * PAGE
            assert raw[base:base + 2] == b'\x01\x01' and raw[base + 4:base + 8] == b'LVAL'
            assert not row['word'] & (DELETED | FORWARDED)
            payload = raw[row['start']:row['end']]
            following = number(payload, 0, 4) if kind == 0 else 0
            encoded.extend(payload[4:] if kind == 0 else payload)
            result['chunks'].append(dict(row, reference=reference, next=following))
            reference = following
    assert len(encoded) == length
    logical = decode_stream(bytes(encoded)) if column['native_type'] == 12 else bytes(encoded)
    result.update(encoded_hex=encoded.hex(), encoded_sha256=hashlib.sha256(encoded).hexdigest())
    if isinstance(logical, str):
        result['logical'] = logical
        result['logical_utf8_bytes'] = len(logical.encode('utf-8'))
        result['logical_utf16_units'] = len(logical.encode('utf-16le')) // 2
    else:
        result['logical'] = {'binary_hex': logical.hex()}
    return result


def parse_metadata(text, raw):
    descriptions = {}
    values = {}
    formats = []
    for line in text.splitlines():
        fields = line.split('\t')
        if fields[0] in GETTER_TAGS:
            continue
        if fields[0] == 'FORMAT':
            formats.append(list(map(int, fields[1:])))
        elif fields[0] == 'MAP':
            assert fields[1] not in descriptions
            descriptions[fields[1]] = {'table_page': int(fields[2]), 'row_start': int(fields[3]),
                                       'row_end': int(fields[4]), 'reported_count': int(fields[5]), 'all_members': []}
        elif fields[0] == 'MEMBER':
            descriptions[fields[1]]['all_members'].append(int(fields[2]))
        else:
            assert fields[0] == 'VALUE' and fields[1] not in values
            values[fields[1]] = fields[2:]
    assert formats == [[4096, 64, 4076, 12]] and set(descriptions) == EXPECTED_MAPS
    result = {}
    for name, description in descriptions.items():
        description['file_members'] = [page for page in description['all_members'] if page < len(raw) // PAGE]
        decoded = MAP.decode_map(raw, description, PAGE, len(raw) // PAGE)
        decoded['all_members'] = description['all_members']
        decoded['row_wire_hex'] = raw[decoded['row_start']:decoded['row_end']].hex()
        result[name] = decoded
    return result, values


def declarations(raw, table):
    base = table['definition'] * PAGE
    physical = number(raw, base + 51, 4)
    logical = number(raw, base + 47, 4)
    column_start = base + 63 + physical * 12
    assert number(raw, base + 41) == number(raw, base + 45) == 4
    position = column_start + 4 * 25
    for label, column in sorted(table['columns'].items(), key=lambda pair: pair[1]['position']):
        at = column_start + column['position'] * 25
        column['flags'] = raw[at + 15]
        column['extended_flags'] = raw[at + 16]
        column['raw_block_hex'] = raw[at:at + 25].hex()
        assert number(raw, at + 5) == column['number']
        width = number(raw, position)
        position += 2
        assert raw[position:position + width].decode('utf-16le') == label
        position += width
    position += physical * 52 + logical * 28
    names = []
    for _ in range(logical):
        width = number(raw, position)
        position += 2
        names.append(raw[position:position + width].decode('utf-16le'))
        position += width
    assert set(names) == set(table['indexes'])
    pairs = {}
    for _ in range(5):
        column = number(raw, position)
        if column == 0xFFFF:
            return pairs
        assert column not in pairs
        pairs[column] = {'owned': number(raw, position + 2, 4), 'hint': number(raw, position + 6, 4)}
        position += 10
    raise AssertionError('Unexpected declaration list extent')


def snapshot(path, getter, metadata):
    raw = path.read_bytes()
    getter_text = '\n'.join(line for line in getter.read_text().splitlines() if line.split('\t', 1)[0] in GETTER_TAGS)
    tables = facts.parse(getter_text)
    assert set(tables) == {TABLE}
    table = tables[TABLE]
    assert table['count'] == 2 and set(table['rows']) == {IDENTITY, SENTINEL}
    pairs = declarations(raw, table)
    maps, values = parse_metadata(metadata.read_text(), raw)
    selected = {}
    root_path = []
    for identity, row in table['rows'].items():
        final, path_frames = final_frame(raw, row)
        body = raw[final['start']:final['end']]
        assert body.hex() == table['raw_rows'][identity]
        row['root_path'] = path_frames
        if identity != IDENTITY:
            continue
        root_path = path_frames
        for label in LONG_COLUMNS:
            column = table['columns'][label]
            value = long_value(raw, body, column)
            stored = token_value(row['values'][column['position']])
            decoded = value['logical']
            if isinstance(decoded, dict):
                decoded = bytes.fromhex(decoded['binary_hex'])
            assert decoded == stored
            getter_bytes = b'' if stored is None else stored.encode('utf-8') if isinstance(stored, str) else stored
            assert values[label] == ['NULL' if stored is None else 'PRESENT', str(len(getter_bytes)), hashlib.sha256(getter_bytes).hexdigest()]
            selected[label] = value
            number_ = column['number']
            for role, key in [('Owned', 'owned'), ('Hint', 'hint')]:
                description = maps[label + role]
                assert pairs[number_][key] == (description['map_page'] << 8) | description['map_slot']
            assert set(chunk['page'] for chunk in value['chunks']) <= set(maps[label + 'Owned']['file_members'])
    definition = table['definition'] * PAGE
    index_pages = {index['root'] for index in table['indexes'].values()}
    for index in table['indexes'].values():
        assert index['owned_pages'] == 1 and index['order'] == [91, 92]
        assert raw[index['root'] * PAGE:index['root'] * PAGE + 2] == b'\x04\x01'
    state = {'file': pin(path), 'getter': pin(getter), 'metadata': pin(metadata), 'table': table,
             'pairs': pairs, 'root_path': root_path, 'fields': selected, 'maps': maps,
             'live_count': number(raw, definition + 16, 4),
             'historical_counts_hex': raw[definition + 63:definition + 63 + number(raw, definition + 51, 4) * 12].hex(),
             'protected_pages': {str(page): hashlib.sha256(raw[page * PAGE:(page + 1) * PAGE]).hexdigest()
                                 for page in sorted({table['definition']} | index_pages)}}
    return raw, state


def prerequisites(profile, raw, state):
    assert raw[:4] == b'\x00\x01\x00\x00' and raw[4:20] == b'Standard ACE DB\0'
    assert raw[20] == {'V2016': 5, 'V2019': 6}[profile] and raw[21] == 0
    assert raw[62:66] == bytes.fromhex('fb8abc4e')
    table = state['table']
    assert table['columns_count'] == table['max_columns'] == 4 and not table['properties']
    assert set(table['columns']) == {'ID', 'CompactMemo', 'RawMemo', 'OLE'}
    for label, kind, flags, extended, length, variable, compressed in [
        ('ID', 4, 3, 0, 4, None, False),
        ('CompactMemo', 12, 2, 1, 0, 0, True),
        ('RawMemo', 12, 2, 0, 0, 1, False),
        ('OLE', 11, 2, 0, 0, 2, False),
    ]:
        field = table['columns'][label]
        assert (field['native_type'], field['flags'], field['extended_flags'], field['declared_bytes']) == (kind, flags, extended, length)
        assert field['compressed'] == compressed
        if variable is not None:
            assert field['variable'] == variable
    assert table['columns']['CompactMemo']['sort'] == table['columns']['RawMemo']['sort']
    assert set(table['indexes']) == {'PrimaryID', 'IDAsc', 'IDTwin'}
    assert table['indexes']['PrimaryID']['primary']
    assert table['indexes']['IDAsc']['physical'] == table['indexes']['IDTwin']['physical']
    assert table['indexes']['IDAsc']['root'] == table['indexes']['IDTwin']['root']
    assert all(index['columns'] == [['ID', 'true']] for index in table['indexes'].values())
    for label in MEMOS:
        assert state['fields'][label]['logical'] == 'seed' * 1200
        assert state['fields'][label]['kind'] == 'chain'
        assert state['maps'][label + 'Owned']['payload_type'] == 0
        assert state['maps'][label + 'Hint']['payload_type'] == 0
        assert len(state['maps'][label + 'Hint']['file_members']) <= 1
    assert state['fields']['OLE']['logical'] == {'binary_hex': bytes(range(256)).hex()}
    assert state['maps']['GlobalFree']['payload_type'] == 0
    assert state['maps']['TableOwned']['payload_type'] in (0, 1)
    assert state['live_count'] == 2



def map_cell(raw, description, page):
    start, end = description['row_start'], description['row_end']
    kind = raw[start]
    if kind == 0:
        relative = page - number(raw, start + 1, 4)
        assert 0 <= relative < (end - start - 5) * 8
        return start + 5 + relative // 8, 1 << (relative % 8)
    assert kind == 1
    width = (PAGE - 4) * 8
    pointer_at = start + 1 + (page // width) * 4
    assert pointer_at + 4 <= end
    bitmap = number(raw, pointer_at, 4)
    assert bitmap and bitmap < len(raw) // PAGE
    relative = page % width
    return bitmap * PAGE + 4 + relative // 8, 1 << (relative % 8)


def map_wire_proof(old_raw, raw, old_maps, new_maps, deltas):
    # Existing and future members are included. No in-file-only truncation can
    # hide an unrelated future free bit or a map-header/pointer change.
    proofs = {}
    for name, original in old_maps.items():
        candidate = new_maps[name]
        stable = all(original[key] == candidate[key] for key in
                     ('payload_type', 'map_page', 'map_slot', 'row_start', 'row_end', 'bitmap_pages'))
        if not stable:
            proofs[name] = {'representation_changed': True,
                            'exact_bit_only_claim': False}
            continue
        start, end = original['row_start'], original['row_end']
        spans = [(start, end)]
        spans.extend((page * PAGE, (page + 1) * PAGE) for page in original['bitmap_pages'])
        edits = {}
        for page in deltas[name]['removed'] + deltas[name]['added']:
            before_cell = map_cell(old_raw, original, page)
            assert before_cell == map_cell(raw, candidate, page)
            offset, mask = before_cell
            value = edits.get(offset, old_raw[offset])
            value = value | mask if page in deltas[name]['added'] else value & ~mask
            edits[offset] = value
        for first, last in spans:
            expected = bytearray(old_raw[first:last])
            for offset, value in edits.items():
                if first <= offset < last:
                    expected[offset - first] = value
            assert bytes(expected) == raw[first:last], 'Map bytes outside exact membership deltas changed'
        proofs[name] = {'representation_changed': False, 'exact_bit_only_claim': True,
                        'complete_spans_checked': len(spans), 'changed_bitmap_bytes': len(edits)}
    return proofs


def cumulative_chunks(raw, saved):
    result = []
    for chunk in saved:
        after_body = raw[chunk['start']:chunk['end']]
        word = number(raw, chunk['page'] * PAGE + SLOT_START + chunk['slot'] * SLOT_BYTES)
        result.append({'source': chunk['source'], 'column': chunk['column'],
                       'reference': chunk['reference'], 'before_sha256': chunk['body_sha256'],
                       'after_sha256': hashlib.sha256(after_body).hexdigest(),
                       'body_unchanged': after_body.hex() == chunk['body_hex'],
                       'slot_word_before': chunk['word'], 'slot_word_after': word})
    return result


def compare(before, after, selection, payload):
    old_raw, old = before
    raw, new = after
    column = selection['column']
    old_table, table = old['table'], new['table']
    for key in ['count', 'definition', 'columns_count', 'max_columns', 'columns', 'properties', 'indexes']:
        assert old_table[key] == table[key], key
    position = table['columns'][column]['position']
    for identity, old_row in old_table['rows'].items():
        row = table['rows'][identity]
        assert (old_row['page'], old_row['slot']) == (row['page'], row['slot'])
        if identity != IDENTITY:
            assert old_row == row and old_table['raw_rows'][identity] == table['raw_rows'][identity]
        else:
            for index, value in enumerate(old_row['values']):
                if index != position:
                    assert value == row['values'][index]
    for other in LONG_COLUMNS:
        if other != column:
            assert old['fields'][other]['descriptor_hex'] == new['fields'][other]['descriptor_hex']
            assert old['fields'][other]['encoded_hex'] == new['fields'][other]['encoded_hex']
    requested = None if selection['kind'] == 'null' else payload.decode('utf-8', errors='strict')
    actual = new['fields'][column]['logical']
    exact = requested == actual
    if not selection['normalization_probe']:
        assert exact, 'Unexpected requested/getter mismatch; stop without adapting the selector'
    assert old['live_count'] == new['live_count'] and old['historical_counts_hex'] == new['historical_counts_hex']
    for page, digest in old['protected_pages'].items():
        start = int(page) * PAGE
        assert hashlib.sha256(raw[start:start + PAGE]).hexdigest() == digest
    old_chunks = []
    for label, description in old['fields'].items():
        for chunk in description['chunks']:
            original_body = old_raw[chunk['start']:chunk['end']]
            after_body = raw[chunk['start']:chunk['end']]
            word = number(raw, chunk['page'] * PAGE + SLOT_START + chunk['slot'] * SLOT_BYTES)
            old_chunks.append({'column': label, 'reference': chunk['reference'], 'before_sha256': chunk['body_sha256'],
                               'after_sha256': hashlib.sha256(after_body).hexdigest(),
                               'body_unchanged': original_body == after_body, 'slot_word_before': chunk['word'], 'slot_word_after': word})
    deltas = {}
    for label, previous in old['maps'].items():
        current = new['maps'][label]
        deltas[label] = {'added': sorted(set(current['all_members']) - set(previous['all_members'])),
                         'removed': sorted(set(previous['all_members']) - set(current['all_members'])),
                         'before_type': previous['payload_type'], 'after_type': current['payload_type'],
                         'before_row_wire_hex': previous['row_wire_hex'], 'after_row_wire_hex': current['row_wire_hex']}
    wire_proofs = map_wire_proof(old_raw, raw, old['maps'], new['maps'], deltas)
    changed = []
    allowed = set(old['maps']['TableOwned']['file_members']) | set(new['maps']['TableOwned']['file_members'])
    allowed |= set(old['maps'][column + 'Owned']['file_members']) | set(new['maps'][column + 'Owned']['file_members'])
    for label in ['GlobalFree', 'TableOwned', 'TableHint', column + 'Owned', column + 'Hint']:
        for state in [old, new]:
            allowed.add(state['maps'][label]['map_page'])
            allowed.update(state['maps'][label]['bitmap_pages'])
    for page in range(len(raw) // PAGE):
        previous = old_raw[page * PAGE:(page + 1) * PAGE]
        current = raw[page * PAGE:(page + 1) * PAGE]
        if previous != current:
            assert page in allowed, 'Unrelated original page changed'
            changed.append({'page': page, 'new': not previous, 'before_sha256': hashlib.sha256(previous).hexdigest(),
                            'after_sha256': hashlib.sha256(current).hexdigest(), 'header_before': previous[:16].hex(),
                            'header_after': current[:16].hex()})
    return {'public_update_completed': True, 'requested_value_equal': exact,
            'normalization_probe': selection['normalization_probe'],
            'requested_utf8_hex': None if requested is None else requested.encode('utf-8').hex(),
            'actual_utf8_hex': None if actual is None else actual.encode('utf-8').hex(),
            'all_other_values_rows_indexes_preserved': True, 'protected_metadata_indexes_preserved': True,
            'old_chunk_bodies': old_chunks, 'map_deltas': deltas, 'map_wire_proofs': wire_proofs,
            'changed_pages': changed,
            'file_growth_bytes': len(raw) - len(old_raw), 'before_kind': old['fields'][column]['kind'],
            'after_kind': new['fields'][column]['kind']}


def verify_freeze(freeze_path):
    freeze = json.loads(freeze_path.read_text())
    for item in freeze['inputs']:
        assert pin(item['path']) == item, item['path']
    plan = json.loads((ROOT / freeze['plan']['path']).read_text())
    assert pin(freeze['plan']['path']) == freeze['plan']
    assert plan['profiles'] == ['V2016', 'V2019']
    count = len(plan['cases'])
    assert 0 < count <= 92
    profile_processes = 2 if 'originals' in plan else 3
    assert plan['limits']['processes'] == 1 + len(plan['profiles']) * profile_processes + count * 3
    assert len({case['id'] for case in plan['cases']}) == count
    seen = {'original:V2016', 'original:V2019'}
    for case in plan['cases']:
        assert case['source_ref'] in seen and case['kind'] in ('null', 'text')
        assert case['column'] in MEMOS and pin(case['payload']['path']) == case['payload']
        common.safe_label(case['id'])
        seen.add(case['id'])
    return freeze, plan


def execute(freeze_path, work):
    freeze, plan = verify_freeze(freeze_path)
    directory = common.fresh_work(work)
    classes = directory / 'classes'
    originals = directory / 'originals'
    outputs = directory / 'updates'
    for path in [classes, originals, outputs]:
        path.mkdir()
    capture = common.Capture(directory)
    report_path = directory / 'report.json'
    report = {'scope': 'Actual new public-writer observations; no TableCraft candidates or native Access execution.',
              'freeze': pin(freeze_path), 'plan': freeze['plan'], 'sources': freeze['inputs'],
              'prerequisites': [], 'cases': [], 'oracles': [], 'completed': False}

    def command(argv, label):
        assert len(capture.commands) < plan['limits']['processes']
        result = capture.run(argv, label)
        report['commands'] = capture.commands
        save(report_path, report)
        assert result['status'] == 0 and not result['timed_out'], 'Bounded command failed; no retry'
        assert not (ROOT / result['stderr']['path']).read_bytes(), 'Unexpected diagnostic retained'
        return result

    jar = plan['jar']['path']
    helper = plan['helper']['path']
    try:
        command(['javac', '-cp', jar, '-d', str(classes.relative_to(ROOT)), helper], 'compile')
        report['classes'] = [pin(path) for path in sorted(classes.rglob('*.class'))]
        assert report['classes']
        java = ['java', '-Dstdout.encoding=UTF-8', '-cp', os.pathsep.join([jar, str(classes.relative_to(ROOT))]), 'CompressedMemoUpdateProbe']
        known = {}
        source_paths = {}
        ancestor_chunks = {}
        original_pins = []
        for profile in plan['profiles']:
            path = originals / (profile + '-before.accdb')
            if 'originals' in plan:
                accepted = plan['originals'][profile]
                source = ROOT / accepted['path']
                assert pin(source) == accepted
                shutil.copyfile(source, path)
                assert pin(path)['sha256'] == accepted['sha256']
            else:
                command(java + ['create', str(path.relative_to(ROOT)), profile], profile + '-create')
            source_paths['original:' + profile] = path
            original_pins.append(pin(path))
        report['accepted_source_originals'] = plan.get('originals', {})
        report['originals_before'] = original_pins
        # BOTH combined metadata/getter calls and all preconditions complete
        # before the first descendant copy is opened for update.
        for profile in plan['profiles']:
            path = source_paths['original:' + profile]
            result = command(java + ['inspect', str(path.relative_to(ROOT))], profile + '-prerequisite')
            output = ROOT / result['stdout']['path']
            state = snapshot(path, output, output)
            prerequisites(profile, *state)
            known['original:' + profile] = state
            ancestor_chunks['original:' + profile] = [
                dict(chunk, source='original:' + profile, column=label)
                for label, value in state[1]['fields'].items() for chunk in value['chunks']
            ]
            report['prerequisites'].append({'profile': profile, 'command': result, 'state': state[1], 'matched': True})
        assert len(report['prerequisites']) == 2
        report['prerequisites_completed_before_updates'] = True
        save(report_path, report)
        oracle_lines = {profile: [] for profile in plan['profiles']}
        oracle_expected = {profile: {} for profile in plan['profiles']}
        for case in plan['cases']:
            source = source_paths[case['source_ref']]
            source_pin = pin(source)
            before = known[case['source_ref']]
            assert before[1]['file'] == source_pin
            output = outputs / (case['id'] + '.accdb')
            assert not output.exists()
            shutil.copyfile(source, output)
            assert pin(output)['sha256'] == source_pin['sha256']
            action = {'selection': case, 'source_before': source_pin, 'completed': False}
            report['cases'].append(action)
            save(report_path, report)
            action['update'] = command(java + ['update', str(output.relative_to(ROOT)), case['column'], case['kind'], case['payload']['path']], case['id'] + '-update')
            getter = command(java + ['getter', str(output.relative_to(ROOT))], case['id'] + '-getter')
            metadata = command(java + ['metadata', str(output.relative_to(ROOT))], case['id'] + '-metadata')
            after = snapshot(output, ROOT / getter['stdout']['path'], ROOT / metadata['stdout']['path'])
            action['after'] = after[1]
            action['comparison'] = compare(before, after, case, (ROOT / case['payload']['path']).read_bytes())
            action['cumulative_old_chunks'] = cumulative_chunks(after[0], ancestor_chunks[case['source_ref']])
            ancestor_chunks[case['id']] = ancestor_chunks[case['source_ref']] + [
                dict(chunk, source=case['id'], column=label)
                for label, value in after[1]['fields'].items() for chunk in value['chunks']
            ]
            action['completed'] = True
            assert pin(source) == source_pin
            source_paths[case['id']] = output
            known[case['id']] = after
            value = after[1]['fields'][case['column']]
            if value['kind'] != 'NULL':
                oracle_lines[case['profile']].append(case['id'] + '\t' + case['column'] + '\t' + value['encoded_hex'])
                oracle_expected[case['profile']][case['id']] = token_value(after[1]['table']['rows'][IDENTITY]['values'][after[1]['table']['columns'][case['column']]['position']])
            save(report_path, report)
        assert len(report['cases']) == len(plan['cases']) and all(case['completed'] for case in report['cases'])
        for profile in plan['profiles']:
            corpus = directory / (profile + '-encoded-oracle.tsv')
            corpus.write_text('\n'.join(oracle_lines[profile]) + '\n', encoding='ascii')
            original = source_paths['original:' + profile]
            result = command(java + ['decode', str(original.relative_to(ROOT)), str(corpus.relative_to(ROOT))], profile + '-decode')
            actual = {}
            for line in (ROOT / result['stdout']['path']).read_text().splitlines():
                tag, identity, column, token = line.split('\t')
                assert tag == 'DECODE' and column in MEMOS and identity not in actual
                actual[identity] = token_value(token)
            assert actual == oracle_expected[profile]
            report['oracles'].append({'profile': profile, 'input': pin(corpus), 'command': result, 'matched_streams': len(actual)})
        for item in list(plan.get('originals', {}).values()) + original_pins:
            assert pin(item['path']) == item
        for state in known.values():
            assert pin(state[1]['file']['path']) == state[1]['file']
        verify_freeze(freeze_path)
        report['completed'] = True
        report['public_updates'] = len(plan['cases'])
        report['processes'] = len(capture.commands)
        assert report['processes'] == plan['limits']['processes']
        report['normalization_observations'] = [case['selection']['id'] for case in report['cases'] if not case['comparison']['requested_value_equal']]
        report['originals_and_all_source_parents_unchanged'] = True
        save(report_path, report)
        print(json.dumps({'completed': True, 'updates': len(plan['cases']), 'processes': report['processes'], 'normalization_observations': report['normalization_observations']}), flush=True)
    except BaseException as error:
        report['failure'] = {'class': type(error).__name__, 'message': str(error), 'traceback': traceback.format_exc()}
        report['commands'] = capture.commands
        save(report_path, report)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--freeze', type=Path, required=True)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--verify-only', action='store_true')
    mode.add_argument('--execute', action='store_true')
    parser.add_argument('--work', type=Path)
    arguments = parser.parse_args()
    freeze_path = common.path(arguments.freeze)
    if arguments.verify_only:
        _, plan = verify_freeze(freeze_path)
        print(json.dumps({'verified': True, 'cases': len(plan['cases']), 'process_cap': plan['limits']['processes'], 'external_processes': 0}))
        return
    assert arguments.work is not None
    execute(freeze_path, arguments.work)


if __name__ == '__main__':
    main()
