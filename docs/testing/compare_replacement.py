"""Check Replace's explicit binary/ordinal profile against a UTF-16 list model.

Independent native getters supply input rows only. No Access SQL or stored active
content is executed, and the original source bytes must remain unchanged.
"""
import argparse
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest
from compare_string_search import quoted, utf16


def model(source, find, replacement, start, count):
    source, find, replacement = utf16(source), utf16(find), utf16(replacement)
    start -= 1
    if start >= len(source):
        return ''
    if start and 0xDC00 <= source[start] <= 0xDFFF:
        raise ValueError('start splits a surrogate pair')
    source = source[start:]
    result = []
    index = 0
    while index < len(source):
        if find and count != 0 and source[index:index+len(find)] == find:
            result.extend(replacement)
            index += len(find)
            if count > 0:
                count -= 1
        else:
            result.append(source[index])
            index += 1
    return b''.join(unit.to_bytes(2, 'big') for unit in result).decode('utf-16-be')


def spelling(value):
    return display('T'+value.encode('utf-8').hex())


def refused(cli, mode, expression, options=()):
    result = subprocess.run([str(cli), 'query', mode, 'SELECT 1,'+expression, *options], capture_output=True)
    assert result.returncode != 0 and not result.stdout, expression[:160]
    return result.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--database', type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    before = digest(args.database)
    controls = 0
    cases = [('banana', 'an', 'X'), ('aaaa', 'aa', 'a'), ('é😀a😀', '😀', '中'),
             ("a'b", "'", ''), ('', 'a', 'b'), ('abc', '', 'x')]
    for mode, comparison, options in [('eval', 0, []), ('run', 0, []),
                                      ('run', 2, ['--collation', 'ordinal']),
                                      ('run', -1, ['--collation', 'ordinal'])]:
        for source, find, replacement in cases:
            for start in [1, 2, 3, 20]:
                for count in [-1, 0, 1, 2]:
                    expression = f'Replace({quoted(source)},{quoted(find)},{quoted(replacement)},{start},{count},{comparison})'
                    try:
                        expected = model(source, find, replacement, start, count)
                    except ValueError:
                        refused(args.cli, mode, expression, options)
                    else:
                        assert run(args.cli, 'query', mode, 'SELECT '+expression+' AS R', *options) == 'R\n'+spelling(expected)+'\n'
                    controls += 1
    for mode in ['eval', 'run']:
        for expression, expected in [("Replace('banana','an','X')", 'bXXa'),
                                     ("Replace('banana','an','X',,,)", 'bXXa'),
                                     ("Replace('banana','an','X',,1,)", 'bXana'),
                                     ("Replace('banana','an','X',3,0)", 'nana')]:
            assert run(args.cli, 'query', mode, 'SELECT '+expression+' AS R') == 'R\n'+spelling(expected)+'\n'
            controls += 1
        for expression in ["Replace(NULL,'a','b')", "Replace('a',NULL,'b')",
                           "Replace('a','a',NULL)", "Replace('a','a','x',0)",
                           "Replace('','a','x',1,-2)", "Replace(NULL,'a','x',1/0)",
                           "Replace('a','a','x',1,-1,NULL)", "Replace('a','a','x',,,1)",
                           "Replace('a','a','x',,,2)", "Replace('a','a','x',,,-1)"]:
            refused(args.cli, mode, expression)
            controls += 1
        expression = f"Replace({quoted('a'*513)},'a',{quoted('x'*128)})"
        # Scalar eval has a smaller work/payload budget than retrieval. Both
        # paths must refuse expansion atomically at their applicable ceiling.
        expected_error = b'resource limit exceeded' if mode == 'eval' else b'text output limit exceeded'
        assert expected_error in refused(args.cli, mode, expression)
        controls += 1
    tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                 RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
    rows = []
    for line in tagged.splitlines():
        row = line.split('\t')[1:]
        identity = int(row[0].removeprefix('I32:'))
        value = '' if row[1] == 'N' else bytes.fromhex(row[1].removeprefix('T')).decode('utf-8')
        rows.append((identity, spelling(model(value, '2', 'X', 1, -1))))
    expected = 'N\tR\n'+''.join(f'{n}\t{value}\n' for n, value in sorted(rows))
    sql = "SELECT ORDR_NBR AS N,Replace(Nz(NBR_VALUE,''),'2','X') AS R FROM ROULETTE_TB ORDER BY N"
    for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
        assert run(args.cli, 'query', 'run', sql, '--database', args.database, *options) == expected
        controls += 1
    assert digest(args.database) == before
    print(f'Passed {controls} replacement controls; unchanged source {before}; native Access unrun.')


if __name__ == '__main__':
    main()
