"""Check bounded Space/String/BMP StrReverse using independent scalar models.

No native Access SQL executes. Original independently decoded table rows supply
inputs only, and their source database hash must remain unchanged.
"""
import argparse
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest
from compare_string_search import quoted, utf16


def spelling(value):
    return display('T'+value.encode('utf-8').hex())


def check(cli, mode, expression, expected):
    result = run(cli, 'query', mode, 'SELECT '+expression+' AS R')
    expected = 'NULL' if expected is None else spelling(expected)
    assert result == 'R\n'+expected+'\n', expression


def refused(cli, mode, expression):
    result = subprocess.run([str(cli), 'query', mode, 'SELECT 1,'+expression], capture_output=True)
    assert result.returncode != 0 and not result.stdout, expression


def reversed_units(value):
    units = utf16(value)
    units.reverse()
    return b''.join(unit.to_bytes(2, 'big') for unit in units).decode('utf-16-be')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--database', type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    before = digest(args.database)
    controls = 0
    for mode in ['eval', 'run']:
        for count in [0, 1, 4, 10]:
            check(args.cli, mode, f'Space({count})', ' '*count)
            controls += 1
            for character in ['A', 'é中', '中', 'A😀']:
                check(args.cli, mode, f'String({count},{quoted(character)})', character[0]*count)
                controls += 1
            for code in [0, 1, 42, 65, 127, 256, 298, 512]:
                check(args.cli, mode, f'String({count},{code})', chr(code % 256)*count)
                controls += 1
        for value in ['', 'abcdef', 'é中', 'e\u0301中', "O'Reilly"]:
            check(args.cli, mode, f'StrReverse({quoted(value)})', reversed_units(value))
            controls += 1
        for expression, expected in [('Space(NULL)', None), ("String(NULL,'x')", None),
                                     ('String(3,NULL)', None), ('Space(2.5)', '  '),
                                     ("String(3.5,'ab')", 'aaaa'), ('String(2,65.5)', 'BB')]:
            check(args.cli, mode, expression, expected)
            controls += 1
        for expression in ['Space(-1)', 'Space(2147483648)', "String(1,'')",
                           'String(1,128)', 'String(1,255)', 'String(1,384)',
                           "String(2,'😀')", "StrReverse('😀x')", 'StrReverse(NULL)',
                           'String(NULL,1/0)', "Space(1,1/0)", "String(1,'x',1/0)",
                           'Space(2147483647)']:
            refused(args.cli, mode, expression)
            controls += 1
    tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                 RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
    rows = []
    for line in tagged.splitlines():
        row = line.split('\t')[1:]
        identity = int(row[0].removeprefix('I32:'))
        value = '' if row[1] == 'N' else bytes.fromhex(row[1].removeprefix('T')).decode('utf-8')
        rows.append((identity, spelling(reversed_units(value)), spelling(' '*len(utf16(value)))))
    expected = 'N\tR\tS\n'+''.join(f'{n}\t{r}\t{s}\n' for n,r,s in sorted(rows))
    sql = "SELECT ORDR_NBR AS N,StrReverse(Nz(NBR_VALUE,'')) AS R,Space(Len(Nz(NBR_VALUE,''))) AS S FROM ROULETTE_TB ORDER BY N"
    for options in [[], ['--index', 'ROULETTE_TB', 'Number']]:
        assert run(args.cli, 'query', 'run', sql, '--database', args.database, *options) == expected
        controls += 1
    assert digest(args.database) == before
    print(f'Passed {controls} text-construction controls; unchanged source {before}; native Access unrun.')


if __name__ == '__main__':
    main()
