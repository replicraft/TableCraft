"""Fourteen fixed original-only choices; no image construction or processes."""

SQL_EMPTY = (
    'UPDATE [PreservedRows] SET [Short_Text]=[Short_Text], '
    '[Flag]=[Flag] WHERE [ID]=-1'
)


def text(value):
    return {'kind': 'Null'} if value is None else {'kind': 'Text', 'value': value}


def boolean(value):
    return {'kind': 'Boolean', 'value': value}


def group(column, assignments):
    return {
        'column': column,
        'updates': [
            {'ordinal': identifier - 1, 'row_id': {'ID': identifier},
             'expected': before, 'replacement': after}
            for identifier, before, after in assignments
        ],
    }


def choices():
    for header in [5, 6]:
        yield case(header, 'beta-flag', [
            group('Short_Text', [(1, text('alpha'), text('beta'))]),
            group('Flag', [(1, boolean(False), boolean(True))]),
        ], 1, {'1': 'fffe62657461'})
        yield case(header, 'text-noop-flag', [
            group('Short_Text', [(1, text('alpha'), text('alpha'))]),
            group('Flag', [(1, boolean(False), boolean(True))]),
        ], 1, {'1': 'fffe616c706861'})
        yield case(header, 'null-flag', [
            group('Short_Text', [(1, text('alpha'), text(None))]),
            group('Flag', [(1, boolean(False), boolean(True))]),
        ], 1, {'1': ''})
        yield case(header, 'empty-null-flag', [
            group('Short_Text', [(2, text(None), text(''))]),
            group('Flag', [(2, boolean(True), boolean(False))]),
        ], 1, {'2': ''})
        yield case(header, 'disjoint-rows', [
            group('Short_Text', [(1, text('alpha'), text('beta'))]),
            group('Flag', [(3, boolean(False), boolean(True))]),
        ], 2, {'1': 'fffe62657461'})
        yield case(header, 'all-noops', [
            group('Short_Text', [(1, text('alpha'), text('alpha')), (2, text(None), text(None))]),
            group('Flag', [(1, boolean(False), boolean(False)), (2, boolean(True), boolean(True))]),
        ], 0, {'1': 'fffe616c706861', '2': ''})
        yield case(header, 'sql-empty', [group('Short_Text', []), group('Flag', [])], 0, {}, SQL_EMPTY)


def case(header, suffix, groups, changed, payloads, sql=None):
    return {
        'id': f'h{header}-{suffix}',
        'header': header,
        'transport': 'sql' if sql is not None else 'typed',
        'sql': sql,
        'native_request': {'table': 'PreservedRows', 'groups': groups},
        'literal_text_payloads': payloads,
        'expected_changed_rows': changed,
    }
