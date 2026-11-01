"""Literal cross-page INSERT AFTER from observed originals and original requests.

No TableCraft or SDK writer, decoder, reservation or old AFTER is imported.
"""
import hashlib
import struct

PAGE = 4096
KEY_AT = 480
MASK_AT = 27
MASK_BYTES = 453
OLD_ROWS = tuple((number, number - 5, (number - 1) % 3, 'o' * 230) for number in range(1, 10))
REQUESTS = ((10, 17, 7, 'L' * 200), (11, 18, 7, 'new'))


def require(ok, message):
    if not ok:
        raise ValueError(message)


def u16(raw, offset):
    return struct.unpack_from('<H', raw, offset)[0]


def u32(raw, offset):
    return struct.unpack_from('<I', raw, offset)[0]


def row(values):
    identifier, score, guard, label = values
    require(all(type(value) is int and -(1 << 31) <= value < (1 << 31)
                for value in (identifier, score, guard)), 'Exact original I32 carriers')
    payload = label.encode('utf-16le')
    require(len(payload) <= 510 and not label.startswith('\ufeff'), 'Frozen uncompressed Text value')
    end = 14 + len(payload)
    return struct.pack('<Hiii', 4, identifier, score, guard) + payload + struct.pack('<HHH', end, 14, 1) + bytes([15])


def entry(value, page, slot):
    require(0 < page <= 0xffffff and 0 <= slot < 255, 'Qualified ROW reference')
    return bytes([0x7f]) + ((value & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big') + ((page << 8) | slot).to_bytes(4, 'big')


def keys(rows, references, column):
    return b''.join(entry(value, reference >> 8, reference & 255)
                    for value, reference in sorted((values[column], reference)
                                                  for values, reference in zip(rows, references, strict=True)))


def bitmap(count):
    result = bytearray(MASK_BYTES)
    for number in range(1, count + 1):
        end = number * 9
        result[end // 8] |= 1 << (end % 8)
    return result


def construct(before, profile, requests):
    require(hashlib.sha256(before).hexdigest() == profile['sha256'] and len(before) == profile['bytes'],
            'Exact observed SDK original')
    require(tuple(tuple(values) for values in requests) == REQUESTS, 'Exact frozen request order and carriers')
    td, early, late = profile['definition'], profile['early'], profile['late']
    require(u32(before, td * PAGE + 16) == 9 and u32(before, td * PAGE + 20) == 0,
            'Observed manual table count/LAST')
    references = [(early << 8) | slot for slot in range(8)] + [(late << 8)]
    for number, count, free in ((early, 8, 218), (late, 1, 3599)):
        base = number * PAGE
        require(before[base:base + 2] == bytes([1, 1]) and u32(before, base + 4) == td,
                'Original DATA owner/signature')
        require(u16(before, base + 12) == count and u16(before, base + 2) == free,
                'Observed original DATA geometry')
    for values, reference in zip(OLD_ROWS, references, strict=True):
        number, slot = reference >> 8, reference & 255
        base = number * PAGE
        start = u16(before, base + 14 + 2 * slot)
        end = PAGE if slot == 0 else u16(before, base + 12 + 2 * slot)
        require(before[base + start:base + end] == row(values), 'Every original exact complete481B row')

    after = bytearray(before)
    allowed = []
    placements = ((late, 1, 3194, 3615, 2, 3176), (early, 8, 221, 248, 9, 189))
    new_references = []
    for values, (number, slot, start, end, count, free) in zip(REQUESTS, placements, strict=True):
        raw = row(values)
        require(len(raw) == end - start, 'Independent complete requested record width')
        base = number * PAGE
        after[base + start:base + end] = raw
        directory = base + 14 + 2 * slot
        after[directory:directory + 2] = struct.pack('<H', start)
        after[base + 12:base + 14] = struct.pack('<H', count)
        after[base + 2:base + 4] = struct.pack('<H', free)
        require(start - (14 + 2 * count) == free, 'Exact final frontier/directory/free arithmetic')
        new_references.append((number << 8) | slot)
        allowed.extend([(base + start, base + end), (directory, directory + 2),
                        (base + 12, base + 14), (base + 2, base + 4)])
    after[td * PAGE + 16:td * PAGE + 20] = struct.pack('<I', 11)
    allowed.append((td * PAGE + 16, td * PAGE + 20))

    for physical, column, history in ((0, 0, (9, 11)), (1, 2, (3, 4))):
        base = profile['roots'][physical] * PAGE
        old_wire = keys(OLD_ROWS, references, column)
        require(before[base:base + 2] == bytes([4, 1]) and u32(before, base + 4) == td,
                'Observed original leaf identity')
        require(before[base + 8:base + 27] == bytes(19), 'Original prefix0 singleton leaf header')
        require(u16(before, base + 2) == 3535 and before[base + MASK_AT:base + KEY_AT] == bitmap(9),
                'Complete original leaf extent bitmap/free')
        require(before[base + KEY_AT:base + KEY_AT + 81] == old_wire, 'Original native keys/ROWs from literal rows')
        at = td * PAGE + 67 + physical * 12
        require(u32(before, at) == history[0], 'Observed physical history')
        after[at:at + 4] = struct.pack('<I', history[1])
        new_wire = keys(OLD_ROWS + REQUESTS, references + new_references, column)
        require(len(new_wire) == 99, 'All eleven native entries/ROW suffixes')
        after[base + 2:base + 4] = struct.pack('<H', 3517)
        after[base + MASK_AT:base + KEY_AT] = bitmap(11)
        after[base + KEY_AT:base + KEY_AT + 99] = new_wire
        allowed.extend([(at, at + 4), (base + 2, base + 4), (base + MASK_AT, base + KEY_AT),
                        (base + KEY_AT, base + KEY_AT + 99)])

    require(len(after) == len(before), 'No allocation or EOF change')
    require(all(before[index] == after[index] for index in range(len(before))
                if not any(start <= index < end for start, end in allowed)),
            'Whole complement: all original rows/properties/maps/LVAL/system/aliases/tails')
    return bytes(after), {'request_references': new_references,
                          'sequential_ids': list(range(1, 9)) + [11, 9, 10],
                          'guard_cursor_ids': [1, 4, 7, 2, 5, 8, 3, 6, 9, 11, 10],
                          'changed_spans': allowed, 'native_producer_calls': 0}
