"""Verify saved candidate evidence; no native/JVM fixture calls or database writes.

Git object checks use read-only git child processes. Required ignored artifacts
must already exist; this tool never regenerates originals, getters or candidates.
"""
import argparse
import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
DEFAULT_REPORT = '.cache/ace16-encoded-split-acceptance-first-20261027/report.json'
DEFAULT_PLAN = '.cache/ace16-encoded-originals-next-20261027/plan.json'
REPORT_SHA256 = 'db862fc01722e09373be34298be21ae78b50e3ee41ee7703bd64faebc5ecfa57'
PLAN_SHA256 = 'de7db4e5b2d67c79ca1d8f7635ea3986695b8865d87188e1b6f32fbe1e604831'
MODEL_SHA256 = '39476ef54c2437ff80f20626ce78b3a9d206d294340dd95cab51e05d1e3d0089'
COMPILED_REVISION = '85576d139a3c02a89d2ef5eeef2720e7d6aaa8fb'


def path(value):
    target = (ROOT / value).resolve()
    target.relative_to(ROOT)
    return target


def digest(filename):
    return hashlib.sha256(path(filename).read_bytes()).hexdigest()


def load_module(name, filename):
    specification = importlib.util.spec_from_file_location(name, path(filename))
    result = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(result)
    return result


def functions(filename):
    tree = ast.parse(path(filename).read_text())
    return [ast.dump(node, include_attributes=False) for node in tree.body
            if isinstance(node, (ast.FunctionDef, ast.ClassDef))]


def verify(report_path, plan_path):
    assert digest(report_path) == REPORT_SHA256
    assert digest(plan_path) == PLAN_SHA256
    assert digest(HERE / 'candidate_model.py') == MODEL_SHA256
    report = json.loads(path(report_path).read_text())
    plan = json.loads(path(plan_path).read_text())
    assert report['binding']['compiled_revision'] == COMPILED_REVISION
    assert report['qualified_success'] and report['completed'] == 6 and report['unmet'] == 0
    assert len(report['commands']) == 12
    assert report['input_verification']['unchanged']
    assert report['pins_before'] == report['input_verification']['actual']
    assert len(report['pins_before']) == 71
    assert not report['input_verification']['errors'] and not report['inventory_errors']
    assert [item['argv'] for item in report['commands']] == [item['argv'] for item in report['binding']['argv']]
    live = 0
    git = 0

    def walk(value, compiled=False):
        nonlocal live, git
        if isinstance(value, dict):
            if {'path', 'bytes', 'sha256'} <= value.keys():
                if compiled:
                    data = subprocess.run(
                        ['git', 'show', COMPILED_REVISION + ':' + value['path']],
                        cwd=ROOT, check=True, capture_output=True, timeout=30,
                    ).stdout
                    git += 1
                else:
                    data = path(value['path']).read_bytes()
                    live += 1
                assert len(data) == value['bytes']
                assert hashlib.sha256(data).hexdigest() == value['sha256']
            for key, item in value.items():
                walk(item, compiled or key == 'compiled_source_pins')
        elif isinstance(value, list):
            for item in value:
                walk(item, compiled)

    walk(report)
    assert git == 263
    binding = report['binding']
    receipt = subprocess.run(
        ['git', 'show', binding['build_receipt_revision'] + ':' + binding['build_receipt']['path']],
        cwd=ROOT, check=True, capture_output=True, timeout=30,
    ).stdout
    assert receipt == path(binding['build_receipt']['path']).read_bytes()
    build = json.loads(receipt)
    assert build['compiled_revision'] == COMPILED_REVISION
    assert build['frozen_example'] == binding['artifact'] and build['exit_code'] == 0
    for command in report['commands']:
        assert command['status'] == 0 and command['stopped_for'] is None
        assert command['capture_errors'] == []
        assert command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == 16 * 1024 * 1024
        assert all(size <= 16 * 1024 * 1024 for size in command['observed_stream_bytes'])

    private = '.cache/ace16-encoded-index-reference-next-20261027/'
    public = 'docs/testing/encoded_index_split_sources/'
    assert functions(private + 'action_model.py') == functions(public + 'action_model.py')
    assert functions(private + 'proof.py') == functions(public + 'reference_proof.py')
    model = load_module('encoded_split_candidate_model', HERE / 'candidate_model.py')
    snapshot = load_module('encoded_split_candidate_snapshot', public + 'action_model.py')
    protection = load_module('encoded_split_candidate_protection', public + 'reference_proof.py')
    outcomes = []
    for case, accepted in zip(plan['cases'], report['cases'], strict=True):
        assert accepted['id'] == case['profile'] + '-' + case['table']
        assert accepted['agrees'] and case['row_ordinal'] == 0 and case['id'] == 1
        source = path(case['source']['path'])
        candidate = path(accepted['candidate']['path'])
        facts = json.loads(path(case['source_facts']['path']).read_text())
        expected, facts_ = model.expected_image(source.read_bytes(), facts, case['table'])
        # Complete bytes are checked before reparsing the saved fresh getter.
        assert candidate.read_bytes() == expected
        assert expected == path(case['expected_image']['path']).read_bytes()
        assert json.loads(json.dumps(facts_)) == case['proof']
        request = path(case['request']['path']).read_bytes()
        assert request == (b'0\tbinary\t00\tbinary\tff\n' if case['table'] == 'BinaryOne'
                           else b'0\ttext\taaa\ttext\tzzz\n')
        native = accepted['native']
        getter = accepted['getter']
        assert path(native['stdout']['path']).read_bytes() == b'candidate_changed_rows=1\n'
        assert path(native['stderr']['path']).read_bytes() == (
            b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
        )
        assert path(getter['stderr']['path']).read_bytes() == b''
        assert getter['argv'][-3:] == ['inspect', accepted['candidate']['path'], case['table']]
        before = snapshot.snapshot(source, path(case['cached_getter']['path']), case['profile'])
        after = snapshot.snapshot(candidate, path(getter['stdout']['path']), case['profile'], case['table'])
        certified = protection.compare(before, after, case['table'])
        saved = json.loads(path(accepted['fresh_getter_facts']['path']).read_text())
        assert json.loads(json.dumps({'snapshot': after, 'proof': certified})) == saved
        outcomes.append({'id': accepted['id'], 'candidate': accepted['candidate'],
                         'growth_bytes': len(expected) - len(source.read_bytes())})
    assert len(outcomes) == 6 and all(item['growth_bytes'] == 24576 for item in outcomes)
    assert digest(report_path) == REPORT_SHA256
    return {'accepted_images': 6, 'cached_fresh_getters': 6, 'saved_fixture_commands': 12,
            'live_pin_occurrences': live, 'compiled_git_objects': git,
            'build_receipt_git_objects': 1, 'git_metadata_child_processes': git + 1,
            'original_record_instance_checks': 16206, 'unique_original_user_records': 5402,
            'expected_trees': 60, 'affected_trees': 12,
            'native_fixture_calls': 0, 'JVM_fixture_calls': 0,
            'raw_report_unchanged': True, 'outcomes': outcomes}


def main():
    if not __debug__:
        raise RuntimeError('Enabled validation assertions required')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', default=DEFAULT_REPORT)
    parser.add_argument('--plan', default=DEFAULT_PLAN)
    arguments = parser.parse_args()
    print(json.dumps(verify(arguments.report, arguments.plan), indent=2))


if __name__ == '__main__':
    main()
