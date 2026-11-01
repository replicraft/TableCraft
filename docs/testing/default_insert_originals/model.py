"""Independent literal default/NULL INSERT images from observed original bytes.

No TableCraft resolver, parser, serializer, mutation or SDK writer is imported.
"""
import hashlib
import struct

PAGE = 4096
OLD_RECORD = bytes.fromhex('060001000000ffffffffa861000000000000000000006f0072006900670069006e0061006c0026001600010017')
FIELDS = ('Id', 'Score', 'Caption', 'Flag', 'Money', 'Optional')
DEFAULTS = {'Score': ('i32', 7), 'Caption': ('text', 'seed'), 'Flag': ('bool', True),
            'Money': ('currency_scaled', -10001), 'Optional': ('null', None), 'Id': ('null', None)}
SUPPLIED = {'supplied_i32': 'i32', 'supplied_text': 'text', 'supplied_bool': 'bool',
            'supplied_currency_scaled': 'currency_scaled', 'supplied_null': 'null'}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def u16(raw, at):
    return struct.unpack_from('<H', raw, at)[0]


def u32(raw, at):
    return struct.unpack_from('<I', raw, at)[0]


def resolve(original):
    require(len(original) == 6, 'Complete original supplied/omitted width')
    values = []
    for name, presence in zip(FIELDS, original, strict=True):
        require(len(presence) == 1, 'Exactly one original presence/carrier tag')
        tag, value = next(iter(presence.items()))
        if tag == 'omitted':
            require(value is True, 'Explicit omission marker')
            values.append(DEFAULTS[name])
        else:
            require(tag in SUPPLIED, 'Closed original supplied carrier')
            values.append((SUPPLIED[tag], None if tag == 'supplied_null' else value))
    return values


def record(values):
    require(values[0][0] == 'i32', 'Actual required manual primary value')
    require(values[3][0] == 'bool' and type(values[3][1]) is bool, 'No stored NULL Boolean')
    fixed = bytearray(22)
    fixed[:2] = bytes([6, 0])
    mask = 8 if values[3][1] else 0
    for position, offset, tag, format in [(0, 2, 'i32', '<i'), (1, 6, 'i32', '<i'),
                                          (4, 10, 'currency_scaled', '<q'), (5, 18, 'i32', '<i')]:
        actual, value = values[position]
        if actual != 'null':
            require(actual == tag and type(value) is int, 'Exact declared fixed carrier')
            struct.pack_into(format, fixed, offset, value)
            mask |= 1 << position
    tag, value = values[2]
    require(tag in ('text', 'null'), 'Exact nullable Text carrier')
    payload = b'' if tag == 'null' else value.encode('utf-16le')
    require(len(payload) <= 80, 'Observed Text40 declaration')
    if tag == 'text':
        mask |= 4
    end = 22 + len(payload)
    return bytes(fixed) + payload + struct.pack('<HHH', end, 22, 1) + bytes([mask])


def entry(identifier, data, slot):
    return bytes([0x7f]) + ((identifier & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big') + ((data << 8) | slot).to_bytes(4, 'big')


def construct(before, profile, request):
    require(len(before) == profile['bytes'] and hashlib.sha256(before).hexdigest() == profile['sha256'],
            'Exact observed genuine SDK original')
    td, data, leaf = (profile[name] * PAGE for name in ('definition', 'data', 'root'))
    require(u32(before, td + 16) == 1 and u32(before, td + 20) == 0 and u32(before, td + 67) == 1,
            'Original manual table count/LAST/history')
    require(before[data:data + 2] == bytes([1, 1]) and u32(before, data + 4) == profile['definition'],
            'Original DATA ownership')
    require(u16(before, data + 2) == 4035 and u16(before, data + 12) == 1
            and u16(before, data + 14) == 4051 and before[data + 4051:data + PAGE] == OLD_RECORD,
            'Complete observed45B original row/frontier/directory')
    require(before[leaf:leaf + 2] == bytes([4, 1]) and u32(before, leaf + 4) == profile['definition']
            and before[leaf + 8:leaf + 27] == bytes(19) and u16(before, leaf + 2) == 3607,
            'Original singleton prefix0 leaf/header')
    original_bitmap = bytearray(453)
    original_bitmap[1] = 2
    require(before[leaf + 27:leaf + 480] == original_bitmap
            and before[leaf + 480:leaf + 489] == entry(1, profile['data'], 0), 'Complete original entry bitmap/wire')
    resolved = resolve(request['values'])
    if request.get('outcome') == 'refuse':
        require(resolved[0][0] == 'null' or resolved[3][0] == 'null', 'Literal source-backed unsupported presence')
        return None, {'resolved': resolved, 'reason': request['reason'], 'native_producer_calls': 0}
    raw = record(resolved)
    start = 4051 - len(raw)
    free = start - 18
    after = bytearray(before)
    after[data + start:data + 4051] = raw
    after[data + 16:data + 18] = struct.pack('<H', start)
    after[data + 12:data + 14] = struct.pack('<H', 2)
    after[data + 2:data + 4] = struct.pack('<H', free)
    after[td + 16:td + 20] = struct.pack('<I', 2)
    after[td + 67:td + 71] = struct.pack('<I', 2)
    bitmap = bytearray(original_bitmap)
    bitmap[2] = 4
    after[leaf + 2:leaf + 4] = struct.pack('<H', 3598)
    after[leaf + 27:leaf + 480] = bitmap
    after[leaf + 480:leaf + 498] = entry(1, profile['data'], 0) + entry(resolved[0][1], profile['data'], 1)
    spans = [(data + start, data + 4051), (data + 16, data + 18), (data + 12, data + 14),
             (data + 2, data + 4), (td + 16, td + 20), (td + 67, td + 71),
             (leaf + 2, leaf + 4), (leaf + 27, leaf + 480), (leaf + 480, leaf + 498)]
    require(len(after) == len(before) and all(before[at] == after[at] for at in range(len(before))
            if not any(start <= at < end for start, end in spans)),
            'Every unrelated property/counter/row/system/LVAL/map/tail/EOF byte exact')
    return bytes(after), {'resolved': resolved, 'record_hex': raw.hex(), 'record_bytes': len(raw),
                          'slot': 1, 'start': start, 'free': free, 'changed_spans': spans,
                          'native_producer_calls': 0}
