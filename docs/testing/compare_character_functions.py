"""Verify explicit Chr/Asc pages and UTF-16 functions with independent models.

Uses the prior Python-codec/WHATWG model, not TableCraft output, to construct
inverse byte mappings. Native database rows supply input only; Access is unrun.
"""
import argparse
from pathlib import Path
import struct
import subprocess

from compare_browsing import RECIPES, display, run
from compare_character_code_pages import PAGES, decode
from compare_large_tables import digest
from compare_string_search import quoted


def spelling(value):
    return display('T'+value.encode('utf-8').hex())


def refused(cli, mode, expression, options=()):
    result = subprocess.run([str(cli), 'query', mode, 'SELECT 1,'+expression, *options], capture_output=True)
    assert result.returncode != 0 and not result.stdout, (expression, options)


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
        for page in PAGES:
            options = ['--character-code-page', str(page)]
            inverse = {}
            for byte in range(256):
                value = decode(page, byte)
                if value is not None:
                    inverse.setdefault(value, byte)
            valid = []
            for byte in range(256):
                value = decode(page, byte)
                if value is None:
                    refused(args.cli, mode, f'Chr({byte})', options)
                    controls += 1
                else:
                    valid.append((byte, value))
            for start in range(0, len(valid), 16):
                batch = valid[start:start+16]
                projections, names, expected = [], [], []
                for i,(byte,value) in enumerate(batch):
                    names.extend([f'T{i}', f'C{i}'])
                    projections.extend([f'Chr({byte}) AS T{i}', f'Asc(Chr({byte})) AS C{i}'])
                    expected.extend([spelling(value), str(inverse[value])])
                sql = 'SELECT '+','.join(projections)
                assert run(args.cli, 'query', mode, sql, *options) == '\t'.join(names)+'\n'+'\t'.join(expected)+'\n', (mode,page,batch)
                controls += len(batch)*2
        for code in [0,10,65,8364,32767,32768,40000,55295,57344,65535,-1,-32768]:
            unit = code & 0xFFFF
            character = struct.pack('>H',unit).decode('utf-16-be')
            signed = struct.unpack('>h',struct.pack('>H',unit))[0]
            assert run(args.cli, 'query', mode, f'SELECT ChrW({code}) AS T,AscW(ChrW({code})) AS C') == f'T\tC\n{spelling(character)}\t{signed}\n'
            controls += 2
        for value in ['Apple','é中','😀x','𐀀']:
            signed = struct.unpack('>h',value.encode('utf-16-be')[:2])[0]
            assert run(args.cli, 'query', mode, 'SELECT AscW('+quoted(value)+') AS C') == f'C\n{signed}\n'
            controls += 1
        for expression in ['Chr(256)','Chr(-1)','Chr(NULL)',"Asc('')",'Asc(NULL)',
                           'ChrW(55296)','ChrW(57343)','ChrW(-32769)','ChrW(65536)',
                           'ChrW(NULL)','AscW(NULL)','Chr(65,1/0)',"Asc('é')"]:
            refused(args.cli, mode, expression)
            controls += 1
        refused(args.cli,mode,"Asc('中')",['--character-code-page','1252'])
        controls += 1
    tagged = run('java','-Dstdout.encoding=UTF-8','-cp',args.jar,RECIPES/'TableDataOracle.java',args.database,'ROULETTE_TB')
    rows = []
    for line in tagged.splitlines():
        row = line.split('\t')[1:]
        identity = int(row[0].removeprefix('I32:'))
        value = bytes.fromhex(row[1].removeprefix('T')).decode('utf-8')
        first = struct.unpack('>h',value.encode('utf-16-be')[:2])[0]
        rows.append((identity,first,spelling(chr(identity+65))))
    expected = 'N\tA\tC\n'+''.join(f'{n}\t{first}\t{character}\n' for n,first,character in sorted(rows))
    sql = 'SELECT ORDR_NBR AS N,AscW(NBR_VALUE) AS A,Chr(65+ORDR_NBR) AS C FROM ROULETTE_TB ORDER BY N'
    for options in [[],['--index','ROULETTE_TB','Number']]:
        assert run(args.cli,'query','run',sql,'--database',args.database,*options) == expected
        controls += 1
    assert digest(args.database) == before
    print(f'Passed {controls} character-function controls; unchanged source {before}; native Access unrun.')


if __name__ == '__main__':
    main()
