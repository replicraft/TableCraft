"""Raw ordinary record interpretation independent of the new-row encoder.

Stored NULL fixed payloads and padding are opaque. This reads masks/footer and
actual spans; it never reconstructs a canonical row to admit an original row.
"""
from datetime import datetime
from decimal import Decimal
from fractions import Fraction
import struct
import uuid


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unsigned(raw, at, width=2):
    part = raw[at:at + width]
    require(len(part) == width, 'Truncated record integer')
    return int.from_bytes(part, 'little')


def text(raw):
    if not raw.startswith(b'\xff\xfe'):
        require(len(raw) % 2 == 0, 'Odd raw UTF16 payload')
        return raw.decode('utf-16le')
    units = []
    narrow = True
    position = 2
    while position < len(raw):
        first = raw[position]
        if first == 0:
            narrow = not narrow
            position += 1
        elif narrow:
            units.append(first)
            position += 1
        else:
            require(position + 1 < len(raw), 'Truncated switched UTF16 unit')
            units.append(first | raw[position + 1] << 8)
            position += 2
    joined = b''.join(unit.to_bytes(2, 'little') for unit in units)
    return joined.decode('utf-16le')


def decode(layout, raw):
    count = len(layout.fields)
    require(len(raw) >= layout.fixed_end + (count + 7) // 8, 'Record minimum size')
    require(unsigned(raw, 0) == count, 'Stored physical column count')
    mask_at = len(raw) - (count + 7) // 8
    variable_count = len(layout.variables)
    spans = {}
    if variable_count:
        require(unsigned(raw, mask_at - 2) == variable_count, 'Stored variable count')
        footer = mask_at - 2 - (variable_count + 1) * 2
        require(footer >= layout.fixed_end, 'Variable footer overlaps fixed spans')
        offsets = [unsigned(raw, mask_at - 4 - 2 * index)
                   for index in range(variable_count + 1)]
        require(layout.fixed_end <= offsets[0] <= offsets[-1] <= footer,
                'Actual variable start/end framing with opaque capacity padding')
        require(all(left <= right for left, right in zip(offsets, offsets[1:])),
                'Monotone variable offsets')
        for ordinal, field_position in enumerate(layout.variables):
            spans[field_position] = (offsets[ordinal], offsets[ordinal + 1])
    else:
        require(mask_at >= layout.fixed_end, 'Fixed-only payload extent')
    values = []
    for field in layout.fields:
        bit = bool(raw[mask_at + field.number // 8] & (1 << (field.number % 8)))
        native = field.native
        if native == 1:
            values.append(('boolean', bit))
            continue
        if native in (9, 10):
            start, end = spans[field.number]
            require(layout.fixed_end <= start <= end <= footer, 'Variable span extent')
            data = raw[start:end]
            if not bit:
                # Existing reader interprets the absent mask independently of
                # stored bytes. Do not require canonical empty source payload.
                values.append(('null', None))
            elif native == 9:
                require(len(data) <= field.length, 'Stored Binary declared width')
                values.append(('binary', bytes(data)))
            else:
                logical = text(data)
                require(len(logical.encode('utf-16le')) <= field.length,
                        'Stored Text declared UTF16 width')
                values.append(('text', logical))
            continue
        if not bit:
            values.append(('null', None))
            continue
        start = 2 + field.offset
        data = raw[start:start + field.length]
        require(len(data) == field.length and start + len(data) <= layout.fixed_end,
                'Stored fixed span extent')
        if native in (3, 4, 5, 19):
            kind = {3: 'int16', 4: 'int32', 5: 'currency-scaled', 19: 'int64'}[native]
            values.append((kind, int.from_bytes(data, 'little', signed=True)))
        elif native in (2, 6, 7, 8):
            kind = {2: 'byte', 6: 'single-bits', 7: 'double-bits', 8: 'date-bits'}[native]
            values.append((kind, int.from_bytes(data, 'little')))
        elif native == 15:
            canonical = data[:4][::-1] + data[4:6][::-1] + data[6:8][::-1] + data[8:]
            values.append(('guid', str(uuid.UUID(bytes=canonical))))
        else:
            raise ValueError('Unsupported fixed native type')
    return values


def same_values(actual, expected):
    require(len(actual) == len(expected), 'Decoded/request arity')
    for found, wanted in zip(actual, expected, strict=True):
        require(found[0] == wanted[0], 'Decoded/request carrier')
        if wanted[0] == 'guid':
            require(uuid.UUID(found[1]) == uuid.UUID(wanted[1]), 'Decoded/request GUID')
        else:
            require(found[1] == wanted[1], 'Decoded/request value or exact bits')


def check_getter(layout, decoded, stored):
    """Strict projection for the closed source recipe's actual Java carriers."""
    require(len(decoded) == len(stored) == len(layout.fields), 'Getter field inventory')
    for field, (kind, value), rendered in zip(layout.fields, decoded, stored, strict=True):
        if kind == 'null':
            require(rendered == 'NULL', 'Getter NULL distinct from empty')
            continue
        tag, separator, data = rendered.partition(':')
        require(separator == ':', 'Getter typed field')
        if kind == 'boolean':
            require(tag == 'Boolean' and data == ('true' if value else 'false'), 'Getter Boolean')
        elif kind == 'byte':
            require(tag == 'Byte' and int(data) == (value if value < 128 else value - 256),
                    'Raw unsigned Byte versus signed Java Byte')
        elif kind in ('int16', 'int32', 'int64'):
            require(tag == {'int16': 'Short', 'int32': 'Integer', 'int64': 'Long'}[kind]
                    and int(data) == value, 'Getter exact integral Java carrier')
        elif kind == 'currency-scaled':
            require(tag == 'BigDecimal' and Fraction(Decimal(data)) * 10000 == value,
                    'Getter exact scaled Currency')
        elif kind in ('single-bits', 'double-bits'):
            require(tag == ('Float' if kind == 'single-bits' else 'Double'), 'Getter float Java carrier')
            # This closed recipe uses finite exact -0/1.5 values. NaN payload
            # bits cannot be inferred from a Java text projection.
            require(data not in ('NaN', 'Infinity', '-Infinity'), 'No float text payload-bit inference')
            packed = struct.pack('<f' if kind == 'single-bits' else '<d', float(data))
            require(int.from_bytes(packed, 'little') == value, 'Getter exact finite IEEE value')
        elif kind == 'date-bits':
            require(tag == 'LocalDateTime', 'Explicit LOCAL_DATE_TIME source policy')
            actual = datetime.fromisoformat(data)
            require(actual.tzinfo is None, 'LocalDateTime has no hidden offset')
            delta = actual - datetime(1899, 12, 30)
            require(delta.days >= 0, 'Closed source date projection is nonnegative')
            days = Fraction(delta.days) + Fraction(delta.seconds * 1000000 + delta.microseconds,
                                                   86400 * 1000000)
            require(int.from_bytes(struct.pack('<d', float(days)), 'little') == value,
                    'Getter exact legacy DATE bits for source recipe')
        elif kind == 'binary':
            require(tag == 'Binary' and bytes.fromhex(data) == value, 'Getter Binary')
        elif kind == 'text':
            require(tag == 'String16' and bytes.fromhex(data).decode('utf-16le') == value,
                    'Getter exact Text UTF16')
        elif kind == 'guid':
            require(tag == 'String16', 'Getter GUID string carrier')
            logical = bytes.fromhex(data).decode('utf-16le')
            require(uuid.UUID(logical) == uuid.UUID(value), 'Getter native GUID projection')
        else:
            raise ValueError('Unqualified getter projection')
