"""Compare scalar queries over real mixed-field inputs with inert explicit getters."""
import argparse
import csv
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tempfile

from compare_browsing import display


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('jar', 'cli', 'ace2007', 'ace2016', 'ace2019'):
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args()
    controls = 0
    with tempfile.TemporaryDirectory(prefix='tablecraft-scalar-projection-') as directory:
        subprocess.run(['javac', '-cp', str(args.jar), '-d', directory, 'docs/testing/MultivalueOracle.java', 'docs/testing/ScalarProjectionOracle.java'], check=True)
        classpath = str(args.jar) + os.pathsep + directory
        for family in ('2007', '2016', '2019'):
            path = getattr(args, 'ace' + family)
            before = path.read_bytes()
            tagged = subprocess.check_output(['java', '-Dstdout.encoding=UTF-8', '-cp', classpath, 'ScalarProjectionOracle', str(path), 'Collections', 'ID', 'Notes'], text=True, encoding='utf-8')
            rows = [list(map(display, line.split('\t'))) for line in tagged.splitlines()]
            ids = [row[0] for row in rows]
            cases = [
                ('SELECT ID, Notes FROM Collections ORDER BY ID', ['ID', 'Notes'], rows),
                ('SELECT ID, Notes FROM Collections WHERE ID>=2 ORDER BY ID DESC', ['ID', 'Notes'], list(reversed([row for row in rows if int(row[0]) >= 2]))),
                ('SELECT Count(*) AS N FROM Collections', ['N'], [[str(len(rows))]]),
                ('SELECT A.ID, B.ID FROM Collections AS A LEFT JOIN Collections AS B ON A.ID=B.ID+100 ORDER BY A.ID', ['ID', 'ID'], [[value, 'NULL'] for value in ids]),
                ('SELECT A.ID, B.ID FROM Collections AS A RIGHT JOIN Collections AS B ON A.ID=B.ID+100 ORDER BY B.ID', ['ID', 'ID'], [['NULL', value] for value in ids]),
                ('SELECT A.ID, (SELECT Count(*) FROM Collections AS B WHERE B.ID=A.ID) AS N FROM Collections AS A ORDER BY A.ID', ['ID', 'N'], [[value, '1'] for value in ids]),
                ('SELECT D.* FROM (SELECT ID, Notes FROM Collections) AS D ORDER BY ID', ['ID', 'Notes'], rows),
            ]
            for sql, header, expected in cases:
                result = subprocess.run(list(map(str, [args.cli, 'query', 'run', sql, '--database', path, '--format', 'tsv'])), capture_output=True, text=True, encoding='utf-8')
                assert result.returncode == 0 and not result.stderr, result.stderr
                assert list(csv.reader(io.StringIO(result.stdout), delimiter='\t', strict=True)) == [header] + expected
                controls += 1
            for sql in ['SELECT * FROM Collections', 'SELECT C.* FROM Collections AS C', 'SELECT Files FROM Collections', 'SELECT Count(Files) FROM Collections', 'SELECT ID FROM Collections WHERE Choices IS NULL', 'SELECT Notes FROM Collections ORDER BY Files', 'SELECT missing_name FROM Collections']:
                result = subprocess.run(list(map(str, [args.cli, 'query', 'run', sql, '--database', path])), capture_output=True)
                assert result.returncode == 2 and not result.stdout
                controls += 1
            result = subprocess.run(list(map(str, [args.cli, 'query', 'run', 'SELECT ID FROM Collections WHERE ID=1', '--database', path, '--index', 'Collections', 'unqualified'])), capture_output=True)
            assert result.returncode == 2 and not result.stdout
            controls += 1
            assert path.read_bytes() == before
            print(family, len(cases), 'query comparisons; bytes unchanged', hashlib.sha256(before).hexdigest(), flush=True)
    print(controls, 'scalar projection/query/refusal controls passed; independent getters are not native Access SQL qualification')


if __name__ == '__main__':
    main()
