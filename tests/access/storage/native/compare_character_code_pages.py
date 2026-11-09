"""Check explicit single-byte character pages against independent Python codecs.

The selected contract uses WHATWG mappings. Undefined C1 bytes map to their
controls, and Windows-1255 byte CA maps to U+05BA (WHATWG index position 74),
which differ from older strict Python/Windows mapping tables. These extensions
are checked explicitly, not inferred from TableCraft output. No tables are copied.
"""
import argparse
from pathlib import Path
import subprocess

from compare_browsing import RECIPES, display, run
from compare_large_tables import digest

PAGES = [866, 874, *range(1250, 1259)]


def decode(page, byte):
    if page == 1255 and byte == 0xCA:
        return '\u05ba'
    try:
        return bytes([byte]).decode(f'cp{page}')
    except UnicodeDecodeError:
        if 0x80 <= byte <= 0x9F:
            return chr(byte)
        return None


def spelling(value):
    return display('T'+value.encode('utf-8').hex())


def refused(cli, mode, expression, options=(), error=None):
    result = subprocess.run([str(cli), 'query', mode, 'SELECT 1,'+expression, *options], capture_output=True)
    assert result.returncode != 0 and not result.stdout, (expression, options)
    if error:
        assert error in result.stderr, result.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--database', type=Path, required=True)
    args = parser.parse_args()
    assert digest(args.jar) == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    before = digest(args.database)
    controls = 0
    unmapped = 0
    for mode in ['eval', 'run']:
        for page in PAGES:
            valid = []
            options = ['--character-code-page', str(page)]
            for byte in range(256):
                value = decode(page, byte)
                if value is None:
                    refused(args.cli, mode, f'String(1,{byte})', options, b'character byte is not mapped')
                    controls += 1
                    unmapped += 1
                else:
                    valid.append((byte, value))
            for start in range(0, len(valid), 32):
                batch = valid[start:start+32]
                names = [f'C{i}' for i in range(len(batch))]
                sql = 'SELECT '+','.join(f'String(1,{byte}) AS {name}' for name, (byte,_) in zip(names,batch))
                expected = '\t'.join(names)+'\n'+'\t'.join(spelling(value) for _,value in batch)+'\n'
                actual = run(args.cli, 'query', mode, sql, *options)
                assert actual == expected, (mode, page, batch, actual, expected)
                controls += len(batch)
            expected = 'C\tM\tT\n'+'\t'.join([spelling(decode(page,192)*2),spelling(decode(page,192)),spelling('éé')])+'\n'
            assert run(args.cli, 'query', mode, "SELECT String(2,192) AS C,String(1,448) AS M,String(2,'é') AS T", *options) == expected
            controls += 1
        refused(args.cli, mode, 'String(1,128)', error=b'character code requires an explicit code-page policy')
        controls += 1
        for page in ['0','437','932','65001','99999','bogus']:
            refused(args.cli, mode, 'String(1,128)', ['--character-code-page',page], b'supported single-byte code page')
            controls += 1
    tagged = run('java', '-Dstdout.encoding=UTF-8', '-cp', args.jar,
                 RECIPES/'TableDataOracle.java', args.database, 'ROULETTE_TB')
    identities = sorted(int(line.split('\t')[1].removeprefix('I32:')) for line in tagged.splitlines())
    for page in [1251,1252]:
        expected = 'N\tC\n'+''.join(f'{identity}\t{spelling(decode(page,(128+identity)%256)*2)}\n' for identity in identities)
        sql = 'SELECT ORDR_NBR AS N,String(2,128+ORDR_NBR) AS C FROM ROULETTE_TB ORDER BY N'
        for options in [[], ['--index','ROULETTE_TB','Number']]:
            assert run(args.cli, 'query', 'run', sql, '--database', args.database,
                       '--character-code-page', str(page), *options) == expected
            controls += 1
    assert digest(args.database) == before
    print(f'Passed {controls} character-code controls ({unmapped} unmapped-byte refusals); unchanged source {before}; native Access unrun.')


if __name__ == '__main__':
    main()
