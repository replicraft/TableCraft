"""Reconstruct saved candidates and proofs; only optional Git-object reads spawn."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
from compat_artifacts import ArtifactStore

sys.path.insert(0, str(ROOT / 'docs/testing'))
from ordinary_hint_growth_candidates import candidate_model
from ordinary_hint_growth_references import action_model, reference_proof

STORE = ArtifactStore(ROOT)

PREPARED = '.cache/ace16-hint-transition-acceptance-next-20261027'
ACTUAL = '.cache/ace16-hint-transition-acceptance-first-20261027'
MODELS = '.cache/ace16-hint-transition-model-next-20261027'
FREEZE_SHA = 'c2367630828e938c6a424d6e3b7efbb77e2f28b8145dfba33126620b3a26cdb9'
NATIVE_SHA = '19ba7d3b97e298e05ca1eec10ca38f5853422434ae195d87bd6256521c57ec96'
GETTERS_SHA = '5c3789172d2a23325c41267e669c0f05774ee8dabdd3e65c012e9171720b900e'
COMPILED = '69443c6accdd5b6c3118a5a9a31ba30c0fc4d03b'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


def normalized(value):
    return json.loads(json.dumps(value))


def committed_objects(protocol, receipt):
    queries = [protocol['receipt_revision'] + ':' + protocol['build_receipt']['path']]
    queries += [COMPILED + ':' + item['path'] for item in receipt['source_pins']]
    assert len(queries) == 296 and all('\n' not in query for query in queries)
    result = subprocess.run(['git', 'cat-file', '--batch'], cwd=ROOT,
                            input=('\n'.join(queries) + '\n').encode(),
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            check=True, timeout=30)
    expected = [protocol['build_receipt']] + receipt['source_pins']
    position = 0
    for item in expected:
        stop = result.stdout.index(b'\n', position)
        header = result.stdout[position:stop].split()
        assert len(header) == 3 and header[1] == b'blob'
        size = int(header[2])
        begin = stop + 1
        end = begin + size
        blob = result.stdout[begin:end]
        assert size == item['bytes'] and hashlib.sha256(blob).hexdigest() == item['sha256']
        assert result.stdout[end:end + 1] == b'\n'
        position = end + 1
    assert position == len(result.stdout)
    return {'compiled_Git_objects': 295, 'receipt_Git_objects': 1, 'Git_processes': 1}


def verify(verify_git):
    freeze_path = PREPARED + '/freeze.json'
    native_path = ACTUAL + '/native/report.json'
    getter_path = ACTUAL + '/getters/report.json'
    for name, sha in [(freeze_path, FREEZE_SHA), (native_path, NATIVE_SHA), (getter_path, GETTERS_SHA)]:
        assert STORE.pin(name)['sha256'] == sha
    freeze = STORE.load(freeze_path)
    protocol = STORE.load(freeze['protocol']['path'])
    assert STORE.pin(freeze['protocol']['path']) == freeze['protocol']
    assert protocol['compiled_revision'] == COMPILED
    native = STORE.load(native_path)
    getters = STORE.load(getter_path)
    pins = {}
    occurrences = 0

    def check(item):
        nonlocal occurrences
        occurrences += 1
        prior = pins.setdefault(item['path'], item)
        assert prior == item and STORE.pin(item['path']) == item

    for report, phase, expected in [(native, 'native', protocol['native_argv']),
                                    (getters, 'getters', protocol['getter_argv'])]:
        assert report['phase'] == phase and report['qualified_success']
        assert report['failure'] is None and report['completed'] == 12
        assert report['freeze'] == STORE.pin(freeze_path)
        assert report['input_postcheck']['unchanged'] and report['output_postcheck']['unchanged']
        assert all(item['matches'] for item in report['input_postcheck']['checks'])
        assert all(item['matches'] for item in report['output_postcheck']['checks'])
        assert not report['inventory_errors']
        assert len(report['commands']) == len(report['cases']) == 12
        assert [command['argv'] for command in report['commands']] == [command['argv'] for command in expected]
        for command in report['commands']:
            assert command['status'] == 0 and command['stopped_for'] is None
            assert not command['capture_errors']
            assert command['timeout_seconds'] == 30
            assert command['stream_limit_bytes'] == 16777216
        for item in report['pins_before'] + report['artifacts']:
            check(item)
    assert native['native_attempts'] == 12 and native['getter_attempts'] == 0
    assert getters['native_attempts'] == 0 and getters['getter_attempts'] == 12
    assert STORE.pin(native_path) in getters['pins_before']
    for item in freeze['inputs']:
        check(item)
    for name in (freeze_path, native_path, getter_path, PREPARED + '/post-terminal-check.json'):
        check(STORE.pin(name))
    postcheck = STORE.load(PREPARED + '/post-terminal-check.json')
    assert postcheck['unchanged'] and postcheck['fixture_processes'] == 0
    assert postcheck['raw_getter_report_sha256'] == GETTERS_SHA
    for item in postcheck['checks']:
        assert item['matches'] and item['actual'] == item['expected']
        check(item['expected'])

    public = 'docs/testing/ordinary_hint_growth_candidates/'
    source = STORE.path(MODELS + '/candidate_model.py').read_text()
    assert source.count('parents[2]') == 1
    old_import = "'.cache/ace16-hint-transition-model-next-20261027/allocation_model.py'"
    new_import = "'docs/testing/ordinary_hint_growth_candidates/allocation_model.py'"
    assert source.count(old_import) == 1
    assert STORE.path(public + 'candidate_model.py').read_text() == source.replace('parents[2]', 'parents[3]').replace(old_import, new_import)
    assert STORE.path(public + 'allocation_model.py').read_bytes() == STORE.path(MODELS + '/allocation_model.py').read_bytes()
    reference_directory = '.cache/ace16-ordinary-hint-reference-next-20261027/'
    for filename in ('action_model.py', 'reference_proof.py', 'map_proof.py'):
        raw = STORE.path(reference_directory + filename).read_text()
        maintained = raw.replace('parents[2]', 'parents[3]')
        if filename == 'reference_proof.py':
            maintained = maintained.replace('import map_proof', 'from ordinary_hint_growth_references import map_proof')
        assert STORE.path('docs/testing/ordinary_hint_growth_references/' + filename).read_text() == maintained

    receipt = STORE.load(protocol['build_receipt']['path'])
    assert receipt['compiled_revision'] == COMPILED and receipt['exit_code'] == 0
    assert receipt['frozen_example'] == protocol['artifact']
    assert receipt['source_pins'] == native['compiled_source_pins'] == getters['compiled_source_pins']
    assert receipt['exported_file_count'] == 4217
    git = committed_objects(protocol, receipt) if verify_git else {
        'compiled_Git_objects': 0, 'receipt_Git_objects': 0, 'Git_processes': 0,
    }
    model_report = STORE.load(protocol['model_report']['path'])
    assert model_report['completed'] and model_report['fixture_calls'] == model_report['native_calls'] == 0
    assert len(model_report['cases']) == 12
    original_bytes = rows = entries = physical_trees = 0
    purposes = {'DATA': 0, 'INDEX': 0, 'INDEX_BITMAP': 0, 'CANDIDATE_BITMAP': 0}
    outcomes = []
    for case, frozen, made, got, native_command, getter_command in zip(
        protocol['cases'], model_report['cases'], native['cases'], getters['cases'],
        native['commands'], getters['commands'], strict=True,
    ):
        assert case['id'] == made['id'] == got['id']
        assert made['completed'] and got['completed'] and made['whole_image_match_before_getter']
        assert STORE.path(native_command['stdout']['path']).read_bytes() == b'candidate_changed_rows=1\n'
        assert STORE.path(native_command['stderr']['path']).read_bytes() == NOTICE
        assert STORE.path(getter_command['stderr']['path']).read_bytes() == b''
        accepted = STORE.load(case['facts']['path'])['snapshot']
        expected, observations = candidate_model.expected_image(
            STORE.path(case['source']['path']).read_bytes(), accepted, case['table'],
        )
        candidate = STORE.pin(case['output'])
        assert candidate == made['candidate'] == got['candidate']
        assert expected == STORE.path(case['expected']['path']).read_bytes() == STORE.path(case['output']).read_bytes()
        assert len(expected) == case['expected']['bytes']
        assert hashlib.sha256(expected).hexdigest() == case['expected']['sha256']
        assert observations == frozen['observations']
        before = action_model.snapshot(STORE.path(case['source']['path']), STORE.path(case['getter']['path']),
                                       case['profile'], case['padding'])
        assert normalized(before) == accepted
        after = action_model.snapshot(STORE.path(case['output']), STORE.path(getter_command['stdout']['path']),
                                      case['profile'], case['padding'], changed=case['table'])
        proof = reference_proof.compare(before, after, case['table'])
        saved = STORE.load(got['facts']['path'])
        assert saved == normalized({'snapshot': after, 'proof': proof})
        assert STORE.pin(case['output']) == candidate
        rows += sum(table['count'] for table in after['tables'].values())
        entries += sum(index['count'] for table in after['tables'].values() for index in table['indexes'].values())
        physical_trees += observations['physical_trees_checked']
        original_bytes += observations['whole_original_prefix_checked']
        for purpose in observations['typed_suffix_purposes']:
            purposes[purpose['kind']] += 1
        outcomes.append({'id': case['id'], 'candidate': candidate,
                         'complete_image_match_before_cached_getter': True,
                         'raw_prefix_bytes_checked': observations['whole_original_prefix_checked'],
                         'typed_suffix_purposes': observations['typed_suffix_purposes']})
    assert (rows, entries, physical_trees, original_bytes) == (38916, 129612, 120, 34603008)
    assert purposes == {'DATA': 12, 'INDEX': 72, 'INDEX_BITMAP': 12, 'CANDIDATE_BITMAP': 4}
    return {'verified': True, 'fixture_processes': 0, 'native_calls': 0, 'JVM_calls': 0,
            **git, 'saved_pin_occurrences': occurrences, 'unique_saved_pins': len(pins),
            'candidate_images_reconstructed': 12, 'cached_getter_proofs': 12,
            'typed_rows_checked': rows, 'logical_index_entries_including_aliases': entries,
            'physical_tree_instances_checked': physical_trees, 'original_prefix_bytes_checked': original_bytes,
            'typed_suffix_pages': purposes, 'outcomes': outcomes}


def main():
    if not __debug__:
        raise RuntimeError('Saved verification requires enabled assertions')
    parser = argparse.ArgumentParser()
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--output', type=Path)
    arguments = parser.parse_args()
    result = verify(arguments.verify_git)
    encoded = json.dumps(result, indent=2) + '\n'
    if arguments.output is not None:
        with arguments.output.open('x') as output:
            output.write(encoded)
    print(encoded, end='')


if __name__ == '__main__':
    raise SystemExit(main())
