"""Compare explicit saved SELECT execution with getters and independent SQL assembly.

Inputs must use the original SavedSelectFixture recipe. Jackcess supplies stored
metadata SQL and inert table getters, never query execution or Access SQL evidence.
"""
import argparse
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import display

RECIPES = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--jar', required=True, type=Path)
    parser.add_argument('--cli', required=True, type=Path)
    parser.add_argument('--input', required=True, action='append', type=Path)
    args = parser.parse_args()
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-saved-execution-') as directory:
        subprocess.run(['javac', '-cp', str(args.jar), '-d', directory,
                        str(RECIPES / 'SavedSelectOracle.java'), str(RECIPES / 'TableDataOracle.java')], check=True)
        classpath = str(args.jar) + os.pathsep + directory
        def java(recipe, *options):
            return subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', classpath,
                                            recipe, *map(str, options)], text=True, encoding='utf-8')
        for path in args.input:
            original = path.read_bytes()
            names = java('SavedSelectOracle', path).splitlines()
            assert len(names) in (13, 21, 22, 28), 'Not an original saved-SELECT recipe profile'
            getter = java('TableDataOracle', path, 'Items', 'Order Items').splitlines()
            values = {line.split('\t')[0]: list(map(display, line.split('\t')[1:])) for line in getter}
            assert len(getter) == 2 and len(values['Items']) == 2 and len(values['Order Items']) == 1
            item_id, label = values['Items']
            assert item_id == '1', 'Recipe ID changed'
            ordinary = [['ID', 'Label'], [item_id, label]]
            joined = [['ID', 'Label', 'ID'], [item_id, label, item_id]]
            if any(name.startswith(('Join', 'Compound')) or name == 'ParameterJoin' for name in names):
                right = java('TableDataOracle', path, 'RightItems', 'Ordres 🦀').splitlines()
                assert [list(map(display, line.split('\t')[1:])) for line in right] == [[item_id], [item_id]]
            expected = {
                'Constants': [['Answer'], ['42']],
                'AllItems': ordinary,
                'Columns': [['Result ID', 'Result Label'], [item_id, label]],
                'DistinctLabels': [['Label'], [label]],
                'TopPercent': [['ID'], [item_id]],
                'Grouped': [['ID', 'Label', 'Total'], [item_id, label, '1']],
                'ExplicitStar': ordinary,
                'UnicodeNames': [['Résultat 🦀'], values['Order Items']],
                'PromptText': [['Pick label?'], ['"é🦀"']],
                'DefaultTextLength': [['P'], ['""']],
                'UnspecifiedTextLength': [['P'], ['""']],
                'MaxTextSyntax': [['P'], ['""']],
                'OrderedParameterClause': [['ID'], [item_id]],
                'ParameterJoin': joined,
                'PunctuationName': [["Floor's value?"], ['7']],
                'AllParameterTypes': [['Chosen'], ['42']],
                'JoinSelf': [['ID', 'Label', 'ID', 'Label'], [item_id, label, item_id, label]],
                'JoinUnicodeTable': [['ID', 'ID', 'Label'], [item_id, item_id, label]],
                'JoinNamesReverse': [['ID', 'ID', 'Label'], [item_id, item_id, label]],
            }
            refused = {'DistinctRowOwner', 'FunctionInert', 'SourceIsQuery', 'OmittedType', 'TopZero', 'OrderedParameterClause'}
            for name in names:
                options = []
                if name in ('Grouped', 'DistinctLabels'):
                    options += ['--collation', 'ordinal']
                if name == 'DistinctLabels':
                    options += ['--like-mode', 'ansi89']
                bindings = {
                    'PromptText': ['Pick label?=text:é🦀'],
                    'DefaultTextLength': ['P=text:'],
                    'UnspecifiedTextLength': ['P=text:'],
                    'MaxTextSyntax': ['P=text:'],
                    'OrderedParameterClause': ['pFloor=long:0', 'pCeiling=long:2'],
                    'ParameterJoin': ['pFloor=long:1'],
                    'PunctuationName': ["Floor's value?=long:7"],
                    'AllParameterTypes': [name + ('=long:42' if name == 'pLong' else '=null:') for name in
                                          ['pBool', 'pByte', 'pShort', 'pLong', 'pMoney', 'pSingle',
                                           'pDouble', 'pDate', 'pBinary', 'pText', 'pOLE', 'pGuid']],
                }.get(name, [])
                for binding in bindings:
                    options += ['--param', binding]
                sql = java('SavedSelectOracle', path, name)
                wanted = expected.get(name)
                if wanted is None and name not in refused:
                    assert name.startswith(('JoinInner', 'JoinLeft', 'JoinRight', 'Compound'))
                    wanted = joined
                for input_args in (['--saved', name], [sql]):
                    result = subprocess.run([str(args.cli), 'query', 'run', *input_args,
                                             '--database', str(path), '--format', 'tsv', *options],
                                            capture_output=True, text=True, encoding='utf-8')
                    if name in refused:
                        assert result.returncode == 2 and not result.stdout, (path.name, name, result)
                    else:
                        assert result.returncode == 0 and not result.stderr, (path.name, name, result.stderr)
                        actual = list(csv.reader(io.StringIO(result.stdout), delimiter='\t', strict=True))
                        assert actual == wanted, (path.name, name, actual, wanted)
                    controls += 1
            for name, options in [('missing', []), ('ParameterJoin', []),
                                  ('ParameterJoin', ['--param', 'pFloor=text:bad']),
                                  ('DistinctLabels', ['--collation', 'ordinal'])]:
                if name != 'missing' and name not in names:
                    continue
                result = subprocess.run([str(args.cli), 'query', 'run', '--saved', name,
                                         '--database', str(path), *options], capture_output=True)
                assert result.returncode == 2 and not result.stdout
                controls += 1
            assert path.read_bytes() == original
            print(path.name, len(names), 'saved definitions; unchanged', hashlib.sha256(original).hexdigest(), flush=True)
    print(controls, 'saved/direct execution/getter/refusal controls passed; native Access SQL unrun')


if __name__ == '__main__':
    main()
