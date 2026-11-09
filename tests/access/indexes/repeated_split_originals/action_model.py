"""Observe exact typed rows, keys, topology and counters; no process side effects."""
from shared_split.actions import analyze as shared_analyze
from shared_split.actions import complete_members as shared_complete_members
from shared_split.actions import inspect_indexes as shared_inspect_indexes
from decimal import Decimal
import hashlib
from pathlib import Path

from repeated_split_originals import model as source
from delete_reference_maps import decode_map

PAGE = 4096
TABLE_HEADER = 63
COUNTER_BYTES = 12
COLUMN_BYTES = 25
PHYSICAL_BYTES = 52
COLUMN_NAME_BYTES = 2
PHYSICAL_COUNT_OFFSET = 51
USAGE_OFFSET = 34
ROOT_OFFSET = 38
FLAGS_OFFSET = 46
FIXED_DATA_OFFSET = 21
RECORD_HEADER_BYTES = 2
VALUE_INDEX = 1
MAP_INLINE = 0
MAP_INDIRECT = 1
MAP_BASE = 1
MAP_BITMAP = 5
BITMAP_DATA = 4
POINTER_BYTES = 4
BITS_PER_BYTE = 8


def digest(data):
    return hashlib.sha256(data).hexdigest()


def units(value, native_type):
    carrier, text = value.split(':', 1)
    expected = {2: 'Byte', 3: 'Short', 4: 'Integer', 19: 'Long', 5: 'BigDecimal'}
    assert carrier == expected[native_type]
    if native_type == 5:
        scaled = Decimal(text) * 10000
        assert scaled == scaled.to_integral_value()
        result = int(scaled)
        assert -(1 << 63) <= result < 1 << 63
        return result
    result = int(text)
    if native_type == 2:
        assert -128 <= result <= 127
        return result if result >= 0 else result + 256
    return result


def physical_start(raw, table):
    definition = raw[table['definition'] * PAGE:(table['definition'] + 1) * PAGE]
    count = source.number(definition, PHYSICAL_COUNT_OFFSET)
    position = TABLE_HEADER + count * COUNTER_BYTES + table['columns_count'] * COLUMN_BYTES
    for name in table['columns']:
        length = source.number(definition, position, COLUMN_NAME_BYTES)
        position += COLUMN_NAME_BYTES
        assert definition[position:position + length].decode('utf-16le') == name
        position += length
    return definition, position


def key_offset(raw, table):
    definition = raw[table['definition'] * PAGE:(table['definition'] + 1) * PAGE]
    count = source.number(definition, PHYSICAL_COUNT_OFFSET)
    field = TABLE_HEADER + count * COUNTER_BYTES + table['columns']['KeyValue']['position'] * COLUMN_BYTES
    return RECORD_HEADER_BYTES + source.number(definition, field + FIXED_DATA_OFFSET, 2)


def inspect_indexes(raw, tables):
    """Bind this feature's source policy to shared independent mechanics."""
    return shared_inspect_indexes(
        raw,
        tables,
        COUNTER_BYTES=COUNTER_BYTES,
        FLAGS_OFFSET=FLAGS_OFFSET,
        PAGE=PAGE,
        PHYSICAL_BYTES=PHYSICAL_BYTES,
        ROOT_OFFSET=ROOT_OFFSET,
        TABLE_HEADER=TABLE_HEADER,
        USAGE_OFFSET=USAGE_OFFSET,
        decode_map=decode_map,
        physical_start=physical_start,
        source=source,
        units=units,
    )


def normalized_tables(tables):
    return {name: {**table, 'rows': {str(key): value for key, value in table['rows'].items()},
                   'raw_rows': {str(key): value for key, value in table['raw_rows'].items()}}
            for name, table in tables.items()}



def complete_members(raw, metadata):
    """Bind this feature's source policy to shared independent mechanics."""
    return shared_complete_members(
        raw,
        metadata,
        BITMAP_DATA=BITMAP_DATA,
        BITS_PER_BYTE=BITS_PER_BYTE,
        MAP_BASE=MAP_BASE,
        MAP_BITMAP=MAP_BITMAP,
        MAP_INDIRECT=MAP_INDIRECT,
        MAP_INLINE=MAP_INLINE,
        PAGE=PAGE,
        POINTER_BYTES=POINTER_BYTES,
        source=source,
    )

def analyze(before_path, after_path, original, target, description):
    """Bind this feature's source policy to shared independent mechanics."""
    return shared_analyze(
        before_path,
        after_path,
        original,
        target,
        description,
        PAGE=PAGE,
        Path=Path,
        VALUE_INDEX=VALUE_INDEX,
        complete_members=complete_members,
        decode_map=decode_map,
        digest=digest,
        inspect_indexes=inspect_indexes,
        key_offset=key_offset,
        normalized_tables=normalized_tables,
        source=source,
        units=units,
    )
