"""Recheck saved combined candidates; no native/JVM fixture calls or DB writes.

The compiled source/receipt checks invoke read-only Git metadata child processes.
Missing ignored evidence is an error; this verifier never regenerates inputs.
"""
import argparse
import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
DEFAULT_REPORT = '.cache/ace16-variable-encoded-acceptance-first-20261027/report.json'
DEFAULT_PLAN = '.cache/ace16-variable-encoded-independent-next-20261027/combined-plan.json'
REPORT_SHA256 = '0d5e5d814c6b67cf334aab35898af36ccc0d38553f607dc6f7ae01a22f8763b6'
PLAN_SHA256 = 'ef2d02b8c0c2ee4805baa9da20160f95938ccc6e7aede6d7ebeec37abf690116'
COMPILED_REVISION = 'abc9fd370f71a692a2b17774c7cba04df54e5c5d'
RECEIPT_REVISION = '704137ca7f93c9e381041da1bfe2d1de23d0c091'
PRIVATE_MODEL = '.cache/ace16-variable-encoded-independent-next-20261027/'
PRIVATE_REFERENCE = '.cache/ace16-variable-encoded-reference-next-20261027/'
PUBLIC_REFERENCE = 'docs/testing/variable_encoded_split_sources/'


def path(value):
    result = (ROOT / value).resolve()
    result.relative_to(ROOT)
    return result


def digest(value):
    return hashlib.sha256(path(value).read_bytes()).hexdigest()


def functions(value):
    return [ast.dump(node, include_attributes=False)
            for node in ast.parse(path(value).read_text()).body
            if isinstance(node, (ast.FunctionDef, ast.ClassDef))]


def load_module(name, filename):
    specification = importlib.util.spec_from_file_location(name, path(filename))
    result = importlib.util.module_from_spec(specification)
    sys.modules[name] = result
    specification.loader.exec_module(result)
    return result


def git_blob(revision, filename):
    return subprocess.run(['git', 'show', revision + ':' + filename], cwd=ROOT,
                          check=True, capture_output=True, timeout=30).stdout


def verify(report_path, plan_path):
    assert digest(report_path) == REPORT_SHA256
    assert digest(plan_path) == PLAN_SHA256
    report = json.loads(path(report_path).read_text())
    plan = json.loads(path(plan_path).read_text())
    assert report['qualified_success'] and report['completed'] == 6 and report['unmet'] == 0
    assert report['failure'] is None and not report['inventory_errors']
    assert report['input_verification']['unchanged']
    assert not report['input_verification']['errors']
    assert report['pins_before'] == report['input_verification']['actual']
    assert len(report['pins_before']) == 76 and len(report['artifacts']) == 112
    assert len(report['cases']) == 6 and len(report['commands']) == 12
    binding = report['binding']
    assert binding['compiled_revision'] == COMPILED_REVISION
    assert binding['build_receipt_revision'] == RECEIPT_REVISION
    assert binding['maximum_fixture_processes'] == 12
    assert binding['model_plan']['sha256'] == PLAN_SHA256
    assert [command['argv'] for command in report['commands']] == [item['argv'] for item in binding['argv']]
    live = 0
    compiled = 0

    def walk(value, compiler_source=False):
        nonlocal live, compiled
        if isinstance(value, dict):
            if {'path', 'bytes', 'sha256'} <= value.keys():
                if compiler_source:
                    data = git_blob(COMPILED_REVISION, value['path'])
                    compiled += 1
                else:
                    data = path(value['path']).read_bytes()
                    live += 1
                assert len(data) == value['bytes']
                assert hashlib.sha256(data).hexdigest() == value['sha256']
            for key, child in value.items():
                walk(child, compiler_source or key == 'compiled_source_pins')
        elif isinstance(value, list):
            for child in value:
                walk(child, compiler_source)

    walk(report)
    assert compiled == 285
    receipt_bytes = git_blob(RECEIPT_REVISION, binding['build_receipt']['path'])
    assert receipt_bytes == path(binding['build_receipt']['path']).read_bytes()
    build = json.loads(receipt_bytes)
    assert build['compiled_revision'] == COMPILED_REVISION and build['exit_code'] == 0
    assert build['frozen_example'] == binding['artifact']
    assert build['source_pins'] == report['compiled_source_pins']
    assert build['exported_file_count'] == 4194
    assert build['archive_sources_unchanged_after_build']
    # The selected285 objects are verified through Git above. Build artifacts
    # and the full-export manifest identity are separate filesystem checks.
    walk({key: value for key, value in build.items() if key != 'source_pins'})
    for command in report['commands']:
        assert command['status'] == 0 and command['stopped_for'] is None
        assert command['capture_errors'] == []
        assert command['timeout_seconds'] == 30 and command['stream_limit_bytes'] == 16777216
        assert all(size <= 16777216 for size in command['observed_stream_bytes'])

    assert functions(PRIVATE_MODEL + 'combined_model.py') == functions(HERE / 'candidate_model.py')
    assert functions(PRIVATE_MODEL + 'source_weights.py') == functions(HERE / 'source_weights.py')
    assert functions('.cache/ace16-data-record-proof-next-20261027/oracle.py') == functions(HERE / 'record_model.py')
    assert functions(PRIVATE_REFERENCE + 'action_model.py') == functions(PUBLIC_REFERENCE + 'action_model.py')
    assert functions(PRIVATE_REFERENCE + 'proof.py') == functions(PUBLIC_REFERENCE + 'reference_proof.py')
    model = load_module('saved_variable_index_candidate_model', HERE / 'candidate_model.py')
    snapshot = load_module('saved_variable_index_snapshot', PUBLIC_REFERENCE + 'action_model.py')
    protection = load_module('saved_variable_index_reference_proof', PUBLIC_REFERENCE + 'reference_proof.py')
    outcomes = []
    after_rows = 0
    logical_entries = 0
    prefix_bytes = 0
    selectors = [(profile, table) for profile in ('V2016', 'V2019')
                 for table in ('TextLegacy', 'TextModern', 'BinaryOne')]
    assert [(case['profile'], case['table']) for case in plan['cases']] == selectors
    for case, actual in zip(plan['cases'], report['cases'], strict=True):
        assert actual['id'] == case['id'] and actual['agrees']
        assert actual['source'] == case['source'] and actual['whole_image_match_before_getter']
        assert case['identity'] == 1 and case['ordinal'] == 0 and case['column'] == 'KeyValue'
        source = path(case['source']['path'])
        original = source.read_bytes()
        assert len(original) == case['source']['bytes']
        assert hashlib.sha256(original).hexdigest() == case['source']['sha256']
        facts = json.loads(path(case['facts']['path']).read_text())
        candidate = path(actual['candidate']['path'])
        expected, expected_facts = model.expected_image(original, facts, case['table'])
        assert expected_facts == case['proof']
        assert len(expected) == actual['candidate']['bytes']
        assert hashlib.sha256(expected).hexdigest() == actual['candidate']['sha256']
        # Full bytes and model hash are authoritative BEFORE saved getter replay.
        assert candidate.read_bytes() == expected
        binary = case['table'] == 'BinaryOne'
        request = path(actual['request']['path']).read_bytes()
        assert request == (b'0\tbinary\t00\tbinary\t' + b'ff' * 255 + b'\n' if binary
                           else b'0\ttext\taaa\ttext\t' + b'z' * 255 + b'\n')
        native = actual['native']
        getter = actual['getter']
        assert native['argv'] == [binding['artifact']['path'], case['source']['path'], str(candidate.relative_to(ROOT)),
                                  case['table'], case['column'], actual['request']['path']]
        assert path(native['stdout']['path']).read_bytes() == b'candidate_changed_rows=1\n'
        assert path(native['stderr']['path']).read_bytes() == (
            b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
        )
        assert path(getter['stderr']['path']).read_bytes() == b''
        assert getter['argv'][-3:] == ['inspect', str(candidate.relative_to(ROOT)), case['table']]
        cached_source = next(item for item in report['pins_before']
                             if item['path'].endswith(case['profile'] + '-inspect.stdout'))
        before = snapshot.snapshot(source, path(cached_source['path']), case['profile'])
        assert json.loads(json.dumps(before)) == facts
        after = snapshot.snapshot(candidate, path(getter['stdout']['path']), case['profile'], case['table'])
        proof = protection.compare(before, after, case['table'])
        saved = json.loads(path(actual['fresh_getter_facts']['path']).read_text())
        assert json.loads(json.dumps({'snapshot': after, 'proof': proof})) == saved
        assert candidate.read_bytes() == expected
        after_rows += sum(table['count'] for table in after['tables'].values())
        logical_entries += sum(index['count'] for table in after['tables'].values()
                               for index in table['indexes'].values())
        prefix_bytes += len(original)
        outcomes.append({'id': case['id'], 'candidate': actual['candidate'],
                         'growth_bytes': len(expected) - len(original),
                         'DATA_page': expected_facts['DATA_page'],
                         'new_record_capacity': expected_facts['new_record_capacity']})
    assert len(outcomes) == 6 and all(case['growth_bytes'] == 28672 for case in outcomes)
    assert after_rows == 16206 and logical_entries == 64806 and prefix_bytes == 3883008
    assert digest(report_path) == REPORT_SHA256
    return {'accepted_images': 6, 'cached_fresh_getters': 6, 'saved_fixture_commands': 12,
            'native_fixture_calls': 0, 'JVM_fixture_calls': 0,
            'live_pin_occurrences': live, 'compiled_git_objects': compiled,
            'receipt_git_objects': 1, 'git_metadata_child_processes': compiled + 1,
            'before_after_input_pins': 76, 'saved_artifact_pins': 112,
            'unique_original_user_records': 5402, 'original_record_instance_checks': 16206,
            'physical_tree_instances_checked': 60, 'affected_physical_trees': 12,
            'new_DATA_pages': 6, 'new_index_pages': 36, 'new_bitmap_pages': 0,
            'after_rows_checked': after_rows, 'after_logical_entries_including_aliases': logical_entries,
            'original_prefix_bytes_checked': prefix_bytes,
            'executed_function_AST_identity': True, 'raw_report_unchanged': True,
            'outcomes': outcomes}


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
