"""Literal retained-height NODE INSERT model; never invokes a native writer."""
import hashlib
import struct

PAGE = 4096
ENTRY_AT = 480
MASK_AT = 27
MASK_BYTES = 453


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
    new_values = (801, 400, 1)
    reference = profile['new_reference']
    data = profile['target_data']
    require(reference == ((data << 8) | 80), 'Frozen next direct ROW')
    record = bytes([3, 0]) + struct.pack('<iii', *new_values) + bytes([7])
    require(len(record) == 15, 'Literal exact new three-I32 record')
    page = before[data * PAGE:(data + 1) * PAGE]
    require(page[:2] == bytes([1, 1]) and u32(page, 4) == definition
            and u16(page, 12) == 80 and u16(page, 2) == 2722
            and u16(page, 14 + 2 * 79) == 2896, 'Observed existing DATA capacity')
    require(u32(before, definition * PAGE + 59) == profile['candidate_reference'],
            'Original actual candidate reference')
    hint_ref = profile['candidate_reference']
    hint_page = before[(hint_ref >> 8) * PAGE:((hint_ref >> 8) + 1) * PAGE]
    hint_slot = hint_ref & 255
    hint_start = u16(hint_page, 14 + 2 * hint_slot) & 0x1fff
    hint_end = PAGE if hint_slot == 0 else u16(hint_page, 12 + 2 * hint_slot) & 0x1fff
    hint = hint_page[hint_start:hint_end]
    require(len(hint) == 69 and hint[0] == 0 and u32(hint, 1) == 0,
            'Observed represented candidate window')
    members = [number for number in range(512) if hint[5 + number // 8] & (1 << (number % 8))]
    require(members == [data], 'Only target original DATA page is hinted')
    for number in profile['data_pages'][:-1]:
        earlier = before[number * PAGE:(number + 1) * PAGE]
        require(earlier[:2] == bytes([1, 1]) and u32(earlier, 4) == definition
                and u16(earlier, 12) == 240 and u16(earlier, 2) == 2,
                'Earlier owned DATA pages cannot fit this row')

    after = bytearray(before)
    allowed = []
    count = definition * PAGE + 16
    require(u32(before, count) == 800, 'Original live count')
    after[count:count + 4] = struct.pack('<I', 801)
    allowed.append((count, count + 4))
    base = data * PAGE
    after[base + 2881:base + 2896] = record
    after[base + 174:base + 176] = struct.pack('<H', 2881)
    after[base + 12:base + 14] = struct.pack('<H', 81)
    after[base + 2:base + 4] = struct.pack('<H', 2705)
    allowed.extend([(base + 2881, base + 2896), (base + 174, base + 176),
                    (base + 12, base + 14), (base + 2, base + 4)])

    partitions = []
    for physical, column, children in [
        (0, 0, profile['primary_children']), (1, 2, profile['guard_children']),
    ]:
        root = profile['roots'][physical]
        bounds, root_page = decode(before, root, definition, True)
        require(len(bounds) == 1 and bounds[0][2] == children[0]
                and u32(root_page, 20) == children[1]
                and root_page[8:20] == bytes(12) and root_page[26] == 1,
                'Observed separate-tail NODE and child order')
        originals = []
        leaf_pages = []
        for position, child in enumerate(children):
            entries, leaf = decode(before, child, definition, False)
            require(u32(leaf, 12) == (0 if position == 0 else children[0])
                    and u32(leaf, 16) == (children[1] if position == 0 else 0)
                    and u32(leaf, 20) == 0 and leaf[26] == 0, 'Original leaf topology')
            originals.extend((value, row) for value, row, _ in entries)
            require(len(entries) == profile['before_counts'][physical][position],
                    'Frozen original leaf count')
            leaf_pages.append(leaf)
        require(originals == sorted((row['values'][column], row['reference']) for row in rows),
                'Complete original native key/ROW population')
        final = sorted(originals + [(new_values[column], reference)])
        split = profile['before_counts'][physical][0]
        assigned = [final[:split], final[split:]]
        require([len(keys) for keys in assigned] == profile['after_counts'][physical],
                'Earlier leaf keeps original count, tail absorbs growth')
        require(bounds[0][:2] == tuple(profile['before_fences'][physical]), 'Original full fence')
        require(assigned[0][-1] == tuple(profile['after_fences'][physical]), 'Final full fence')
        for child, leaf, keys in zip(children, leaf_pages, assigned, strict=True):
            require(keys, 'Every original child remains nonempty')
            allowed.extend(write_page(after, leaf, child, [key(value, row) for value, row in keys]))
        allowed.extend(write_page(after, root_page, root, [key(*assigned[0][-1], children[0])]))
        history_at = definition * PAGE + 67 + 12 * physical
        expected_history = 800 if physical == 0 else 7
        require(u32(before, history_at) == expected_history, 'Original historical counter')
        delta = len({value for value, _ in final}) - len({value for value, _ in originals})
        require(delta == (1 if physical == 0 else 0), 'Distinct-family history delta')
        after[history_at:history_at + 4] = struct.pack('<I', expected_history + delta)
        allowed.append((history_at, history_at + 4))
        partitions.append({'physical': physical, 'root': root, 'children': children,
                           'counts': [len(keys) for keys in assigned], 'bound': list(assigned[0][-1]),
                           'distinct_added': delta})

    require(len(after) == len(before), 'No allocation or EOF growth')
    allowed.sort()
    at = 0
    for start, end in allowed:
        require(before[at:start] == after[at:start], 'Complete unaffected complement')
        at = max(at, end)
    require(before[at:] == after[at:], 'Final unaffected complement')
    return bytes(after), {'new_values': list(new_values), 'new_reference': reference,
                          'partitions': partitions, 'allowed_spans': allowed,
                          'changed_pages': [page for page in range(len(before) // PAGE)
                                            if before[page * PAGE:(page + 1) * PAGE]
                                            != after[page * PAGE:(page + 1) * PAGE]],
                          'native_producer_calls': 0}
