"""Literal retained-height NODE DELETE model; never invokes a native writer."""
import hashlib
import struct

PAGE = 4096
ENTRY_AT = 480
MASK_AT = 27
MASK_BYTES = 453
SELECTED_IDS = (302, 354)


def require(ok, message):
    if not ok:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def u16(raw, at):
    return struct.unpack_from('<H', raw, at)[0]


def u32(raw, at):
    return struct.unpack_from('<I', raw, at)[0]


def key(value, reference, child=None):
    encoded = bytes([0x7f]) + struct.pack('>II', (value & 0xffffffff) ^ 0x80000000, reference)
    return encoded if child is None else encoded + struct.pack('>I', child)


def decode(raw, number, definition, node):
    page = raw[number * PAGE:(number + 1) * PAGE]
    require(len(page) == PAGE and page[:2] == bytes([3 if node else 4, 1])
            and u32(page, 4) == definition, 'Observed owned INDEX page')
    prefix = u16(page, 24)
    previous = 0
    entries = []
    first = b''
    for bit in range(MASK_BYTES * 8):
        if page[MASK_AT + bit // 8] & (1 << (bit % 8)) == 0:
            continue
        require(previous < bit <= PAGE - ENTRY_AT, 'Strict bounded entry ends')
        wire = page[ENTRY_AT + previous:ENTRY_AT + bit]
        if entries:
            require(prefix <= len(first), 'Bounded original prefix')
            wire = first[:prefix] + wire
        else:
            first = wire
        require(len(wire) == (13 if node else 9) and wire[0] == 0x7f,
                'Exact present-I32/ROW/optional-child original wire')
        value = int.from_bytes(wire[1:5], 'big') ^ 0x80000000
        value = value - (1 << 32) if value >= (1 << 31) else value
        reference = int.from_bytes(wire[5:9], 'big')
        child = int.from_bytes(wire[9:13], 'big') if node else None
        require(wire == key(value, reference, child), 'Independent wire inversion')
        entries.append((value, reference, child))
        previous = bit
    require(u16(page, 2) == PAGE - ENTRY_AT - previous, 'Original INDEX free counter')
    return entries, page


def records(raw, definition):
    rows = []
    for number in range(len(raw) // PAGE):
        page = raw[number * PAGE:(number + 1) * PAGE]
        if page[:2] != bytes([1, 1]) or u32(page, 4) != definition:
            continue
        count = u16(page, 12)
        require(count <= 255, 'Direct source directory ceiling')
        end = PAGE
        for slot in range(count):
            start = u16(page, 14 + 2 * slot)
            require(start & 0xe000 == 0 and 14 + 2 * count <= start < end,
                    'Direct original logical row')
            record = page[start:end]
            require(len(record) == 15 and record[:2] == bytes([3, 0]) and record[-1] == 7,
                    'Complete present three-I32 record')
            values = struct.unpack_from('<iii', record, 2)
            require(1 <= values[0] <= 800
                    and values == (values[0], values[0] - 401, (values[0] - 1) % 7),
                    'Original literal row recipe')
            rows.append({'reference': (number << 8) | slot, 'values': values,
                         'directory': number * PAGE + 14 + 2 * slot, 'word': start})
            end = start
    require(len(rows) == 800 and [row['values'][0] for row in rows] == list(range(1, 801)),
            'Complete original physical row order')
    return rows


def write_page(after, original, number, entries):
    require(entries, 'Every retained leaf/node remains nonempty')
    first = entries[0]
    common = len(first)
    for wire in entries[1:]:
        common = min(common, len(wire))
        common = next((at for at in range(common) if first[at] != wire[at]), common)
    prefix = min(u16(original, 24), common)
    payload = entries[0] + b''.join(wire[prefix:] for wire in entries[1:])
    require(len(payload) <= PAGE - ENTRY_AT, 'No-allocation fitting wire')
    bitmap = bytearray(MASK_BYTES)
    end = 0
    for position, wire in enumerate(entries):
        end += len(wire) if position == 0 else len(wire) - prefix
        bitmap[end // 8] |= 1 << (end % 8)
    base = number * PAGE
    after[base + 2:base + 4] = struct.pack('<H', PAGE - ENTRY_AT - len(payload))
    after[base + 24:base + 26] = struct.pack('<H', prefix)
    after[base + MASK_AT:base + ENTRY_AT] = bitmap
    after[base + ENTRY_AT:base + ENTRY_AT + len(payload)] = payload
    require(after[base + ENTRY_AT + len(payload):base + PAGE]
            == original[ENTRY_AT + len(payload):], 'Unused/vacated tail remains original')
    return [(base + 2, base + 4), (base + 24, base + 26),
            (base + MASK_AT, base + ENTRY_AT + len(payload))]


def construct(before, profile):
    header = profile['header']
    expected_hash = {
        5: '0d9cf91fff6c9153c628bf5cd54dc39a9420b18f4fad669fca8e9a3c8a92694b',
        6: '18eff74333ed55f4bfef879d6a2530fab8031aaef6a07895f50aec13fdf079ae',
    }[header]
    require(digest(before) == expected_hash and before[20] == header, 'Exact original BEFORE')
    definition = profile['definition']
    rows = records(before, definition)
    chosen = [rows[number - 1] for number in SELECTED_IDS]
    removed = {row['reference'] for row in chosen}
    require([row['reference'] for row in chosen] == profile['selected_refs'], 'Frozen selected roots')
    after = bytearray(before)
    allowed = []
    count = definition * PAGE + 16
    require(u32(before, count) == 800, 'Original live count')
    after[count:count + 4] = struct.pack('<I', 798)
    allowed.append((count, count + 4))
    for row in chosen:
        at = row['directory']
        after[at:at + 2] = struct.pack('<H', row['word'] | 0xc000)
        allowed.append((at, at + 2))

    partitions = []
    for physical, column, children in [
        (0, 0, profile['primary_children']), (1, 2, profile['guard_children']),
    ]:
        root = profile['roots'][physical]
        bounds, root_page = decode(before, root, definition, True)
        require(len(bounds) == 1 and bounds[0][2] == children[0]
                and u32(root_page, 20) == children[1]
                and root_page[8:20] == bytes(12) and root_page[26] == 1,
                'Observed height-one separate-tail form and child order')
        original_population = []
        maxima = []
        counts = []
        for position, child in enumerate(children):
            entries, page = decode(before, child, definition, False)
            require(u32(page, 12) == (0 if position == 0 else children[0])
                    and u32(page, 16) == (children[1] if position == 0 else 0)
                    and u32(page, 20) == 0 and page[26] == 0, 'Original leaf links/height')
            original_population.extend((value, reference) for value, reference, _ in entries)
            retained = [(value, reference) for value, reference, _ in entries if reference not in removed]
            require(retained, 'No child retirement or contraction')
            maxima.append(retained[-1])
            counts.append(len(retained))
            allowed.extend(write_page(after, page, child,
                                      [key(value, reference) for value, reference in retained]))
        require(original_population == sorted((row['values'][column], row['reference']) for row in rows),
                'Complete original leaf populations, exact bounds including ROW tie suffix')
        require(bounds[0][:2] == tuple(profile['primary_fence_before' if physical == 0 else 'guard_fence_before']),
                'Frozen original full fence')
        expected_max = profile['primary_fence_after' if physical == 0 else 'guard_fence_after']
        require(maxima[0] == tuple(expected_max), 'Independent retained maximum')
        expected_counts = profile['primary_after_counts' if physical == 0 else 'guard_after_counts']
        require(counts == expected_counts, 'Original-partition survivor counts')
        allowed.extend(write_page(after, root_page, root, [key(*maxima[0], children[0])]))
        partitions.append({'physical': physical, 'root': root, 'children': children,
                           'survivors': counts, 'bound': list(maxima[0])})

    require(len(after) == len(before), 'No growth')
    require(before[definition * PAGE + 67:definition * PAGE + 83]
            == after[definition * PAGE + 67:definition * PAGE + 83], 'Historical counters remain exact')
    allowed.sort()
    at = 0
    for start, end in allowed:
        require(before[at:start] == after[at:start], 'Complete unaffected byte complement')
        at = max(at, end)
    require(before[at:] == after[at:], 'Final unaffected complement')
    return bytes(after), {'selected_ids': list(SELECTED_IDS), 'selected_refs': sorted(removed),
                          'partitions': partitions, 'allowed_spans': allowed,
                          'changed_pages': [page for page in range(len(before) // PAGE)
                                            if before[page * PAGE:(page + 1) * PAGE]
                                            != after[page * PAGE:(page + 1) * PAGE]],
                          'native_producer_calls': 0}
