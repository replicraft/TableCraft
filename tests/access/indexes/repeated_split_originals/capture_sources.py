"""Frozen finite source preparation; explicit execute is required for processes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

REPO = Path(__file__).resolve().parents[3]
BASE = Path(__file__).resolve().parent
TIMEOUT_SECONDS = 120
POLL_SECONDS = 0.05
MAX_STREAM_BYTES = 16 * 1024 * 1024
JAVA_HEAP = "512m"
REVIEWED_JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
RECEIPT = 'docs/testing/results/ace16-fixed-split-example-build-2026-10-27.json'


def repository_path(value):
    path = (REPO / value).resolve()
    path.relative_to(REPO)
    return path


def pin(value):
    path = repository_path(value)
    data = path.read_bytes()
    return {'path': path.relative_to(REPO).as_posix(), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def require_pin(expected):
    assert pin(expected['path']) == expected, expected['path']


def command_records(freeze):
    work = freeze['work']
    classes = work + '/classes'
    classpath = os.pathsep.join([freeze['jar']['path'], classes])
    java = ['java', '-Xmx' + JAVA_HEAP, '-Dstdout.encoding=UTF-8', '-cp', classpath, 'RepeatedSplitFixture']
    commands = [{'label': 'compile', 'argv': ['javac', '-J-Xmx' + JAVA_HEAP, '-encoding', 'UTF-8', '-cp',
                 freeze['jar']['path'], '-d', classes, *freeze['java_sources']]}]
    for profile in ['V2016', 'V2019']:
        source = f'{work}/{profile}-before.accdb'
        commands.append({'label': profile + '-generate',
                         'argv': [*java, 'generate', source, profile]})
        commands.append({'label': profile + '-describe', 'argv': [*java, 'describe', source]})
        for table, kind in [('Int64Keys', 'int64'), ('CurrencyKeys', 'currency-scaled')]:
            commands.append({'label': profile + '-' + table + '-noop',
                             'argv_template': [freeze['baseline']['path'], source,
                             f'{work}/{profile}-{table}-noop.accdb', table, 'KeyValue',
                             '<measured original ID1 ordinal>', kind, '0', '0']})
    assert len(commands) == 9
    return commands


class Capture:
    def __init__(self, work):
        self.work = work
        self.records = []

    def run(self, argv, label):
        stdout = self.work / (label + '.stdout')
        stderr = self.work / (label + '.stderr')
        print('Starting fixture process:', label, flush=True)
        with stdout.open('xb') as out, stderr.open('xb') as err:
            process = subprocess.Popen(argv, cwd=REPO, stdout=out, stderr=err)
            deadline = time.monotonic() + TIMEOUT_SECONDS
            reason = None
            while process.poll() is None:
                if time.monotonic() >= deadline:
                    reason = 'timeout'
                if max(stdout.stat().st_size, stderr.stat().st_size) > MAX_STREAM_BYTES:
                    reason = 'output ceiling'
                if reason:
                    process.kill()
                    try:
                        process.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        reason += '; killed process did not reap within ten seconds'
                    break
                time.sleep(POLL_SECONDS)
        print('Completed fixture process:', label, 'status', process.returncode, flush=True)
        result = {'label': label, 'argv': argv, 'status': process.returncode,
                  'stopped_for': reason, 'stdout': pin(stdout), 'stderr': pin(stderr)}
        self.records.append(result)
        with (self.work / 'commands.jsonl').open('a') as output:
            output.write(json.dumps(result) + '\n')
        assert result['status'] == 0 and reason is None, result
        return stdout


def validate(freeze):
    require_pin(freeze['execution_driver'])
    assert repository_path(freeze['execution_driver']['path']) == Path(__file__).resolve()
    for source in freeze['sources']:
        require_pin(source)
    assert freeze['jar']['sha256'] == REVIEWED_JAR_SHA256
    require_pin(freeze['jar'])
    require_pin(freeze['baseline'])
    require_pin(freeze['baseline_receipt'])
    receipt = json.loads(repository_path(RECEIPT).read_text())
    assert receipt['compiled_revision'] == freeze['compiled_revision']
    assert receipt['frozen_example'] == freeze['baseline'] and receipt['exit_code'] == 0
    for source in receipt['source_pins']:
        expected = source['sha256']
        data = subprocess.check_output(['git', 'show', freeze['compiled_revision'] + ':' + source['path']], cwd=REPO)
        assert len(data) == source['bytes'] and hashlib.sha256(data).hexdigest() == expected
    require_pin(freeze['prior_typed_acceptance'])
    acceptance = json.loads(repository_path(freeze['prior_typed_acceptance']['path']).read_text())
    assert acceptance['completed'] == 8 and acceptance['unmet'] == 0
    assert acceptance['inputs_unchanged'] and len(acceptance['commands']) == 16
    assert command_records(freeze) == freeze['process_inventory']


def execute(freeze):
    sys.path.insert(0, str(REPO / 'docs/testing'))
    from repeated_split_originals import model

    work = repository_path(freeze['work'])
    work.relative_to(REPO / '.cache')
    assert not work.exists(), 'Use the exact fresh frozen work directory'
    work.mkdir()
    (work / 'classes').mkdir()
    archive = work / 'precall-sources'
    archive.mkdir()
    for position, source in enumerate(freeze['sources']):
        data = repository_path(source['path']).read_bytes()
        (archive / f'{position:02d}-{Path(source["path"]).name}').write_bytes(data)
    (work / 'freeze.json').write_text(json.dumps(freeze, indent=2) + '\n')
    capture = Capture(work)
    report = {'scope': freeze['scope'], 'commands': capture.records, 'profiles': [],
              'native_noop_attempts': 0, 'native_non_noop_calls': 0, 'public_update_calls': 0}
    try:
        commands = command_records(freeze)
        capture.run(commands[0]['argv'], commands[0]['label'])
        report['classes'] = [pin(path) for path in sorted((work / 'classes').rglob('*.class'))]
        for group, profile in enumerate(['V2016', 'V2019']):
            first = 1 + group * 4
            generate = commands[first]
            describe = commands[first + 1]
            capture.run(generate['argv'], generate['label'])
            source = repository_path(generate['argv'][-2])
            original = pin(source)
            description = capture.run(describe['argv'], describe['label'])
            assert pin(source) == original
            facts = model.certify(source, profile, description.read_text())
            entry = {'profile': profile, 'original': original, 'facts': facts, 'noops': []}
            report['profiles'].append(entry)
            for plan in commands[first + 2:first + 4]:
                argv = list(plan['argv_template'])
                table = argv[3]
                argv[5] = str(facts['tables'][table]['selected_ordinal'])
                previous = len(capture.records)
                try:
                    stdout = capture.run(argv, plan['label'])
                finally:
                    report['native_noop_attempts'] += len(capture.records) - previous
                assert stdout.read_text() == 'candidate_changed_rows=0\n'
                assert 'Engineering candidate:' in (work / (plan['label'] + '.stderr')).read_text()
                output = repository_path(argv[2])
                assert output.read_bytes() == source.read_bytes()
                assert pin(source) == original
                entry['noops'].append({'command_label': plan['label'], 'output': pin(output),
                                       'byte_identical': True})
        assert len(capture.records) == 9
        report['status'] = 'all source gates and four exact no-op admissions passed'
    except BaseException as error:
        report['status'] = 'first failure preserved'
        report['error'] = str(error)
        raise
    finally:
        report['artifacts'] = [pin(path) for path in sorted(work.rglob('*'))
                               if path.is_file() and path.name != 'report.json']
        (work / 'report.json').write_text(json.dumps(report, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--freeze', type=Path, default=BASE / 'source-plan.json')
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    freeze = json.loads(args.freeze.read_text())
    validate(freeze)
    if args.execute:
        execute(freeze)
    else:
        print('Static source/artifact/Git/inventory validation passed; no JVM/native/database actions.')


if __name__ == '__main__':
    main()
