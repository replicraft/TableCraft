"""Check original native UNION metadata, independent SQL assembly and inert inputs.

Jackcess does not execute SQL. Expected row operations are independently modelled;
this is not native Windows Access qualification.
"""
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
from compare_cstr import spelling

PROFILES = [('V2000', 'jet4.mdb'), ('V2007', '2007.accdb'), ('V2016', '2016.accdb'), ('V2019', '2019.accdb')]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--directory', type=Path, required=True)
    parser.add_argument('--generate', action='store_true')
    args = parser.parse_args()
    assert hashlib.sha256(args.jar.read_bytes()).hexdigest() == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-union-oracle-') as classes:
        subprocess.run(['javac', '-cp', str(args.jar), '-d', classes, *[str(RECIPES / (name + '.java')) for name in ['SavedUnionFixture', 'SavedReadOracle', 'TableDataOracle']]], check=True)
        classpath = str(args.jar) + os.pathsep + classes
        def java(name, *values):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', classpath, name, *map(str, values)], text=True, encoding='utf-8')
        def cli(*values, accepted=True):
            result = subprocess.run([str(args.cli), *map(str, values)], capture_output=True, text=True, encoding='utf-8')
            if accepted:
                assert result.returncode == 0, (values, result.stderr)
            else:
                assert result.returncode == 2 and not result.stdout, (values, result)
            return result.stdout
        if args.generate:
            args.directory.mkdir(exist_ok=False)
            for profile, suffix in PROFILES:
                java('SavedUnionFixture', args.directory / ('union-' + suffix), profile)
        for _, suffix in PROFILES:
            path = args.directory / ('union-' + suffix)
            before = path.read_bytes()
            names = java('SavedReadOracle', path).splitlines()
            assert len(names) == 14
            getter = java('TableDataOracle', path, 'Items', 'Others')
            items, others = [], []
            for line in getter.splitlines():
                table, value = line.split('\t')
                (items if table == 'Items' else others).append(int(display(value)))
            assert items == [1, 2, 3] and others == [2, 4]
            all_ids = sorted([x for x in items if x >= 2] + [x for x in items if x <= 2])
            expected = {
                'Leaf': ('N', [x for x in items if x <= 2]),
                'UnionAll': ('N', all_ids),
                'UnionDistinct': ('N', sorted(set(all_ids), reverse=True)),
                'UnionChain': ('N', sorted(set([x for x in items if x in [1, 2]]) | {3})),
                'UnionReferences': ('N', sorted([x for x in items if x <= 2] + others)),
                'UnionParameter': ('N', sorted([x for x in items if x >= 2] + [x for x in others if x <= 2])),
                'FromUnion': ('Chosen', sorted(all_ids, reverse=True)),
                'UnionComments': ('N', [1, 2]),
                'UnionUnicode': ('N', [spelling('é🦀')]),
                'UnionNulls': ('N', ['NULL']),
            }
            for name in names:
                sql = java('SavedReadOracle', path, name)
                normalized = cli('query', 'parse', sql)
                assert cli('query', 'saved', '--database', path, name) == normalized, name
                controls += 1
                params = ['--param', 'Floor=long:2'] if name == 'UnionParameter' else []
                for query in [sql, None, f'SELECT * FROM [{name}]']:
                    command = ['query', 'run']
                    command += ['--saved', name] if query is None else [query]
                    command += ['--database', path, '--collation', 'ordinal', *params]
                    text = cli(*command, accepted=name in expected)
                    if name in expected:
                        heading, values = expected[name]
                        actual = list(csv.reader(StringIO(text), delimiter='\t'))
                        assert actual == [[heading], *[[str(value)] for value in values]], (name, query, actual, values)
                    controls += 1
            for options in [[], ['--param', 'Floor=text:2']]:
                cli('query', 'run', '--saved', 'UnionParameter', '--database', path, *options, accepted=False)
                controls += 1
            # Mutate copies only: independent raw layout helpers locate the
            # selected query's physical fixed metadata, not production constants.
            ids = catalog_ids(args, path)
            definition = ids['MSysQueries']
            object_offset, _, _ = column_layout(before, definition, 'ObjectId')
            layout = fields(before, definition)
            def fixed_offset(name, width):
                at = layout[name][0]
                assert before[at + 15] & 1
                assert int.from_bytes(before[at + 23:at + 25], 'little') == width
                return int.from_bytes(before[at + 21:at + 23], 'little')
            attribute_offset = fixed_offset('Attribute', 1)
            flag_offset = fixed_offset('Flag', 2)
            selected = [slot for slot in roots(before, definition) if int.from_bytes(before[slot[0] + 2 + object_offset:slot[0] + 6 + object_offset], 'little', signed=True) == ids['UnionAll']]
            positions = {before[slot[0] + 2 + attribute_offset]: slot[0] for slot in selected}
            for attribute, offset, value in [(1, flag_offset, b'\x01\x00'), (3, flag_offset, b'\x10\x00'), (5, attribute_offset, b'\xc8')]:
                changed = bytearray(before)
                start = positions[attribute] + 2 + offset
                changed[start:start + len(value)] = value
                with tempfile.TemporaryDirectory(prefix='tablecraft-union-refusal-') as temporary:
                    copy = Path(temporary) / path.name
                    copy.write_bytes(changed)
                    for command in [['query','saved','--database',copy,'UnionAll'],['query','run','--database',copy,'--saved','UnionAll'],['query','run','SELECT * FROM UnionAll','--database',copy]]:
                        cli(*command, accepted=False)
                        controls += 1
                    assert copy.read_bytes() == changed
            assert path.read_bytes() == before
            print(path.name, len(before), hashlib.sha256(before).hexdigest())
    print(f'Passed {controls} saved UNION assembly/execution/getter/refusal controls; native Access unrun.')


if __name__ == '__main__':
    main()
