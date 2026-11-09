"""Independent finite record oracle; imports no writer or staged candidate data."""
from dataclasses import dataclass
import hashlib
import json


@dataclass(frozen=True)
class Shape:
    maximum_columns: int
    fixed_end: int
    variable_slots: int
    selected_number: int
    selected_variable: int


def word(data, offset):
    assert 0 <= offset <= len(data) - 2
    return int.from_bytes(data[offset:offset + 2], 'little')


def frame(data, shape):
    count = word(data, 0)
    assert count == shape.maximum_columns
    assert 0 <= shape.selected_number < count
    mask_start = len(data) - (count + 7) // 8
    count_at = mask_start - 2
    variables = word(data, count_at)
    assert 1 <= variables <= shape.variable_slots
    assert 0 <= shape.selected_variable < variables
    data_end = count_at - 2 * (variables + 1)
    offsets = [word(data, count_at - 2 * (position + 1)) for position in range(variables + 1)]
    assert shape.fixed_end <= offsets[0]
    assert all(left <= right for left, right in zip(offsets, offsets[1:]))
    assert offsets[-1] <= data_end
    return count, mask_start, count_at, data_end, offsets


def decode_text(wire):
    if not wire.startswith(b'\xff\xfe'):
        return wire.decode('utf-16le', errors='strict')
    units = []
    narrow = True
    position = 2
    while position < len(wire):
        if wire[position] == 0:
            narrow = not narrow
            position += 1
        elif narrow:
            units.append(wire[position])
            position += 1
        else:
            assert position + 2 <= len(wire)
            units.append(word(wire, position))
            position += 2
    return b''.join(unit.to_bytes(2, 'little') for unit in units).decode('utf-16le', errors='strict')


def preferred(kind, value, compressed=False):
    if value is None:
        return b''
    if kind == 'binary':
        assert isinstance(value, bytes)
        return value
    assert kind == 'text' and isinstance(value, str)
    raw = value.encode('utf-16le', errors='strict')
    units = [word(raw, position) for position in range(0, len(raw), 2)]
    if compressed and len(units) > 2 and all(0 < unit <= 255 for unit in units):
        wire = b'\xff\xfe' + bytes(units)
    else:
        wire = raw
    assert decode_text(wire) == value, 'Unfaithful leading-marker representation'
    return wire


def rebuild(before, shape, payload, present):
    count, mask_start, _, data_end, offsets = frame(before, shape)
    variable = shape.selected_variable
    start, end = offsets[variable:variable + 2]
    new_end = offsets[-1] - (end - start) + len(payload)
    footer = len(before) - data_end
    length = max(len(before), new_end + footer)
    result = bytearray(before + bytes(length - len(before)))
    result[start:start + len(payload)] = payload
    result[start + len(payload):new_end] = before[end:offsets[-1]]
    new_mask = length - (count + 7) // 8
    result[new_mask:] = before[mask_start:]
    bit = 1 << (shape.selected_number % 8)
    at = new_mask + shape.selected_number // 8
    result[at] = (result[at] & ~bit) | (bit if present else 0)
    count_at = new_mask - 2
    variables = len(offsets) - 1
    result[count_at:count_at + 2] = variables.to_bytes(2, 'little')
    for position, offset in enumerate(offsets):
        if position > variable:
            offset = offset - (end - start) + len(payload)
        at = count_at - 2 * (position + 1)
        result[at:at + 2] = offset.to_bytes(2, 'little')
    assert len(result) == length
    frame(result, shape)
    return bytes(result)


def source(count=9, variables=3, padding=8):
    # Selected variable1 has 4 bytes; variable2 is an opaque historical slot.
    prefix = count.to_bytes(2, 'little') + bytes.fromhex('78563412')
    payload = b'abc' + b'A\x00B\x00' + b'\xd3\xe4'
    offsets = [6, 9, 13, 15]
    assert variables == 3
    footer = b''.join(offset.to_bytes(2, 'little') for offset in reversed(offsets))
    return prefix + payload + bytes([0xa7]) * padding + footer + variables.to_bytes(2, 'little') + bytes([0xff, 1])


def selfcheck():
    shape = Shape(9, 6, 5, 7, 1)
    old = source()
    checks = []
    for label, kind, value, compression, expected_length in [
        ('growth_consumes_padding', 'binary', bytes(10), False, len(old)),
        ('growth_exceeds_padding', 'binary', bytes(14), False, len(old) + 2),
        ('shrink_preserves_capacity', 'binary', b'x', False, len(old)),
        ('null_preserves_capacity', 'text', None, True, len(old)),
        ('empty_preserves_capacity', 'text', '', True, len(old)),
        ('compressed_three', 'text', 'ABC', True, len(old)),
        ('raw_two_tie', 'text', 'AB', True, len(old)),
        ('nul_forces_raw', 'text', 'A\0B', True, len(old)),
        ('wide_forces_raw', 'text', 'ĀāĂ', True, len(old)),
    ]:
        payload = preferred(kind, value, compression)
        new = rebuild(old, shape, payload, value is not None)
        assert len(new) == expected_length
        _, mask, _, _, offsets = frame(new, shape)
        assert new[offsets[2]:offsets[3]] == b'\xd3\xe4'
        assert bool(new[mask] & 0x80) == (value is not None)
        checks.append({'name': label, 'length': len(new), 'payload': payload.hex(), 'sha256': hashlib.sha256(new).hexdigest()})
    assert preferred('text', '', True) == b''
    assert preferred('text', 'AB', True) == b'A\x00B\x00'
    assert preferred('text', 'ABC', True) == b'\xff\xfeABC'
    # A semantic no-op is handled before serialization: marker-empty/raw-empty
    # and noncanonical source wires remain byte-exact rather than rebuilt.
    assert decode_text(b'\xff\xfe') == ''
    for value in ['\ufeff', '\ufeffA']:
        try:
            preferred('text', value, False)
        except (AssertionError, UnicodeError):
            pass
        else:
            raise AssertionError('Marker alias should not qualify')
    try:
        frame(old, Shape(10, 6, 5, 7, 1))
    except AssertionError:
        pass
    else:
        raise AssertionError('Historical smaller physical column count must refuse')
    return {'scope': 'pure synthetic record oracle only', 'fixture_calls': 0, 'cases': checks, 'historical_variable_slots_preserved': True, 'column_count_mismatch_refused': True, 'marker_empty_decode': True}


if __name__ == '__main__':
    print(json.dumps(selfcheck(), indent=2))
