"""Exact four-group reference phase, v2 actual-call counters; explicit execute and reviewed freeze required."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
BASE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'docs/testing'))
from integral_split_originals.capture_sources import Capture, pin, require_pin

JAVA_HEAP = '512m'
TARGETS = [
    ('Int64Keys', 'int64', (1 << 63) - 1),
    ('CurrencyKeys', 'currency-scaled', (1 << 63) - 1),
]


def inventory(freeze):
    work = freeze['work']
    classes = work + '/classes'
    classpath = os.pathsep.join([freeze['jar']['path'], classes])
    java = ['java', '-Xmx' + JAVA_HEAP, '-Dstdout.encoding=UTF-8', '-cp', classpath]
    commands = [{'label': 'compile', 'argv': ['javac', '-J-Xmx' + JAVA_HEAP,
                 '-encoding', 'UTF-8', '-cp', freeze['jar']['path'], '-d', classes,
                 *freeze['java_sources']]}]
    for original in freeze['originals']:
        profile = original['profile']
        source = original['input']['path']
        for table, kind, maximum in TARGETS:
            stem = profile + '-' + table
            row = str(original['selectors'][table]['original_ordinal'])
            commands.extend([
                {'label': stem + '-native', 'require_success': False,
                 'argv': [freeze['baseline']['path'], source, work + '/' + stem + '-native.accdb',
                          table, 'KeyValue', row, kind, '0', str(maximum)]},
                {'label': stem + '-public', 'require_success': True,
                 'argv': [*java, 'RepeatedSplitUpdateReference', source,
                          work + '/' + stem + '-reference.accdb', table]},
                {'label': stem + '-describe', 'require_success': True,
                 'argv': [*java, 'RepeatedSplitFixture', 'describe',
                          work + '/' + stem + '-reference.accdb']},
            ])
    assert len(commands) == 13
    return commands


def record(capture, command):
    try:
        capture.run(command['argv'], command['label'])
    except AssertionError:
        # The reused recorder asserts success after saving status/stdout/stderr.
        # Only old-service negative exits are expected here. Never consume a
        # timeout/output-limit failure or an assertion before a record exists.
        if command.get('require_success', True) or not capture.records:
            raise
        latest = capture.records[-1]
        if latest['label'] != command['label'] or latest['stopped_for'] is not None:
            raise
        if latest['status'] is None or latest['status'] == 0:
            raise
    return capture.records[-1]


def counted(capture, command, report, counter):
    previous = len(capture.records)
    try:
        return record(capture, command)
    finally:
        # A recorder entry exists for each actual completed/stopped subprocess,
        # including a public call whose unexpected failure then propagates.
        report[counter] += len(capture.records) - previous


def validate(freeze):
    require_pin(freeze['execution_driver'])
    assert (ROOT / freeze['execution_driver']['path']).resolve() == Path(__file__).resolve()
    for original in freeze['derived_from']:
        require_pin(original)
    for expected in freeze['sources']:
        require_pin(expected)
    for expected in [freeze['jar'], freeze['baseline'], freeze['baseline_receipt'],
                     freeze['source_report'], freeze['source_postcheck'], freeze['source_freeze']]:
        require_pin(expected)
    for original in freeze['originals']:
        require_pin(original['input'])
    for expected in freeze['static_checks']:
        require_pin(expected)
    source_report = json.loads((ROOT / freeze['source_report']['path']).read_text())
    assert len(source_report['profiles']) == len(freeze['originals']) == 2
    for prepared, recorded in zip(freeze['originals'], source_report['profiles'], strict=True):
        assert prepared['profile'] == recorded['profile']
        assert prepared['input'] == recorded['original']
        for table, kind, _ in TARGETS:
            actual = recorded['facts']['tables'][table]
            selected = prepared['selectors'][table]
            assert selected == {'identity': 1, 'original_ordinal': actual['selected_ordinal'],
                                'original_reference': actual['selected_reference'],
                                'expected_units': 0, 'kind': kind}
    receipt = json.loads((ROOT / freeze['baseline_receipt']['path']).read_text())
    assert receipt['compiled_revision'] == freeze['compiled_revision']
    assert receipt['frozen_example'] == freeze['baseline'] and receipt['exit_code'] == 0
    assert freeze['jar']['sha256'] == '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
    for source in receipt['source_pins']:
        data = subprocess.check_output(['git', 'show', freeze['compiled_revision'] + ':' + source['path']], cwd=ROOT)
        assert len(data) == source['bytes'] and hashlib.sha256(data).hexdigest() == source['sha256']
    assert inventory(freeze) == freeze['process_inventory']
    assert not (ROOT / freeze['work']).exists()


def execute(freeze):
    from repeated_split_originals import action_model as model

    work = (ROOT / freeze['work']).resolve()
    work.relative_to(ROOT / '.cache')
    work.mkdir()
    (work / 'classes').mkdir()
    archive = work / 'precall-sources'
    archive.mkdir()
    for position, expected in enumerate(freeze['sources']):
        data = (ROOT / expected['path']).read_bytes()
        (archive / f'{position:02d}-{Path(expected["path"]).name}').write_bytes(data)
    (work / 'freeze.json').write_text(json.dumps(freeze, indent=2) + '\n')
    sources = json.loads((ROOT / freeze['source_report']['path']).read_text())
    capture = Capture(work)
    report = {'scope': freeze['scope'], 'commands': capture.records, 'cases': [],
              'generation_calls': 0, 'prior_split_facade_attempts': 0, 'public_update_calls': 0}
    try:
        commands = inventory(freeze)
        record(capture, commands[0])
        report['classes'] = [pin(path) for path in sorted((work / 'classes').rglob('*.class'))]
        for position in range(4):
            native, public, describe = commands[1 + position * 3:1 + (position + 1) * 3]
            group, target_index = divmod(position, 2)
            original = sources['profiles'][group]
            target = TARGETS[target_index][0]
            original_pin = freeze['originals'][group]['input']
            source = ROOT / original_pin['path']
            case = {'profile': original['profile'], 'target': target,
                    'source': original_pin, 'expected_typed_max_units': TARGETS[target_index][2]}
            report['cases'].append(case)
            try:
                actual = counted(capture, native, report, 'prior_split_facade_attempts')
                candidate = ROOT / native['argv'][2]
                stderr = (ROOT / actual['stderr']['path']).read_text()
                stdout = (ROOT / actual['stdout']['path']).read_text()
                case['native_attempt'] = {
                    'command_label': actual['label'], 'status': actual['status'],
                    'stderr': actual['stderr'], 'stdout': actual['stdout'],
                    'candidate': pin(candidate) if candidate.exists() else None,
                    'indexed_column_refusal_observed': actual['status'] != 0
                        and 'Unsupported(IndexedColumn)' in stderr and not stdout and not candidate.exists(),
                }
                require_pin(original_pin)
                counted(capture, public, report, 'public_update_calls')
                require_pin(original_pin)
                actual = record(capture, describe)
                reference = ROOT / public['argv'][-2]
                case['reference'] = pin(reference)
                case['analysis'] = model.analyze(source, reference, original, target,
                                                (ROOT / actual['stdout']['path']).read_text())
            finally:
                case['source_after'] = pin(source)
                assert case['source_after'] == original_pin
        assert len(capture.records) == 13 and len(report['cases']) == 4
        assert report['prior_split_facade_attempts'] == 4
        assert report['public_update_calls'] == 4
        report['status'] = 'all four fixed-action groups completed'
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
    parser.add_argument('--freeze', type=Path, default=BASE / 'action-plan.json')
    parser.add_argument('--execute', action='store_true')
    args = parser.parse_args()
    freeze = json.loads(args.freeze.read_text())
    validate(freeze)
    if args.execute:
        execute(freeze)
    else:
        print('Static action sources/inputs/artifact/Git/inventory passed; no JVM/native/actions.')


if __name__ == '__main__':
    main()
