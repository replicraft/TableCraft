"""Check UTF-16 substring positions against original independent window models.

This verifies TableCraft's explicit binary/ordinal profiles, not native Access
locale search. The independently decoded source database remains read-only.
"""
import argparse
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run
from compare_large_tables import digest


def quoted(value):
    return "'" + value.replace("'", "''") + "'"


def utf16(value):
    raw = value.encode('utf-16-be')
    return [int.from_bytes(raw[i:i+2], 'big') for i in range(0, len(raw), 2)]


def position(haystack, needle, start, reverse):
    hay, pattern = utf16(haystack), utf16(needle)
    if reverse and start == -1:
        start = len(hay)
    if not hay or start > len(hay):
        return 0
    if not pattern:
        return start
    matches = [i for i in range(len(hay)-len(pattern)+1)
               if hay[i:i+len(pattern)] == pattern]
    if reverse:
        matches = [i for i in matches if i+len(pattern) <= start]
        return matches[-1]+1 if matches else 0
    return next((i+1 for i in matches if i >= start-1), 0)


def refused(cli, mode, expression, options=()):
    result = subprocess.run([str(cli), 'query', mode,
                             'SELECT 1 AS Before,'+expression, *options], capture_output=True)
    assert result.returncode != 0 and not result.stdout, (mode, expression)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--database', type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    before = digest(args.database)
    pairs = [('ababa', 'aba'), ('😀x😀', '😀'), ('😀x😀', 'x'),
             ('', ''), ('abc', ''), ('', 'x'), ('abc', 'abcd'),
             ('é', 'e\u0301'), ('XXpXXpXXPXXP', 'P'), ("O'Reilly", "'")]
    controls = 0
    for mode in ['eval', 'run']:
        for haystack, needle in pairs:
            for start in [1, 2, 4, 20]:
                for reverse in [False, True]:
                    expression = (f'InStrRev({quoted(haystack)},{quoted(needle)},{start},0)'
                                  if reverse else
                                  f'InStr({start},{quoted(haystack)},{quoted(needle)},0)')
                    expected = position(haystack, needle, start, reverse)
                    assert run(args.cli, 'query', mode, 'SELECT '+expression+' AS P') == f'P\n{expected}\n'
                    controls += 1
            expression = f'InStrRev({quoted(haystack)},{quoted(needle)})'
            expected = position(haystack, needle, -1, True)
            assert run(args.cli, 'query', mode, 'SELECT '+expression+' AS P') == f'P\n{expected}\n'
            controls += 1
        for expression in ["InStr(1,NULL,'x',0)", "InStrRev('x',NULL)"]:
            assert run(args.cli, 'query', mode, 'SELECT '+expression+' AS P') == 'P\nNULL\n'
            controls += 1
        for expression in ["InStr('a','a')", "InStr(1,'a','a',1)",
                           "InStr(1,'a','a',2)", "InStr(0,'a','a',0)",
                           "InStrRev('a','a',-2)", "InStrRev(NULL,'a',NULL)",
                           "InStrRev(NULL,'a',-1,NULL)", "InStr(1,NULL,'x',1/0)",
                           "InStr(,'a','a',0)"]:
            refused(args.cli, mode, expression)
            controls += 1
    for haystack, needle in pairs:
        for start in [1, 2, 4, 20]:
            for reverse in [False, True]:
                expression = (f'InStrRev({quoted(haystack)},{quoted(needle)},{start},2)'
                              if reverse else
                              f'InStr({start},{quoted(haystack)},{quoted(needle)},2)')
                expected = position(haystack, needle, start, reverse)
                assert run(args.cli, 'query', 'run', 'SELECT '+expression+' AS P',
                           '--collation', 'ordinal') == f'P\n{expected}\n'
                controls += 1
    tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                 RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
    rows = []
    for line in tagged.splitlines():
        row = line.split('\t')[1:]
        identity = int(row[0].removeprefix('I32:'))
        value = None if row[1] == 'N' else bytes.fromhex(row[1].removeprefix('T')).decode('utf-8')
        expected = ('NULL', 'NULL') if value is None else (
            str(position(value, '2', 1, False)), str(position(value, '2', -1, True)))
        rows.append((identity, *expected))
    expected = 'N\tF\tR\n' + ''.join(f'{n}\t{f}\t{r}\n' for n, f, r in sorted(rows))
    sql = "SELECT ORDR_NBR AS N,InStr(1,NBR_VALUE,'2',0) AS F,InStrRev(NBR_VALUE,'2') AS R FROM ROULETTE_TB ORDER BY N"
    for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
        assert run(args.cli, 'query', 'run', sql, '--database', args.database, *options) == expected
        controls += 1
    assert digest(args.database) == before
    print(f'Passed {controls} string-search controls; unchanged source {before}; native Access unrun.')


if __name__ == '__main__':
    main()
