"""Read-only verification of saved repeated4 outcomes; never executes fixtures."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'docs/testing'))
from repeated_split_originals import action_model

REPORT_SHA256 = 'f20b275538a9f78c4c2408e7ede8edf13605da964410f99833b20ea9b6d40ecd'
PLAN_SHA256 = 'bfc804c74c2d492c4ef71b26e9dbf8f6c6f9f1647886ea87da32cacbdbb620ed'
PROOF_SHA256 = '96ef7b9532a6ea35acf67c2a4fdeadb3c7409466afaa0d3339bf8900501eace9'


def pin(path):
    raw = path.read_bytes()
    return {'path': path.resolve().relative_to(ROOT).as_posix(), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


def canonical(value):
    return json.loads(json.dumps(value))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--proof', type=Path, required=True)
    args = parser.parse_args()
    assert pin(args.report)['sha256'] == REPORT_SHA256
    assert pin(args.plan)['sha256'] == PLAN_SHA256
    assert pin(args.proof)['sha256'] == PROOF_SHA256
    report = json.loads(args.report.read_text())
    plan = json.loads(args.plan.read_text())
    specification = importlib.util.spec_from_file_location('accepted_repeated_literal_proof', args.proof)
    proof = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(proof)
    assert report['completed'] == 4 and report['unmet'] == 0
    assert report['inputs_unchanged'] and report['all_commands_successful']
    assert report['pins_before'] == report['pins_after'] and report['pin_errors'] == []
    live_checks = 0

    def check_pins(value):
        nonlocal live_checks
        if isinstance(value, dict):
            if {'path', 'bytes', 'sha256'} <= value.keys():
                assert pin(ROOT / value['path']) == value
                live_checks += 1
            for name, item in value.items():
                if name != 'compiled_source_pins':
                    check_pins(item)
        elif isinstance(value, list):
            for item in value:
                check_pins(item)

    check_pins(report)
    git_checks = 0
    for expected in report['compiled_source_pins']:
        raw = subprocess.run(
            ['git', 'show', report['binding']['compiled_revision'] + ':' + expected['path']],
            cwd=ROOT, check=True, capture_output=True, timeout=30,
        ).stdout
        assert len(raw) == expected['bytes']
        assert hashlib.sha256(raw).hexdigest() == expected['sha256']
        git_checks += 1
    profiles = {
        profile['profile']: profile
        for profile in json.loads((ROOT / plan['source_report']['path']).read_text())['profiles']
    }
    cases = {case['id']: case for case in plan['cases']}
    assert len(report['commands']) == 8
    for command in report['commands']:
        assert command['status'] == 0 and command['stopped_for'] is None
    for result in report['cases']:
        case = cases[result['id']]
        assert result['agrees']
        original = ROOT / case['source']['path']
        candidate = ROOT / result['candidate']['path']
        expected, facts = proof.expected_image(case, original.read_bytes())
        assert facts == case['expected_image'] == result['whole_image_proof']
        assert candidate.read_bytes() == expected
        getter = result['getter']
        text = (ROOT / getter['stdout']['path']).read_text()
        observed = action_model.analyze(original, candidate, profiles[case['profile']], case['table'], text)
        assert canonical(observed) == result['typed_index_map_analysis']
    print(json.dumps({
        'scope': 'Saved bytes/getters only; zero native/JVM fixture calls',
        'report': pin(args.report), 'cases': 4, 'saved_commands': 8,
        'recursive_live_pin_occurrences': live_checks,
        'historical_git_objects': git_checks, 'complete_images': 4,
        'cached_complete_getters': 4, 'unmet': 0,
    }, indent=2))


if __name__ == '__main__':
    main()
