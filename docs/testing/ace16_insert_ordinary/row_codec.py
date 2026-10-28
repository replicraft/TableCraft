"""Prospective request-derived ordinary row bytes; no engine or fixture calls.

This owns record geometry only. Source/catalog/property/index admission belongs
to the source gate; a constructed Layout is not a mutation authorization.
"""
from dataclasses import dataclass
import uuid


WIDTHS = {1: 0, 2: 1, 3: 2, 4: 4, 5: 8, 6: 4, 7: 8, 8: 8, 15: 16, 19: 8}
KINDS = {1: 'boolean', 2: 'byte', 3: 'int16', 4: 'int32', 5: 'currency-scaled',
         6: 'single-bits', 7: 'double-bits', 8: 'date-bits', 15: 'guid', 19: 'int64',
         9: 'binary', 10: 'text'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def number(raw, at, width=2):
    part = raw[at:at + width]
    require(len(part) == width, 'Truncated metadata integer')
    return int.from_bytes(part, 'little')


@dataclass(frozen=True)
class Field:
    name: str
    number: int
    native: int
    length: int
    offset: int
    variable: int
    compressed: bool


@dataclass(frozen=True)
class Layout:
    fields: tuple
    fixed_end: int
    variables: tuple

    @classmethod
    def bind(cls, raw, table):
        base = table['definition'] * 4096
        count = number(raw, base + 45)
        require(count == number(raw, base + 41) == len(table['columns']),
                'Dense actual/historical field count')
        variable_count = number(raw, base + 43)
        physical_count = number(raw, base + 51, 4)
        column_base = base + 63 + 12 * physical_count
        fields = []
        spans = []
        fixed_end = 2
        native_fixed_end = 2
        variables = {}
        for position, (name, public) in enumerate(table['columns'].items()):
            at = column_base + position * 25
            native = raw[at]
            ordinal = number(raw, at + 5)
            width = number(raw, at + 23)
            offset = number(raw, at + 21)
            variable = number(raw, at + 7)
            flags, extended = raw[at + 15:at + 17]
            require(ordinal == position == public['number'] == public['position'],
                    'Actual field identity/order')
            require(native == public['native_type'] and width == public['declared_bytes'],
                    'Raw/getter native type and width')
            require(native in KINDS, 'Unsupported native field')
            compressed = bool(extended & 1)
            require(compressed == public['compressed'], 'Raw/getter compression flag')
            if native in WIDTHS:
                require(flags == 3 and extended == 0
                        and (width in (0, 1) if native == 1 else width == WIDTHS[native]),
                        'Exact ordinary fixed field')
                # The native TDF/getter may declare BOOLEAN length1. Like the
                # owning native layout reader, its physical payload width is0;
                # only the field-number mask bit stores its value.
                physical_width = WIDTHS[native]
                native_fixed_end = max(native_fixed_end, 2 + offset + physical_width)
                if physical_width:
                    start, end = 2 + offset, 2 + offset + physical_width
                    require(all(end <= a or b <= start for a, b in spans),
                            'Overlapping actual fixed spans')
                    spans.append((start, end))
                    fixed_end = max(fixed_end, end)
            else:
                require(flags == 2 and extended in ((0, 1) if native == 10 else (0,)),
                        'Exact ordinary variable field')
                require(0 < width <= 510 and (native != 10 or width % 2 == 0),
                        'Variable declared width')
                require(variable == public['variable'] and variable not in variables,
                        'Unique actual variable ordinal')
                variables[variable] = position
            fields.append(Field(name, ordinal, native, WIDTHS.get(native, width),
                                offset, variable, compressed))
        require(native_fixed_end == fixed_end, 'Boolean offset invents fixed extent')
        require(set(variables) == set(range(variable_count)), 'Complete variable ordinals')
        cursor = column_base + count * 25
        for field in fields:
            length = number(raw, cursor)
            cursor += 2
            native_name = raw[cursor:cursor + length]
            require(len(native_name) == length and length % 2 == 0,
                    'Complete native field name')
            require(native_name.decode('utf-16le') == field.name, 'Raw/getter native field name')
            cursor += length
        return cls(tuple(fields), fixed_end,
                   tuple(variables[index] for index in range(variable_count)))


def preferred_text(value, compressed):
    raw = value.encode('utf-16le')
    units = [int.from_bytes(raw[index:index + 2], 'little')
             for index in range(0, len(raw), 2)]
    require(not units or units[0] != 0xfeff, 'Leading FEFF marker ambiguity')
    if compressed and len(units) > 2 and all(0 < unit <= 255 for unit in units):
        return b'\xff\xfe' + bytes(units)
    return raw


def payload(field, operand):
    kind, value = operand
    if kind == 'null':
        require(value is None and field.native != 1, 'NULL carrier/Boolean')
        return bytes(WIDTHS.get(field.native, 0)), False
    require(kind == KINDS[field.native], 'Exact request carrier')
    if field.native == 1:
        require(type(value) is bool, 'Boolean request carrier')
        return b'', value
    if field.native == 10:
        require(type(value) is str and len(value.encode('utf-16le')) <= field.length,
                'Text declared UTF16 width')
        return preferred_text(value, field.compressed), True
    if field.native == 9:
        require(type(value) is bytes and len(value) <= field.length, 'Binary declared width')
        return value, True
    if field.native == 15:
        require(type(value) is str, 'GUID canonical carrier')
        canonical = uuid.UUID(value).bytes
        return canonical[:4][::-1] + canonical[4:6][::-1] + canonical[6:8][::-1] + canonical[8:], True
    width = WIDTHS[field.native]
    require(type(value) is int, 'Exact integer/bits carrier')
    signed = field.native in (3, 4, 5, 19)
    return value.to_bytes(width, 'little', signed=signed), True


def encode(layout, operands):
    require(len(operands) == len(layout.fields), 'Complete ordered request arity')
    body = bytearray(layout.fixed_end)
    body[:2] = len(layout.fields).to_bytes(2, 'little')
    mask = bytearray((len(layout.fields) + 7) // 8)
    variable_payload = {}
    for field, operand in zip(layout.fields, operands, strict=True):
        data, present = payload(field, operand)
        if present:
            mask[field.number // 8] |= 1 << (field.number % 8)
        if field.native in WIDTHS:
            if data:
                start = 2 + field.offset
                body[start:start + len(data)] = data
        else:
            variable_payload[field.number] = data
    if layout.variables:
        offsets = []
        for position in layout.variables:
            offsets.append(len(body))
            body.extend(variable_payload[position])
        offsets.append(len(body))
        for offset in reversed(offsets):
            body.extend(offset.to_bytes(2, 'little'))
        body.extend(len(layout.variables).to_bytes(2, 'little'))
    body.extend(mask)
    require(len(body) <= 4080, 'Normal DATA record bound')
    return bytes(body)
