"""Recheck saved source/reference facts and framing without fixture processes."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from repeated_split_originals import action_model, framing, model


def pin(path):
    path = (ROOT / path).resolve()
    path.relative_to(ROOT)
    data = path.read_bytes()
    return {'path': path.relative_to(ROOT).as_posix(), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def verify_pins(value):
    total = 0
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            assert pin(value['path']) == {key: value[key] for key in ['path', 'bytes', 'sha256']}, value['path']
            total += 1
        for child in value.values():
            total += verify_pins(child)
    elif isinstance(value, list):
        total += sum(verify_pins(child) for child in value)
    return total


def normalize(value):
    return json.loads(json.dumps(value))


def stdout_for(report, label):
    selected = [command for command in report['commands'] if command['label'] == label]
    assert len(selected) == 1
    return (ROOT / selected[0]['stdout']['path']).read_text()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--source-report', type=Path, required=True)
    parser.add_argument('--action-report', type=Path, required=True)
    parser.add_argument('--framing', type=Path, required=True)
    parser.add_argument('--source-postcheck', type=Path, required=True)
    parser.add_argument('--action-postcheck', type=Path, required=True)
    args = parser.parse_args()
    inputs = [args.source_report, args.action_report, args.framing,
              args.source_postcheck, args.action_postcheck]
    pins_before = [pin(path) for path in inputs]
    source = json.loads(args.source_report.read_text())
    action = json.loads(args.action_report.read_text())
    frames = json.loads(args.framing.read_text())
    postchecks = [json.loads(path.read_text()) for path in inputs[-2:]]
    total = sum(verify_pins(value) for value in [source, action, frames, *postchecks])
    assert source['status'] == 'all source gates and four exact no-op admissions passed'
    assert len(source['commands']) == 9 and len(source['profiles']) == 2
    assert action['status'] == 'all four fixed-action groups completed'
    assert len(action['commands']) == 13 and len(action['cases']) == 4
    assert action['prior_split_facade_attempts'] == action['public_update_calls'] == 4
    originals = {}
    noops = 0
    for original in source['profiles']:
        profile = original['profile']
        facts = model.certify(ROOT / original['original']['path'], profile,
                              stdout_for(source, profile + '-describe'))
        assert normalize(facts) == original['facts']
        originals[profile] = original
        for noop in original['noops']:
            assert (ROOT / noop['output']['path']).read_bytes() == (ROOT / original['original']['path']).read_bytes()
            noops += 1
    assert noops == 4
    for case in action['cases']:
        assert case['native_attempt']['indexed_column_refusal_observed']
        facts = action_model.analyze(ROOT / case['source']['path'], ROOT / case['reference']['path'],
                                     originals[case['profile']], case['target'],
                                     stdout_for(action, case['profile'] + '-' + case['target'] + '-describe'))
        assert normalize(facts) == case['analysis']
        assert case['source_after'] == case['source']
    assert normalize(framing.framing(args.action_report)) == frames
    assert [pin(path) for path in inputs] == pins_before
    print(json.dumps({'source_commands': 9, 'reference_commands': 13, 'byte_identical_noops': 4,
                      'prior_refusals': 4, 'public_updates': 4, 'recursive_pin_occurrences': total,
                      'fixture_processes_repeated': 0, 'saved_facts_unchanged': True}))


if __name__ == '__main__':
    main()
