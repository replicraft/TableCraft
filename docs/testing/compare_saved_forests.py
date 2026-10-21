"""Independent join-forest SQL rendering and relational models; native Access unrun."""
import argparse
import csv
import hashlib
from io import StringIO
import os
from pathlib import Path
import subprocess
import tempfile
from compare_browsing import RECIPES, display
from compare_saved_queries import catalog_ids
from compare_composite_constraints import roots
from compare_complex_links import column_layout
from compare_attachment_columns import fields


PROFILES = [('V2000', 'jet4.mdb'), ('V2007', '2007.accdb'), ('V2016', '2016.accdb'), ('V2019', '2019.accdb')]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['jar', 'cli', 'directory']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--generate', action='store_true')
    args = parser.parse_args()
    controls = 0
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    with tempfile.TemporaryDirectory(prefix='tablecraft-forest-oracle-') as classes:
        subprocess.run(['javac', '-cp', str(args.jar), '-d', classes, *[str(RECIPES / (name + '.java')) for name in ['SavedForestFixture', 'SavedReadOracle', 'TableDataOracle']]], check=True)

        def java(name, *values):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', str(args.jar) + os.pathsep + classes, name, *map(str, values)], text=True, encoding='utf-8')

        def cli(*values, accepted=True):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True)
            if accepted:
                assert result.returncode == 0, (values, result.stderr)
            else:
                assert result.returncode == 2 and (not result.stdout), (values, result)
            return result.stdout
        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedForestFixture', args.directory / ('forest-' + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ('forest-' + suffix)
            before = path.read_bytes()
            inputs = {key: [] for key in 'abcd'}
            for line in java('TableDataOracle', path, 'AData', 'BData', 'CData', 'DData').splitlines():
                table, value = line.split('\t')
                inputs[table[0].lower()].append(int(display(value)))
            assert list(inputs.values()) == [[1, 2, 4], [2, 3, 4], [1, 2, 3], [2, 4, 5]]

            def leaf(name):
                return [{name: value} for value in inputs[name]]

            def equal(row, left, right):
                return row.get(left) is not None and row.get(right) is not None and (row[left] == row[right])

            def different(row, key):
                return row.get(key) is not None and row[key] != 2

            def join(left, right, kind, predicate):
                lkeys = set().union(*(row.keys() for row in left))
                rkeys = set().union(*(row.keys() for row in right))
                result = []
                seen = set()
                for lrow in left:
                    matches = []
                    for index, rrow in enumerate(right):
                        if predicate(lrow | rrow):
                            matches.append(lrow | rrow)
                            seen.add(index)
                    result += matches
                    if not matches and kind == 'left':
                        result.append(lrow | dict.fromkeys(rkeys))
                if kind == 'right':
                    result.extend((dict.fromkeys(lkeys) | row for index, row in enumerate(right) if index not in seen))
                return result

            def ab(kind='left', extra=lambda row: True):
                return join(leaf('a'), leaf('b'), kind, lambda row: equal(row, 'a', 'b') and extra(row))
            base = lambda kind: join(ab(kind), leaf('c'), kind, lambda row: equal(row, 'b', 'c'))
            expected = {
                'GraphInner': base('inner'),
                'GraphLeft': base('left'),
                'GraphRight': base('right'),
                'GraphReverse': join(leaf('a'), join(leaf('c'), leaf('b'), 'left', lambda row: equal(row, 'c', 'b')), 'right', lambda row: equal(row, 'a', 'b')),
                'GraphMixed': join(ab(), leaf('c'), 'inner', lambda row: equal(row, 'a', 'c')),
                'GraphInnerPredicate': join(ab(extra=lambda row: different(row, 'b')), leaf('c'), 'left', lambda row: equal(row, 'b', 'c')),
                'GraphAncestorPredicate': join(ab(), leaf('c'), 'inner', lambda row: equal(row, 'a', 'c') and different(row, 'a')),
                'GraphFour': join(ab(), join(leaf('c'), leaf('d'), 'right', lambda row: equal(row, 'c', 'd')), 'inner', lambda row: equal(row, 'b', 'c')),
                'GraphDisconnected': [c | d | row for c in leaf('c') for d in leaf('d') for row in ab()],
                'GraphComma': [a | b | c for a in leaf('a') for b in leaf('b') for c in leaf('c')],
                'GraphPairComma': [a | b for a in leaf('a') for b in leaf('b')],
                'GraphPairReverse': ab(extra=lambda row: different(row, 'b')),
                'GraphDeepInner': join(ab('inner'), leaf('c'), 'inner', lambda row: equal(row, 'b', 'c') and equal(row, 'a', 'c')),
            }
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 14
            for name in names:
                sql = java('SavedReadOracle', path, name)
                assert cli('query', 'saved', '--database', path, name) == cli('query', 'parse', sql), name
                controls += 1
                width = 4 if name in ['GraphFour', 'GraphDisconnected'] else 2 if name.startswith('GraphPair') else 3
                keys = 'abcd'[:width]
                records = expected.get(name, [])
                records = sorted(records, key=lambda row: tuple(((row.get(key) is not None, row.get(key, 0)) for key in keys)))
                wanted = [['Key' + key.upper() for key in keys], *[[str(row[key]) if row[key] is not None else 'NULL' for key in keys] for row in records]]
                for query in [sql, None, f'SELECT * FROM [{name}]']:
                    command = ['query', 'run'] + (['--saved', name] if query is None else [query]) + ['--database', path]
                    actual = cli(*command, accepted=name != 'GraphUdf')
                    if name != 'GraphUdf':
                        assert list(csv.reader(StringIO(actual), delimiter='\t')) == wanted, (name, actual, wanted)
                    controls += 1
            ids = catalog_ids(args, path)
            definition = ids['MSysQueries']
            object_offset, _, _ = column_layout(before, definition, 'ObjectId')
            layout = fields(before, definition)

            def offset(name, width):
                at = layout[name][0]
                assert before[at + 15] & 1 and int.from_bytes(before[at + 23:at + 25], 'little') == width
                return int.from_bytes(before[at + 21:at + 23], 'little')
            attribute = offset('Attribute', 1)
            flag = offset('Flag', 2)
            selected = [slot for slot in roots(before, definition) if int.from_bytes(before[slot[0] + 2 + object_offset:slot[0] + 6 + object_offset], 'little', signed=True) == ids['GraphInnerPredicate'] and before[slot[0] + 2 + attribute] == 7]
            assert len(selected) == 3
            for value in [3, 4]:
                changed = bytearray(before)
                start = selected[-1][0] + 2 + flag
                changed[start:start + 2] = value.to_bytes(2, 'little')
                with tempfile.TemporaryDirectory(prefix='tablecraft-forest-refusal-') as temporary:
                    copy = Path(temporary) / path.name
                    copy.write_bytes(changed)
                    for command in [['query', 'saved', '--database', copy, 'GraphInnerPredicate'], ['query', 'run', '--database', copy, '--saved', 'GraphInnerPredicate']]:
                        cli(*command, accepted=False)
                        controls += 1
                    assert copy.read_bytes() == changed
            assert path.read_bytes() == before
            print(path.name, len(before), hashlib.sha256(before).hexdigest())
    print(f'Passed {controls} saved-forest assembly/execution/getter/refusal controls; native Access unrun.')
if __name__ == '__main__':
    main()
