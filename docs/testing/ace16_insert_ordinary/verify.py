"""Saved ordinary INSERT proof; no child process unless --verify-git is explicit."""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_ordinary import acceptance, reference_verify


STORE = ArtifactStore(ROOT)
MODEL = '.cache/ace16-insert-ordinary-actual-model-next-20261028'
QUALIFICATION = '.cache/ace16-insert-ordinary-qualification-next-20261028'
SOURCE = '.cache/ace16-insert-ordinary-source-first-20261028'
SOURCE_PLAN = '.cache/ace16-insert-ordinary-source-next-20261028/plan.json'
EXPECTED = '.cache/ace16-insert-ordinary-expected-second-20261028'
PRIOR = '.cache/ace16-insert-ordinary-expected-first-20261028'
NATIVE = '.cache/ace16-insert-ordinary-candidates-next-20261028'
GETTERS = '.cache/ace16-insert-ordinary-getters-next-20261028'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    EXPECTED + '/freeze.json': acceptance.FREEZE_SHA256,
    EXPECTED + '/plan.json': acceptance.PLAN_SHA256,
    SOURCE + '/report.json': '2a44c95bf5da8825b0a15bfb2a31245e542f063e8b585d98e36335e24e0b15e7',
    QUALIFICATION + '/freeze.json': '9cc63e3c3670869330f4cf4c4918df0b7a60835fd518a61ad5a1954b493673d4',
    NATIVE + '/protocol-freeze.json': '499513934048d9515d0cec31c4ee204fd491d222b14b3624d19d73373bdb6145',
    NATIVE + '/binding-first.json': '26e8c95b3b51df50e13eef87e23107ef929e4ba4a5bab861c55b35b32a05acef',
    GETTERS + '/controller-freeze.json': 'c810f29c00dc748db43e48d0cb77af74796f9c03ca687c78aa08ebc1c4dba054',
    GETTERS + '/binding-first.json': 'f468cc40ae50ebf246143811f3295f0f8bd9db715e446a0a610410ffbef5c7e7',
}


class Pins:
    def __init__(self):
        self.items = {}
        self.occurrences = 0

    def check(self, item):
        self.occurrences += 1
        assert self.items.setdefault(item['path'], item) == item, item['path']
        assert STORE.pin(item['path']) == item, item['path']
        return STORE.path(item['path'])

    def fixed(self, file, digest):
        item = STORE.pin(file)
        assert item['sha256'] == digest, file
        return self.check(item)

    def closure(self, file):
        value = STORE.load(file)
        for item in value['inputs']:
            self.check(item)
        for item in value.get('outputs', []):
            self.check(item)
        return value

    def terminal(self):
        for item in list(self.items.values()):
            self.check(item)


def maintained(pins):
    names = ['row_codec.py', 'record_decode.py', 'requests.py', 'image_model.py', 'acceptance.py', 'source.py']
    for name in names:
        owner = QUALIFICATION if name == 'source.py' else MODEL
        original = pins.check(STORE.pin(owner + '/' + name)).read_text()
        expected = original
        if name == 'image_model.py':
            expected = expected.replace('import row_codec\nimport record_decode\n', '')
            expected = expected.replace('parents[2]', 'parents[3]')
            expected = expected.replace('from ace16_insert_recursive import model as tree',
                                        'from ace16_insert_recursive import model as tree\n'
                                        'from ace16_insert_ordinary import row_codec, record_decode')
        elif name == 'source.py':
            expected = expected.replace('parents[2]', 'parents[3]')
            expected = expected.replace(
                "VALUE_MODEL = ROOT / '.cache/ace16-insert-ordinary-actual-model-next-20261028'\n"
                'sys.path.insert(0, str(VALUE_MODEL))\nimport row_codec\nimport record_decode',
                'from ace16_insert_ordinary import row_codec, record_decode')
        elif name == 'acceptance.py':
            expected = expected.replace('parents[2]', 'parents[3]')
            expected = expected.replace(
                "QUALIFICATION = ROOT / '.cache/ace16-insert-ordinary-qualification-next-20261028'\n", '')
            expected = expected.replace('sys.path.insert(0, str(QUALIFICATION))',
                                        "sys.path.insert(0, str(ROOT / 'docs/testing'))")
            expected = expected.replace('import image_model\nimport record_decode\nimport requests\n'
                                        'import row_codec\nimport source',
                                        'from ace16_insert_ordinary import image_model, record_decode, '
                                        'requests, row_codec, source')
        public = pins.check(STORE.pin('docs/testing/ace16_insert_ordinary/' + name)).read_text()
        assert public == expected, ('Maintained direct-owner transformation', name)
        old = [ast.dump(node, include_attributes=False) for node in ast.parse(original).body
               if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
        new = [ast.dump(node, include_attributes=False) for node in ast.parse(public).body
               if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
        assert old == new, ('Function/class identity', name)


def command_report(pins, file, count, stream_bytes):
    result = STORE.load(file)
    assert result['completed'] is True
    assert result.get('failure') is None
    assert result.get('pin_errors', result.get('after_pin_errors')) == []
    assert len(result['commands']) == count
    for item in result.get('inputs', result.get('before', [])) + result['artifacts']:
        pins.check(item)
    for call in result['commands']:
        assert call['status'] == 0 and call['stopped_for'] is None and call['capture_errors'] == []
        assert call['timeout_seconds'] == 30 and call['stream_limit_bytes'] == stream_bytes
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= stream_bytes
        pins.check(call['stdout'])
        pins.check(call['stderr'])
    return result


def rebinding(pins, plan):
    prior = STORE.load(PRIOR + '/freeze.json')
    assert STORE.pin(PRIOR + '/freeze.json')['sha256'] == '6487c063eb358d81921d9ace7cef6a5f2b39fff3cb28f10126603467a374bf71'
    differences = []
    for item in prior['inputs']:
        current = STORE.pin(item['path'])
        if current != item:
            differences.append({'before': item, 'after': current})
        else:
            pins.check(item)
    assert differences == plan['gate_receipt_drift'] and len(differences) == 1
    assert differences[0]['before']['path'] == QUALIFICATION + '/source-gates-first.json'
    assert differences[0]['before']['bytes'] == 4482 and differences[0]['after']['bytes'] == 4670
    for item in prior['outputs']:
        pins.check(item)
    old_plan = STORE.load(PRIOR + '/plan.json')
    for old, new in zip(old_plan['cases'], plan['cases'], strict=True):
        assert old['id'] == new['id']
        assert old['summary'] == new['summary']
        assert old['expected']['sha256'] == new['expected']['sha256']
        assert old['expected']['bytes'] == new['expected']['bytes']
        assert pins.check(old['expected']).read_bytes() == pins.check(new['expected']).read_bytes()
    return {'receipt_only_drift_before_build': True, 'old_and_rebound_images_identical': 4,
            'original_bundle_preserved': True, 'source_or_model_repairs': 0}


def git_objects(binding):
    receipt = binding['build_receipt']
    items = [receipt] + binding['compiled_sources']
    assert len(binding['compiled_sources']) == 766
    queries = [binding['receipt_revision'] + ':' + receipt['path']]
    queries += [binding['revision'] + ':' + item['path'] for item in binding['compiled_sources']]
    assert all('\n' not in query and '\r' not in query for query in queries)
    assert sum(item['bytes'] for item in items) <= 64 * 1024 * 1024
    result = subprocess.run(['git', '--no-replace-objects', 'cat-file', '--batch'], cwd=ROOT,
                            input=('\n'.join(queries) + '\n').encode(), stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, check=True, timeout=30)
    cursor = 0
    for item in items:
        end = result.stdout.index(b'\n', cursor)
        header = result.stdout[cursor:end].split()
        assert len(header) == 3 and header[1] == b'blob'
        size = int(header[2])
        begin = end + 1
        blob = result.stdout[begin:begin + size]
        assert size == item['bytes'] and hashlib.sha256(blob).hexdigest() == item['sha256']
        assert result.stdout[begin + size:begin + size + 1] == b'\n'
        cursor = begin + size + 1
    assert cursor == len(result.stdout)
    return {'verified': True, 'compiled_source_objects': 766, 'receipt_objects': 1,
            'metadata_child_processes': 1}


def verify(args):
    pins = Pins()
    for file, digest in FIXED.items():
        pins.fixed(file, digest)
    pins.fixed(args.producer_report, args.producer_report_sha256)
    pins.fixed(args.getter_report, args.getter_report_sha256)
    for file in [EXPECTED + '/freeze.json', QUALIFICATION + '/freeze.json',
                 NATIVE + '/protocol-freeze.json', GETTERS + '/controller-freeze.json']:
        pins.closure(file)
    maintained(pins)
    # Source gates for all four cases precede all four complete rederivations.
    # These full images precede interpretation of any cached fresh getter.
    plan, states = acceptance.verify_bundle()
    history = rebinding(pins, plan)
    public_reference = reference_verify.verify(STORE, pins.check)
    source_report = command_report(pins, SOURCE + '/report.json', 9, 16 * 1024 * 1024)
    source_plan = STORE.load(SOURCE_PLAN)
    assert source_report['source_qualification'] is False
    for call, expected in zip(source_report['commands'], source_plan['source_commands'], strict=True):
        assert call['label'] == expected['label'] and call['argv'] == expected['argv']
    assert STORE.load(QUALIFICATION + '/source-gates-first.json')['qualified'] is True
    native = command_report(pins, args.producer_report, 4, 8 * 1024 * 1024)
    getters = command_report(pins, args.getter_report, 8, 16 * 1024 * 1024)
    binding = STORE.load(NATIVE + '/binding-first.json')
    getter_binding = STORE.load(GETTERS + '/binding-first.json')
    native_protocol = STORE.load(NATIVE + '/protocol.json')
    getter_protocol = STORE.load(GETTERS + '/protocol.json')
    assert binding['protocol_freeze'] == STORE.pin(NATIVE + '/protocol-freeze.json')
    assert getter_binding['controller_freeze'] == STORE.pin(GETTERS + '/controller-freeze.json')
    assert getter_binding['producer_report'] == STORE.pin(args.producer_report)
    assert getter_binding['producer_binding'] == STORE.pin(NATIVE + '/binding-first.json')
    assert len(binding['commands']) == len(native['cases']) == len(plan['cases']) == 4
    assert len(getter_binding['commands']) == len(getter_protocol['commands']) == 8
    assert native_protocol['child_limits'] == {'processes': 4, 'seconds': 30, 'stream_bytes': 8388608}
    assert getter_protocol['limits'] == {'processes': 8, 'stream_bytes': 16777216,
                                        'seconds': 30, 'heap_bytes': 536870912}
    receipt = STORE.load(binding['build_receipt']['path'])
    assert receipt['exit_code'] == 0 and receipt['consumer'] == 'preview_ace16_insert'
    assert receipt['compiled_revision'] == binding['revision']
    assert receipt['frozen_example'] == binding['artifact']
    assert receipt['source_pins'] == binding['compiled_sources'] and len(binding['compiled_sources']) == 766
    pins.check(binding['artifact'])
    pins.check(binding['build_receipt'])
    results = []
    for ordinal, (case, (state, all_tables)) in enumerate(zip(plan['cases'], states, strict=True)):
        actual = native['cases'][ordinal]
        assert actual['id'] == case['id'] and actual['matched'] is True and 'failure' not in actual
        candidate = pins.check(actual['candidate'])
        image = pins.check(case['expected']).read_bytes()
        assert candidate.read_bytes() == image
        call = native['commands'][ordinal]
        assert call['label'] == case['id'] and call['argv'] == binding['commands'][ordinal]
        assert call['argv'] == [str(STORE.path(binding['artifact']['path'])),
                                str(STORE.path(case['source'])), str(candidate), case['table'],
                                str(STORE.path(case['requests']['path']))]
        assert pins.check(call['stdout']).read_bytes() == b'candidate_changed_rows=1\n'
        assert pins.check(call['stderr']).read_bytes() == NOTICE
        assert getter_binding['candidates'][ordinal] == {'id': case['id'], 'image': actual['candidate']}
        recorded = getters['cases'][ordinal]
        assert recorded['id'] == case['id'] and recorded['matched'] is True
        assert recorded['whole_image_before_getters'] is True
        streams = []
        for offset, suffix in enumerate(('getter', 'maps')):
            index = ordinal * 2 + offset
            call = getters['commands'][index]
            assert call['label'] == case['id'] + '-' + suffix
            assert call['argv'] == getter_binding['commands'][index]['argv']
            template = getter_protocol['commands'][index]
            assert call['argv'] == [str(candidate) if token == 'CANDIDATE' else token
                                    for token in template['argv']]
            assert call['argv'][:5] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8',
                                       '-Duser.timezone=UTC',
                                       '-Dcom.healthmarketscience.jackcess.dateTimeType=LOCAL_DATE_TIME']
            assert pins.check(call['stderr']).read_bytes() == b''
            streams.append(pins.check(call['stdout']).read_text())
        proof = acceptance.verify_fresh(image, case, state, all_tables, *streams)
        assert proof == recorded['proof'] and proof['verified']
        results.append({'id': case['id'], 'expected_sha256': case['expected']['sha256'],
                        'fresh': proof, 'summary': case['summary']})
    git = git_objects(binding) if args.verify_git else {'verified': False, 'metadata_child_processes': 0}
    pins.terminal()
    return {'verified': True, 'available': True,
            'scope': 'Four ordinary INSERT cases on two immutable originals; no native Access save claim',
            'source_capture_commands': 9, 'producer_commands': 4, 'fresh_readonly_commands': 8,
            'public_reference': public_reference,
            'cases': results, 'receipt_rebinding': history,
            'all_table_typed_rows': sum(case['fresh']['typed_rows'] for case in results),
            'selected_primary_key_ROW_pairs': sum(case['fresh']['selected_primary_key_ROW_pairs'] for case in results),
            'old_selected_ROWS_preserved': sum(case['fresh']['old_selected_ROWS_preserved'] for case in results),
            'pin_occurrences': pins.occurrences, 'distinct_paths': len(pins.items),
            'Git': git, 'native_JVM_Cargo_calls': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--producer-report', default='.cache/ace16-insert-ordinary-candidates-first-20261028/report.json')
    parser.add_argument('--producer-report-sha256', default='96344ef6ac9e4b759395fd077c24e3be196d3808b525a65eb80df6f515c0104e')
    parser.add_argument('--getter-report', default='.cache/ace16-insert-ordinary-getters-first-20261028/report.json')
    parser.add_argument('--getter-report-sha256', default='9586af4d1da0664ef3d6c74a0e8eb6cee130f01ce173ec5420419236c123af94')
    args = parser.parse_args()
    try:
        STORE.path(args.producer_report).stat()
        STORE.path(args.getter_report).stat()
        result = verify(args)
    except FileNotFoundError as error:
        try:
            missing = Path(error.filename).resolve().relative_to(ROOT).as_posix()
        except (TypeError, ValueError):
            missing = 'required private evidence'
        print(json.dumps({'verified': False, 'available': False,
                          'reason': 'Required private evidence is absent', 'missing': missing}, indent=2))
        return 2
    except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({'verified': False, 'available': True,
                          'reason': type(error).__name__ + ': ' + str(error)}, indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
