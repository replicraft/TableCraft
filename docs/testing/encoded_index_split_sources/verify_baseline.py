"""Recheck saved old-slot controls without running any native or child process."""
import argparse
import json

from encoded_index_split_sources.verify import load, path, pin

DEFAULT_REPORT = '.cache/ace16-encoded-old-slot-baseline-first-20261027/report.json'
DEFAULT_FREEZE = '.cache/ace16-encoded-old-slot-baseline-next-20261027/freeze.json'


def verify(report_path, freeze_path):
    report = load(report_path)
    freeze = load(freeze_path)
    assert report['qualified_success'] and report['completed']
    assert report['failure'] is None and report['inputs_unchanged']
    assert not report['inventory_errors']
    assert report['freeze'] == pin(freeze_path)
    assert freeze['scope'] == 'twelve old-slot baseline calls only'
    assert freeze['compiled_revision'] == '59c87310ad4d25fa182336640a85477f5075455c'
    assert freeze['limits'] == {
        'commands': 12,
        'timeout_seconds': 30,
        'stream_bytes': 16 * 1024 * 1024,
    }
    assert len(report['commands']) == len(report['outcomes']) == len(freeze['actions']) == 12
    expected_pairs = [(profile, table) for profile in ('V2016', 'V2019')
                      for table in ('TextLegacy', 'TextModern', 'BinaryOne')]
    actions = freeze['actions']
    assert [item['mode'] for item in actions] == ['noop'] * 6 + ['changed'] * 6
    assert [(item['profile'], item['table']) for item in actions[:6]] == expected_pairs
    assert [(item['profile'], item['table']) for item in actions[6:]] == expected_pairs

    saved = {}
    for item in freeze['inputs'] + report['created_files'] + [report['freeze']]:
        assert saved.setdefault(item['path'], item) == item
        assert pin(item['path']) == item
    assert len(report['input_checks']) == len(freeze['inputs']) + 1
    assert [item['expected'] for item in report['input_checks']] == (
        freeze['inputs'] + [report['freeze']]
    )
    assert all(item['unchanged'] and item['actual'] == item['expected']
               for item in report['input_checks'])

    for action, command, outcome in zip(actions, report['commands'], report['outcomes']):
        assert command['argv'] == action['argv'] == [
            freeze['consumer'], action['source'], action['output'],
            action['table'], 'KeyValue', action['tsv'],
        ]
        assert action['label'] == command['label'] == outcome['label']
        assert command['stopped_for'] is None and not command['capture_errors']
        assert command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == 16 * 1024 * 1024
        assert all(size <= 16 * 1024 * 1024 for size in command['observed_stream_bytes'])
        stdout = path(command['stdout']['path']).read_bytes()
        stderr = path(command['stderr']['path']).read_bytes()
        assert pin(command['stdout']['path']) == command['stdout']
        assert pin(command['stderr']['path']) == command['stderr']
        assert action['source'].endswith(action['profile'] + '-original.accdb')
        assert action['output'] == freeze['work'] + '/' + action['label'] + '.accdb'

        binary = action['table'] == 'BinaryOne'
        kind, old, changed = ('binary', '00', 'ff') if binary else ('text', 'aaa', 'zzz')
        value = old if action['mode'] == 'noop' else changed
        assert path(action['tsv']).read_bytes() == f'0\t{kind}\t{old}\t{kind}\t{value}\n'.encode()
        output = path(action['output'])
        if action['mode'] == 'noop':
            assert command['status'] == 0
            assert stdout == b'candidate_changed_rows=0\n'
            assert stderr == (
                b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
            )
            assert output.read_bytes() == path(action['source']).read_bytes()
            assert outcome['result'] == 'byte-identical noop'
            assert outcome['output'] == pin(output)
        else:
            assert command['status'] == 1
            assert stdout == b'' and stderr == b'Error: Unsupported(IndexedColumn)\n'
            assert outcome['result'] == 'IndexedColumn; no output'
            assert not output.exists()

    return {
        'verified': True,
        'fixture_processes': 0,
        'saved_identities': len(saved),
        'native_baseline_commands': 12,
        'byte_identical_noops': 6,
        'indexed_column_refusals_without_output': 6,
        'frozen_input_checks': len(report['input_checks']),
        'public_update_commands': 0,
    }


def main():
    if not __debug__:
        raise RuntimeError('Saved verification requires enabled assertions')
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', default=DEFAULT_REPORT)
    parser.add_argument('--freeze', default=DEFAULT_FREEZE)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.report, arguments.freeze), indent=2))


if __name__ == '__main__':
    main()
