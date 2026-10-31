"""Twelve original-source request choices; no image construction or processes."""

SQL_EMPTY = (
    'UPDATE [PreservedRows] SET [Flag]=[Flag], '
    '[Large_Number]=[Large_Number], [Date_Field]=[Date_Field], '
    '[Time_Field]=[Time_Field], [Currency_Field]=[Currency_Field] WHERE [ID]=-1'
)
DATE_ID3_BITS = '40e624e000000000'


def integer(value):
    return {'kind': 'Int64', 'value': value}


def currency(scaled):
    return {'kind': 'Currency', 'scaled': scaled}


def date(bits):
    return {'kind': 'DateTime', 'bits': bits}


def boolean(value):
    return {'kind': 'Boolean', 'value': value}


def null():
    return {'kind': 'Null'}


def group(column, assignments):
    # These ordinals are the frozen original getter/raw-root observations.
    # The model separately resolves every ID and requires the ordinal to agree.
    ordinals = {1: 0, 2: 1, 3: 2}
    return {
        'column': column,
        'updates': [
            {'ordinal': ordinals[identifier], 'row_id': {'ID': identifier},
             'expected': expected, 'replacement': replacement}
            for identifier, expected, replacement in assignments
        ],
    }


def choices():
    for header in [5, 6]:
        groups = [
            group('Large_Number', [(1, integer(1000), integer(-9223372036854775808))]),
            group('Currency_Field', [(1, currency(12345), currency(-9223372036854775808))]),
            group('Date_Field', [(1, date('0000000000000000'), date('8000000000000000'))]),
            group('Time_Field', [(1, date('3fe0000000000000'), date('3fd0000000000000'))]),
            group('Flag', [(1, boolean(False), boolean(True))]),
        ]
        yield case(header, 'same-row', groups, 1)
        groups = [
            group('Large_Number', [(2, null(), integer(9223372036854775807))]),
            group('Currency_Field', [(2, null(), currency(9223372036854775807))]),
            group('Date_Field', [(2, null(), date('3fd0000000000000'))]),
            group('Time_Field', [(2, null(), date('3fe0000000000000'))]),
            group('Flag', [(2, boolean(True), boolean(False))]),
        ]
        yield case(header, 'promote-null', groups, 1)
        groups = [
            group('Large_Number', [(1, integer(1000), null())]),
            group('Currency_Field', [(1, currency(12345), null())]),
            group('Date_Field', [(1, date('0000000000000000'), null())]),
            group('Time_Field', [(1, date('3fe0000000000000'), null())]),
            group('Flag', [(1, boolean(False), boolean(True))]),
        ]
        yield case(header, 'clear-values', groups, 1)
        groups = [
            group('Large_Number', [
                (1, integer(1000), integer(-101)),
                (2, null(), integer(9223372036854775807)),
            ]),
            group('Currency_Field', [
                (1, currency(12345), null()),
                (2, null(), currency(-10001)),
            ]),
            group('Date_Field', [
                (2, null(), date('3fd0000000000000')),
                (3, date(DATE_ID3_BITS), null()),
            ]),
            group('Time_Field', [
                (1, date('3fe0000000000000'), date('3fd0000000000000')),
                (3, date('3fd0000000000000'), date('3fe0000000000000')),
            ]),
            group('Flag', [
                (1, boolean(False), boolean(True)),
                (2, boolean(True), boolean(False)),
                (3, boolean(False), boolean(True)),
            ]),
        ]
        yield case(header, 'overlap-rows', groups, 3)
        groups = [
            group('Large_Number', [(2, null(), null())]),
            group('Currency_Field', [(2, null(), null())]),
            group('Date_Field', [(1, date('0000000000000000'), date('0000000000000000'))]),
            group('Time_Field', [(1, date('3fe0000000000000'), date('3fe0000000000000'))]),
            group('Flag', [(2, boolean(True), boolean(True))]),
        ]
        yield case(header, 'mixed-noops', groups, 0)
        groups = [group(name, []) for name in [
            'Flag', 'Large_Number', 'Date_Field', 'Time_Field', 'Currency_Field'
        ]]
        yield case(header, 'sql-empty', groups, 0, SQL_EMPTY)


def case(header, suffix, groups, changed, sql=None):
    return {
        'id': f'h{header}-{suffix}',
        'header': header,
        'transport': 'sql' if sql is not None else 'typed',
        'sql': sql,
        'native_request': {'table': 'PreservedRows', 'groups': groups},
        'expected_changed_rows': changed,
    }
