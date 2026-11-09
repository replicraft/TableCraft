"""Compile one inert first-party reader, observing fresh copies only.

This prepared wrapper is unexecuted until an explicit engineering batch. No SDK
mutation, native Access process, expression evaluation or source rewriting occurs.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
SDK_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
BEFORE = '6c08831dfe4d305064837d93a8d1942a1a87b691ca80e4834a130c202c72923b'
AFTER = 'b451db719c7735c626fe58bd2f3e39530a4dbb03c2bda1d39a4c5ceb26618c30'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--prediction', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--java-bin', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert not args.output.exists(), 'Retain every prior compile/read attempt'
    assert digest(args.jar.read_bytes()) == SDK_SHA256
    originals = [(args.source, BEFORE, 0), (args.prediction, AFTER, 1)]
    for path, expected, _ in originals:
        assert digest(path.read_bytes()) == expected
    args.output.mkdir(parents=True)
    classes = args.output / 'classes'
    classes.mkdir()
    report = {
        'scope': 'Inert raw hypothesis reads only; no SDK INSERT or Access Save',
        'sdk_sha256': SDK_SHA256,
        'helpers': {name: digest((HERE / name).read_bytes()) for name in
                    ['model.py', 'GappedInsertObserver.java', 'observe.py']},
        'cases': [],
    }

    def save():
        (args.output / 'observations.json').write_text(json.dumps(report, indent=2) + '\n')

    def execute(command, name):
        result = subprocess.run(command, capture_output=True, text=True)
        (args.output / (name + '.stdout')).write_text(result.stdout)
        (args.output / (name + '.stderr')).write_text(result.stderr)
        return result

    save()
    result = execute([str(args.java_bin / 'javac'), '-cp', str(args.jar), '-d', str(classes),
                      str(HERE / 'GappedInsertObserver.java')], 'compile')
    report['compile_exit'] = result.returncode
    save()
    if result.returncode != 0:
        return
    for number, (path, expected, rows) in enumerate(originals):
        copied = args.output / ('copy-' + str(number) + '.accdb')
        copied.write_bytes(path.read_bytes())
        result = execute([str(args.java_bin / 'java'), '-Xmx256m', '-cp',
                          os.pathsep.join([str(classes), str(args.jar)]),
                          'GappedInsertObserver', str(copied), str(rows)], 'read-' + str(number))
        item = {
            'role': 'before' if number == 0 else 'predicted_after',
            'expected_sha256': expected,
            'sdk_exit': result.returncode,
            'original_unchanged': digest(path.read_bytes()) == expected,
            'copy_unchanged': digest(copied.read_bytes()) == expected,
            'observed': [line.split('\t') for line in result.stdout.splitlines()],
        }
        report['cases'].append(item)
        save()
        assert item['original_unchanged'] and item['copy_unchanged']


if __name__ == '__main__':
    main()
