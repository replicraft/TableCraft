"""Recheck saved source/no-op evidence without Java, engine calls or generation."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
import model


def check_pin(expected):
    path = (ROOT / expected['path']).resolve()
    path.relative_to(ROOT)
    data = path.read_bytes()
    assert len(data) == expected['bytes']
    assert hashlib.sha256(data).hexdigest() == expected['sha256']
    return path


def all_pins(value):
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            check_pin(value)
            yield value['path']
        for child in value.values():
            yield from all_pins(child)
    elif isinstance(value, list):
        for child in value:
            yield from all_pins(child)


def normalized(value):
    return json.loads(json.dumps(value))


def verify(path):
    raw_bytes = path.read_bytes()
    report = json.loads(raw_bytes)
    pins = list(all_pins(report))
    commands = report['commands']
    assert len(commands) == 13
    assert all(command['status'] == 0 and command['stopped_for'] is None for command in commands)
    by_label = {command['label']: command for command in commands}
    assert len(by_label) == len(commands)
    noops = 0
    for profile in report['profiles']:
        source = check_pin(profile['original'])
        describe = by_label[profile['profile'] + '-describe']
        actual = model.certify(source, profile['profile'], check_pin(describe['stdout']).read_text())
        assert normalized(actual) == profile['facts']
        for noop in profile['noops']:
            command = by_label[noop['command_label']]
            assert check_pin(command['stdout']).read_text() == 'candidate_changed_rows=0\n'
            assert 'Engineering candidate:' in check_pin(command['stderr']).read_text()
            assert check_pin(noop['output']).read_bytes() == source.read_bytes()
            assert noop['byte_identical']
            noops += 1
    assert len(report['profiles']) == 2 and noops == 8
    post_path = path.parent / 'post-execution-pins.json'
    post = json.loads(post_path.read_text())
    post_pins = list(all_pins(post))
    assert post['direct_pin_occurrences'] == 20 and post['mismatches'] == 0
    assert post['report_before'] == post['report_after']
    assert post['report_after']['sha256'] == hashlib.sha256(raw_bytes).hexdigest()
    assert all(check['matches'] and check['expected'] == check['actual'] for check in post['checks'])
    assert path.read_bytes() == raw_bytes
    return {'read_only': True, 'fixture_processes_repeated': 0, 'profiles': 2,
            'historical_fixture_processes': 13, 'byte_identical_noops': noops,
            'raw_pin_occurrences': len(pins), 'postcheck_pin_occurrences': len(post_pins),
            'raw_report_unchanged': True}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    path = (ROOT / args.report).resolve()
    path.relative_to(ROOT)
    print(json.dumps(verify(path), indent=2))


if __name__ == '__main__':
    main()
