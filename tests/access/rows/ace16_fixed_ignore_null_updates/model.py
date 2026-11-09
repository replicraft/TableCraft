"""Complete engine-policy images from original bytes and immutable requests only."""
import hashlib
from pathlib import Path

from ace16_fixed_ignore_null_updates import source

PAGE = 4096
ENTRY_START = 480
CAPACITY = 3616
BITMAP_START = 27


def before(path, getter, profile):
    """Requalify every table, then retain only original raw/getter facts."""
    facts = source.inspect(path, getter, profile)
    raw = Path(path).read_bytes()
    tables, descriptions, properties, absent, counters = source.parse(Path(getter).read_text())
    declarations = {
        name: source.native.declarations.declarations(raw, table)
        for name, table in tables.items()
    }
    return dict(raw=raw, tables=tables, declarations=declarations, facts=facts,
                properties=properties, absent=absent, counters=counters)


def atom(value, ascending):
    """Independent literal I32 component encoding, never the proof encoder."""
    if value is None:
        encoded = b'\x00'
    else:
        assert type(value) is int and -(1 << 31) <= value < 1 << 31
        encoded = b'\x7f' + ((value + (1 << 31)) & 0xffffffff).to_bytes(4, 'big')
    return encoded if ascending else bytes(255 - byte for byte in encoded)


def values_key(row, columns):
    return tuple(row[name] for name, ascending in columns)


def omitted(key):
    return all(value is None for value in key)


def wire(row, columns):
    return b''.join(atom(row[name], ascending) for name, ascending in columns)


def entries(rows, table, columns):
    result = []
    for identity, row in rows.items():
        key = values_key(row, columns)
        if omitted(key):
            continue
        root = table['rows'][identity]
        reference = root['page'] << 8 | root['slot']
        result.append((wire(row, columns), reference, key))
    result.sort(key=lambda item: (item[0], item[1]))
    return result


def intent(state, case):
    action = case['action']
    table = state['tables'][action['table']]
    index = table['indexes']['ValueOrder']
    assert index['physical'] == 1 and not index['primary'] and index['flags'] & 2
    columns = index['columns']
    original = table['values']
    final = {identity: dict(row) for identity, row in original.items()}
    roots = sorted((row['page'] << 8 | row['slot'], identity)
                   for identity, row in table['rows'].items())
    assert len(roots) == len(original) == len(set(reference for reference, identity in roots))
    ordinals = {identity: position for position, (reference, identity) in enumerate(roots)}
    assignments = []
    changed = set()
    for identity, updates in action['updates']:
        assert identity in original and identity not in changed
        assert updates and set(updates) <= {'Left', 'Right'}
        changed.add(identity)
        for name, value in updates.items():
            assert value is None or type(value) is int and -(1 << 31) <= value < 1 << 31
            root = table['rows'][identity]
            assignments.append(dict(table=action['table'], column=name,
                                    ordinal=ordinals[identity], identity=identity,
                                    logical_reference=root['page'] << 8 | root['slot'],
                                    expected=original[identity][name], value=value))
            final[identity][name] = value
    transitions = [(values_key(original[identity], columns), values_key(final[identity], columns))
                   for reference, identity in roots]
    original_families = {key for key, after in transitions if not omitted(key)}
    represented = [key for before, key in transitions if not omitted(key)]
    outcome = 'supported'
    # Required also applies to omitted final keys, before IgnoreNULL filtering.
    if index['flags'] & 8 and any(any(value is None for value in key)
                                for before, key in transitions):
        outcome = 'NullKey'
    elif index['flags'] & 1 and len(set(represented)) != len(represented):
        outcome = 'DuplicateKey'
    introduced = set()
    if outcome == 'supported':
        for old, new in transitions:
            if not omitted(old) or omitted(new):
                continue
            existed = new in original_families
            stays = any(before == new == after for before, after in transitions)
            replacement = any(not omitted(before) and after == new for before, after in transitions)
            if (existed and not stays) or (not existed and replacement):
                outcome = 'IndexedColumn'
                break
            if not existed:
                introduced.add(new)
    expected = case['outcome']
    assert outcome == expected, (case['id'], outcome, expected)
    return dict(table=table, index=index, original=original, final=final,
                assignments=assignments, history_delta=len(introduced), outcome=outcome)


def reframe(original, complete):
    """Retain source header, seed and inactive tail; replace only the active frame."""
    assert len(original) == PAGE and original[:2] == b'\x04\x01'
    assert original[8:24] == bytes(16) and original[26] == 0
    seed = int.from_bytes(original[24:26], 'little')
    common = min(map(len, complete)) if complete else 0
    if complete:
        first = complete[0]
        for entry in complete[1:]:
            length = 0
            while length < common and first[length] == entry[length]:
                length += 1
            common = length
    prefix = min(seed, common)
    total = sum(map(len, complete))
    saved = max(0, len(complete) - 1)
    if total - prefix * saved > CAPACITY:
        prefix = common
    active = total - prefix * saved
    assert active <= CAPACITY, 'Original-file shape falls outside fitting topology'
    after = bytearray(original)
    after[2:4] = (CAPACITY - active).to_bytes(2, 'little')
    after[24:26] = prefix.to_bytes(2, 'little')
    after[BITMAP_START:ENTRY_START] = bytes(ENTRY_START - BITMAP_START)
    end = 0
    for position, entry in enumerate(complete):
        payload = entry[0 if position == 0 else prefix:]
        after[ENTRY_START + end:ENTRY_START + end + len(payload)] = payload
        end += len(payload)
        after[BITMAP_START + end // 8] |= 1 << (end % 8)
    assert end == active
    return bytes(after), dict(seed=seed, prefix=prefix, active=active, free=CAPACITY - active)


def record_range(raw, row, owner):
    assert (row['page'], row['slot']) == (row['target_page'], row['target_slot'])
    base = row['page'] * PAGE
    page = raw[base:base + PAGE]
    assert page[:2] == b'\x01\x01' and int.from_bytes(page[4:8], 'little') == owner
    slot = row['slot']
    assert slot < int.from_bytes(page[12:14], 'little')
    word = int.from_bytes(page[14 + slot * 2:16 + slot * 2], 'little')
    assert word & 0xe000 == 0
    start = word & 0x1fff
    end = PAGE if slot == 0 else int.from_bytes(page[12 + slot * 2:14 + slot * 2], 'little') & 0x1fff
    assert 14 + 2 * int.from_bytes(page[12:14], 'little') <= start < end <= PAGE
    assert page[start:end] == row['raw'] and len(row['raw']) == 19 and row['raw'][:2] == b'\x04\x00'
    return base + start, base + end


def derive(state, case):
    change = intent(state, case)
    if change['outcome'] != 'supported':
        raise ValueError('Engine refusal has no expected candidate image: ' + change['outcome'])
    raw = state['raw']
    table, index = change['table'], change['index']
    owner = table['definition']
    columns = index['columns']
    old = entries(change['original'], table, columns)
    new = entries(change['final'], table, columns)
    old_wires = [key + reference.to_bytes(4, 'big') for key, reference, values in old]
    new_wires = [key + reference.to_bytes(4, 'big') for key, reference, values in new]
    observed, decoded = source.leaf(raw, index['root'], owner, columns)
    assert observed == old_wires
    declarations = state['declarations'][case['action']['table']]['columns']
    fields = {item['name']: item for item in declarations}
    assert list(fields) == ['ID', 'Left', 'Right', 'Marker']
    after = bytearray(raw)
    allowed = bytearray(len(raw))
    for assignment in change['assignments']:
        row = table['rows'][assignment['identity']]
        start, end = record_range(raw, row, owner)
        declaration = fields[assignment['column']]
        block = bytes.fromhex(declaration['raw_hex'])
        number = declaration['number']
        assert declaration['native'] == 4 and declaration['flags'] == 3 and declaration['extended'] == 0
        assert int.from_bytes(block[23:25], 'little') == 4
        offset = start + 2 + int.from_bytes(block[21:23], 'little')
        assert start + 2 <= offset and offset + 4 <= end - 1
        expected, value = assignment['expected'], assignment['value']
        assert source.records.scalar_value(raw[start:end], declaration) == expected
        mask = 1 << number
        assert bool(raw[end - 1] & mask) == (expected is not None)
        if value is not None and value != expected:
            after[offset:offset + 4] = value.to_bytes(4, 'little', signed=True)
            allowed[offset:offset + 4] = b'\xff' * 4
        if expected != value:
            after[end - 1] = (after[end - 1] & ~mask) | (mask if value is not None else 0)
            allowed[end - 1] |= mask
    root = index['root'] * PAGE
    if old_wires != new_wires:
        replacement, frame = reframe(raw[root:root + PAGE], new_wires)
        after[root:root + PAGE] = replacement
        allowed[root + 2:root + 4] = b'\xff' * 2
        allowed[root + 24:root + 26] = b'\xff' * 2
        old_active = CAPACITY - int.from_bytes(raw[root + 2:root + 4], 'little')
        limit = ENTRY_START + max(old_active, frame['active'])
        allowed[root + BITMAP_START:root + limit] = b'\xff' * (limit - BITMAP_START)
        assert replacement[limit:] == raw[root + limit:root + PAGE]
    else:
        frame = dict(unchanged=True)
    history = index['history'] + change['history_delta']
    assert 0 <= history < 1 << 32
    counter = owner * PAGE + index['history_offset']
    assert int.from_bytes(raw[counter:counter + 4], 'little') == index['history']
    if change['history_delta']:
        after[counter:counter + 4] = history.to_bytes(4, 'little')
        allowed[counter:counter + 4] = b'\xff' * 4
    result = bytes(after)
    actual, decoded = source.leaf(result, index['root'], owner, columns)
    assert actual == new_wires
    assert decoded == [(values, reference) for key, reference, values in new]
    for identity, row in table['rows'].items():
        start, end = record_range(raw, row, owner)
        values = {name: source.records.scalar_value(result[start:end], item)
                  for name, item in fields.items()}
        assert values == change['final'][identity]
        assert result[start:start + 2] == raw[start:start + 2]
    assert all((before ^ current) & ~mask == 0
               for before, current, mask in zip(raw, result, allowed, strict=True))
    differences = [offset for offset, (before, current) in enumerate(zip(raw, result, strict=True))
                   if before != current]
    return result, dict(id=case['id'], bytes=len(result), sha256=hashlib.sha256(result).hexdigest(),
                        source_sha256=hashlib.sha256(raw).hexdigest(), table=case['action']['table'],
                        assignments=change['assignments'], root=index['root'], physical=index['physical'],
                        eligible_before=len(old), eligible_after=len(new), frame=frame,
                        history_before=index['history'], history_after=history,
                        historical_delta=change['history_delta'], preserved_maps=True,
                        preserved_length=True, preserved_other_tables=True,
                        changed_bytes=len(differences), changed_offsets=differences,
                        image_authority='SOURCE plus original requests; no public AFTER/native output')
