"""Compare explicit DATE-serial operators to independent raw-bit/float models.

Calendar intervals are deliberately outside this recipe. Jackcess supplies only
inert stored DATE bits; this is not native Access SQL arithmetic qualification.
"""
import argparse
import datetime
import struct
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, run
from compare_cached_pages import date_display
from compare_large_tables import digest

EPOCH = datetime.datetime(1899, 12, 30)
FIRST_DAY = -657434
LAST_DAY = 2958465
JACKCESS_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'


def encoded(value):
    return struct.pack('>d', value).hex()


def serial(text):
    value = datetime.datetime.fromisoformat(text)
    whole = (value.date() - EPOCH.date()).days
    fraction = (value - value.replace(hour=0, minute=0, second=0)).total_seconds() / 86400
    return whole - fraction if whole < 0 else whole + fraction


def check(cell, value, date_result):
    if value is None:
        assert cell == 'NULL'
    elif date_result and FIRST_DAY <= int(value) <= LAST_DAY:
        assert cell == date_display('Y64:' + encoded(value)), (cell, value)
    else:
        assert encoded(float(cell)) == encoded(value), (cell, value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--database', type=Path, action='append', required=True)
    args = parser.parse_args()
    assert digest(args.jar) == JACKCESS_SHA256
    controls = 0
    dates = ['0100-01-01', '1899-12-29 06:00:00', '1899-12-30', '1900-01-01 12:00:00', '2024-02-29 18:00:00', '9999-12-31']
    for mode in ['eval', 'run']:
        for literal in dates:
            stamp = serial(literal)
            for offset in [-1, 0.5, 1]:
                sql = f'SELECT #{literal}#+({offset}) AS A,({offset})+#{literal}# AS B,#{literal}#-({offset}) AS C,({offset})-#{literal}# AS D'
                header, output = run(args.cli, 'query', mode, sql).splitlines()
                assert header == 'A\tB\tC\tD'
                for cell, expected in zip(output.split('\t'), [stamp+offset, offset+stamp, stamp-offset, offset-stamp], strict=True):
                    check(cell, expected, True)
                controls += 1
            sql = f'SELECT #{literal}#-#1899-12-30# AS Delta,-#{literal}# AS Negated,+#{literal}# AS Unchanged'
            header, output = run(args.cli, 'query', mode, sql).splitlines()
            assert header == 'Delta\tNegated\tUnchanged'
            for cell, expected, date_result in zip(output.split('\t'), [stamp, -stamp, stamp], [False, True, True], strict=True):
                check(cell, expected, date_result)
            controls += 1
        for expression in ['#2025-01-01#+NULL', 'NULL-#2025-01-01#']:
            assert run(args.cli, 'query', mode, f'SELECT {expression} AS N') == 'N\nNULL\n'
            controls += 1
        for expression in ['NULL+(#2025-01-01#+1/0)', "#2025-01-01#+'1'"]:
            result = subprocess.run([str(args.cli), 'query', mode, f'SELECT 1 AS Before,{expression} AS N'], capture_output=True)
            assert result.returncode != 0 and result.stdout == b''
            controls += 1
        header, output = run(args.cli, 'query', mode, 'SELECT #2025-01-01#*2 AS Product').splitlines()
        assert header == 'Product'
        check(output, serial('2025-01-01') * 2, False)
        controls += 1
    for path in args.database:
        before = digest(path)
        tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar, RECIPES / 'TableDataOracle.java', path, '--date-bits', 'ScalarSamples')
        records = [line.split('\t')[1:] for line in tagged.splitlines()]
        assert len(records) == 3 and all(len(row) == 14 for row in records)
        output = run(args.cli, 'query', 'run', 'SELECT ID,Stamp+0.5 AS Shifted,Stamp-#1899-12-30# AS Serial,0.5-Stamp AS Reverse,Stamp-Stamp AS Difference FROM ScalarSamples ORDER BY ID', '--database', path).splitlines()
        assert output[0] == 'ID\tShifted\tSerial\tReverse\tDifference'
        assert len(output) == len(records) + 1
        for line, row in zip(output[1:], records, strict=True):
            cells = line.split('\t')
            assert cells[0] == row[0].removeprefix('I32:')
            stamp = None if row[-1] == 'N' else struct.unpack('>d', bytes.fromhex(row[-1].removeprefix('Y64:')))[0]
            values = [None]*4 if stamp is None else [stamp+0.5, stamp, 0.5-stamp, stamp-stamp]
            for cell, value, date_result in zip(cells[1:], values, [True, False, True, False], strict=True):
                check(cell, value, date_result)
        controls += 1
        assert digest(path) == before
        print('Unchanged source', before)
    print(f'Passed {controls} DATE-serial model controls; native Access unrun.')


if __name__ == '__main__':
    main()
