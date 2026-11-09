"""Twenty-four literal request choices; source binding is deliberately separate."""

FAMILIES = [
    ('int64', 'Large_Number', {'kind': 'Int64', 'value': 1000},
     {'kind': 'Int64', 'value': -9223372036854775808},
     {'kind': 'Int64', 'value': 9223372036854775807}),
    ('currency', 'Currency_Field', {'kind': 'Currency', 'scaled': 12345},
     {'kind': 'Currency', 'scaled': -9223372036854775808},
     {'kind': 'Currency', 'scaled': 9223372036854775807}),
    ('date', 'Date_Field', {'kind': 'DateTime', 'bits': '0000000000000000'},
     {'kind': 'DateTime', 'bits': '8000000000000000'},
     {'kind': 'DateTime', 'bits': '3fd0000000000000'}),
]


def choices():
    """Yield the closed 2 x 3 x 4 domain, without source bytes or construction."""
    for header in [5, 6]:
        for family, column, present, change, inserted in FAMILIES:
            for recipe, identifier, before, after in [
                ('change', 1, present, change),
                ('null-to-value', 2, {'kind': 'Null'}, inserted),
                ('value-to-null', 1, present, {'kind': 'Null'}),
                ('null-noop', 2, {'kind': 'Null'}, {'kind': 'Null'}),
            ]:
                yield {
                    'id': f'h{header}-{family}-{recipe}',
                    'header': header,
                    'recipe': recipe,
                    'table': 'PreservedRows',
                    'column': column,
                    'row_id': {'ID': identifier},
                    'expected': before,
                    'replacement': after,
                    'transport': 'typed',
                    'sql': None,
                }
