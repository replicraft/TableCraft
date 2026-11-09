"""Original literal native framing, extracted from the prior Number oracle.

No production codecs or SDK encoder is imported. Explicit bytes are independent.
"""
import struct

PAGE = 4096


def short(data, offset):
    return struct.unpack_from('<H', data, offset)[0]


def long(data, offset):
    return struct.unpack_from('<I', data, offset)[0]


def put_short(data, offset, value):
    struct.pack_into('<H', data, offset, value)


def put_long(data, offset, value):
    struct.pack_into('<I', data, offset, value)


def utf16(text):
    return text.encode('utf-16le')


def append_definition(before, name):
    definition = before[7 * PAGE:8 * PAGE]
    count = short(definition, 41)
    variables = short(definition, 43)
    length = long(definition, 8)
    names = 75 + count * 25
    tail = names
    for _ in range(count):
        tail += 2 + short(definition, tail)
    fixed = max((short(definition, offset + 21) + short(definition, offset + 23)
                 for offset in range(75, names, 25)
                 if definition[offset + 15] & 1 and definition[offset] != 1), default=0)
    field = bytearray(25)
    field[0] = 4
    put_long(field, 1, 1625)
    put_short(field, 5, count)
    put_short(field, 7, variables)
    put_short(field, 9, count)
    field[15] = 3
    put_short(field, 21, fixed)
    put_short(field, 23, 4)
    name = utf16(name)
    result = bytearray(definition[:names] + field + definition[names:tail]
                       + struct.pack('<H', len(name)) + name + definition[tail:length])
    put_short(result, 2, PAGE - len(result) - 8)
    put_long(result, 8, len(result))
    put_short(result, 41, count + 1)
    put_short(result, 45, count + 1)
    result.extend(bytes(PAGE - len(result)))
    return result


def number_group(name, required, dictionary):
    name = utf16(name)
    values = bytearray()
    for index, key in enumerate(dictionary):
        if key == 'Required' and required:
            values.extend(struct.pack('<HBBHHB', 9, 1, 1, index, 1, 0))
        elif key == 'DefaultValue':
            values.extend(struct.pack('<HBBHH', 10, 1, 12, index, 2) + utf16('0'))
    return (struct.pack('<IHIH', 12 + len(name) + len(values), 1,
                        6 + len(name), len(name)) + name + values)

