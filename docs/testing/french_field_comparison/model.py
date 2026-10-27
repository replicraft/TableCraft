"""Independent expected values for the bounded French field-data replay."""

from french_comparison_latin_model import model_key


def short_hex(value):
    return value.encode('utf-16-be').hex()


def text(raw):
    return None if raw is None else raw.decode('utf-16-le')


def general_latin_key(value, weights):
    primary = bytearray()
    secondary = bytearray()
    for unit in value.rstrip(' '):
        inline, extra = weights[unit]
        primary.extend(inline)
        secondary.extend(extra or bytes([2]))
    while secondary and secondary[-1] <= 2:
        secondary.pop()
    return bytes(primary + bytes([1]) + secondary + bytes([0]))


class FieldModel:
    def __init__(self, captured, keys, weights):
        self.rows = {identifier: {field: text(value) for field, value in row.items()}
                     for identifier, row in captured['values'].items()}
        self.keys = keys
        self.weights = weights

    def short_key(self, value, french=True):
        if value is None:
            return (0, b'')
        return (1, self.keys[(1036 if french else 1033, short_hex(value))])

    def memo_key(self, value, french=True):
        if value is None:
            return (0, b'')
        return (1, model_key(value, self.weights) if french
                else general_latin_key(value, self.weights))

    def ordered_ids(self, field, identifiers=None, ordinal=False):
        identifiers = list(self.rows) if identifiers is None else identifiers
        if ordinal:
            key = lambda identifier: (self.rows[identifier][field] is not None,
                                      b'' if self.rows[identifier][field] is None else
                                      self.rows[identifier][field].encode('utf-16-be'), identifier)
        elif field.endswith('Short'):
            key = lambda identifier: (self.short_key(self.rows[identifier][field], field.startswith('French')), identifier)
        else:
            key = lambda identifier: (self.memo_key(self.rows[identifier][field], field.startswith('French')), identifier)
        return sorted(identifiers, key=key)


def cases(model):
    table = 'Comparisons'
    rows = model.rows
    for field in ('FrenchShort', 'GeneralShort'):
        yield (field + '-sort', f'SELECT ID FROM {table} ORDER BY {field},ID',
               ['ID'], [[identifier] for identifier in model.ordered_ids(field)], None, [])
    groups = {}
    for identifier, row in rows.items():
        key = model.short_key(row['FrenchShort'])
        if key not in groups:
            groups[key] = [row['FrenchShort'], 0]
        groups[key][1] += 1
    yield ('short-groups', f'SELECT FrenchShort AS V,Count(*) AS N FROM {table} GROUP BY FrenchShort',
           ['V', 'N'], list(groups.values()), None, [])
    yield ('short-distinct', f'SELECT DISTINCT FrenchShort AS V FROM {table}',
           ['V'], [[value] for value, _ in groups.values()], None, [])
    present = [identifier for identifier in rows if rows[identifier]['FrenchShort'] is not None]
    minimum = min(present, key=lambda identifier: model.short_key(rows[identifier]['FrenchShort']))
    maximum = max(present, key=lambda identifier: model.short_key(rows[identifier]['FrenchShort']))
    yield ('short-extrema', f'SELECT Min(FrenchShort) AS Lo,Max(FrenchShort) AS Hi FROM {table}',
           ['Lo', 'Hi'], [[rows[minimum]['FrenchShort'], rows[maximum]['FrenchShort']]], None, [])
    pairs = [[left, right] for left in range(1, 21) for right in range(1, 21)
             if rows[left]['FrenchShort'] is not None and rows[right]['FrenchShort'] is not None
             and model.short_key(rows[left]['FrenchShort']) == model.short_key(rows[right]['FrenchShort'])]
    yield ('short-join', f'SELECT a.ID AS L,b.ID AS R FROM {table} AS a INNER JOIN {table} AS b ON a.FrenchShort=b.FrenchShort WHERE a.ID<=20 AND b.ID<=20 ORDER BY a.ID,b.ID',
           ['L', 'R'], pairs, None, [])
    yield ('derived-policy', f'SELECT R.ID FROM (SELECT Q.ID,Q.W AS V FROM (SELECT ID,FrenchShort AS W FROM {table}) AS Q) AS R ORDER BY R.V,R.ID',
           ['ID'], [[identifier] for identifier in model.ordered_ids('FrenchShort')], None, [])
    yield ('explicit-ordinal', f'SELECT ID FROM {table} ORDER BY FrenchShort,ID',
           ['ID'], [[identifier] for identifier in model.ordered_ids('FrenchShort', ordinal=True)], None, ['--collation', 'ordinal'])
    for block, length in enumerate((256, 1024, 20000)):
        first = 21 + block * 6
        identifiers = list(range(first, first + 6))
        for field in ('FrenchMemo', 'GeneralMemo'):
            yield (f'{field}-{length}-sort', f'SELECT ID FROM {table} WHERE ID>={first} AND ID<={first + 5} ORDER BY {field},ID',
                   ['ID'], [[identifier] for identifier in model.ordered_ids(field, identifiers)], None, [])
        for label, left, right in [('primary', first, first + 1), ('accent', first + 2, first + 3)]:
            result = model.memo_key(rows[left]['FrenchMemo']) < model.memo_key(rows[right]['FrenchMemo'])
            sql = f'SELECT (SELECT FrenchMemo FROM {table} WHERE ID={left})<(SELECT FrenchMemo FROM {table} WHERE ID={right}) AS C'
            yield (f'memo-{length}-{label}', sql, ['C'], [[result]], None, [])
    empty_key = model.short_key('')
    matching = [[identifier] for identifier, row in rows.items()
                if row['FrenchMemo'] is None or (identifier <= 20 and model.short_key(row['FrenchMemo']) == empty_key)]
    yield ('memo-null-empty', f"SELECT ID FROM {table} WHERE FrenchMemo IS NULL OR FrenchMemo='' ORDER BY ID",
           ['ID'], matching, None, [])
    yield ('memo-full-length', f'SELECT ID,Len(FrenchMemo) AS N FROM {table} ORDER BY ID',
           ['ID', 'N'], [[identifier, None if row['FrenchMemo'] is None else len(row['FrenchMemo'].encode('utf-16-le')) // 2]
                        for identifier, row in rows.items()], None, [])
    yield ('policy-conflict', f'SELECT ID FROM {table} WHERE FrenchShort=GeneralShort',
           [], [], 'conflicting text comparison policies', [])
    yield ('like-refusal', f"SELECT ID FROM {table} WHERE FrenchShort LIKE 'co*'",
           [], [], 'expression form is not implemented', ['--like-mode', 'ansi89'])
    yield ('operation-default-search', f"SELECT InStr(1,FrenchShort,'ot',2) AS P FROM {table} WHERE ID=3",
           ['P'], [[2]], None, [])
    yield ('unregistered-lcid', f"SELECT InStr(1,FrenchShort,'ot',1036) AS P FROM {table} WHERE ID=3",
           [], [], 'text comparison requires a collation', [])
    yield ('binary-search', f"SELECT InStr(1,FrenchShort,'ot',0) AS P FROM {table} WHERE ID=3",
           ['P'], [[2]], None, [])
    yield ('comparison-quota', f'SELECT ID FROM {table} WHERE ID>=33 AND ID<=38 ORDER BY FrenchMemo,ID',
           [], [], 'Bytes resource limit exceeded', ['--comparison-bytes', '1048576'])
