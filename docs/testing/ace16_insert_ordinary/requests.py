"""Closed prospective complete requests, independent of Java row templates."""
import csv
import io
import struct


def values(mixed):
    # 2000-01-01 noon is exactly 36526.5 OLE days after 1899-12-30.
    date_bits = int.from_bytes(struct.pack('<d', 36526.5), 'little')
    common = [
        ('null', None), ('boolean', True), ('int32', 226), ('byte', 255),
        ('int16', -32768), ('int64', 9223372036854775807),
        ('currency-scaled', 1234567), ('single-bits', 0x80000000),
        ('double-bits', 0x8000000000000000), ('date-bits', date_bits),
        ('guid', 'fedcba98-7654-3210-ffff-001122334455'),
    ]
    variable = [('text', 'z' * 255), ('binary', bytes([255]) * 255),
                ('text', 'Ā' * 255), ('binary', b'')]
    trailing = [('int32', -2147483648), ('boolean', False)]
    return common + (variable if mixed else []) + trailing


def tsv(mixed):
    fields = []
    for kind, value in values(mixed):
        if kind == 'null':
            data = ''
        elif kind == 'boolean':
            data = 'true' if value else 'false'
        elif kind == 'binary':
            data = value.hex()
        elif kind.endswith('-bits'):
            data = format(value, 'x')
        else:
            data = str(value)
        fields.extend((kind, data))
    stream = io.StringIO(newline='')
    csv.writer(stream, delimiter='\t', lineterminator='\n').writerow(fields)
    return stream.getvalue().encode('utf-8')
