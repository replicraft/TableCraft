"""Saved source/reference and general-height-one candidates; optional Git is the only child."""
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
from ace16_insert_general_height_one import model as independent_model, reference as source_reference

STORE = ArtifactStore(ROOT)
MODEL = '.cache/ace16-insert-general-height-one-independent-next-20261027'
REFERENCE = '.cache/ace16-insert-general-height-one-reference-next-20261027'
REFERENCE_WORK = '.cache/ace16-insert-general-height-one-reference-first-20261027'
PRODUCER = '.cache/ace16-insert-general-height-one-candidates-next-20261027'
GETTERS = '.cache/ace16-insert-general-height-one-getters-next-20261027'
CONTINUATION = '.cache/ace16-insert-general-height-one-getters-continuation-next-20261028'
FIXED = {'.cache/ace16-insert-general-height-one-independent-next-20261027/freeze.json': '628be6c2988fe50d0ab6804b0d99f773f27f88f73b42dc3984c1378217b9b4e5', '.cache/ace16-insert-general-height-one-reference-first-20261027/report.json': 'ad3fe53c16072301f53e97bd14e1eb98ed608cc4a47a0e2afe261b536175fe76', '.cache/ace16-insert-general-height-one-candidates-next-20261027/protocol-freeze.json': '58914b2c2e2f96c736802436b13cfa5f191a3a1419871964b58076ce02d1b300', '.cache/ace16-insert-general-height-one-getters-next-20261027/freeze.json': 'd1c32b5b2c897a041b94140c94194cc369cc848c666af72a12e5a0eced6eec77', '.cache/ace16-insert-general-height-one-getters-next-20261027/controller-freeze.json': '097ef3a00c752cfb59c195e0c87bc44a593b7fcc3a3ba67560f2be68e2468f5a'}
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


class Pins:
    def __init__(self):
        self.count = 0
        self.items = {}

    def check(self, item):
        self.count += 1
        assert self.items.setdefault(item['path'], item) == item
        assert STORE.pin(item['path']) == item, item['path']
        return STORE.path(item['path'])

    def fixed(self, file, digest):
        item = STORE.pin(file)
        assert item['sha256'] == digest, file
        return self.check(item)

    def freeze(self, file):
        value = STORE.load(file)
        assert value['inputs']
        for item in value['inputs']:
            self.check(item)
        return value

    def terminal(self):
        for item in list(self.items.values()):
            self.check(item)


def portable_missing(filename):
    try:
        return Path(filename).resolve().relative_to(ROOT).as_posix()
    except (TypeError, ValueError):
        return 'required private evidence'


def maintained_sources(pins):
    private = pins.check(STORE.pin(MODEL + '/prospective_model.py')).read_text()
    assert hashlib.sha256(private.encode()).hexdigest() == 'aa17311422ec385becf4a4e7104b27f3ee5ac8f7e01b69922798f4af43e169bf'
    public = pins.check(STORE.pin('docs/testing/ace16_insert_general_height_one/model.py')).read_text()
    assert public == private.replace('Path(__file__).resolve().parents[2]',
                                      'Path(__file__).resolve().parents[3]')
    original = pins.check(STORE.pin(REFERENCE + '/proof.py')).read_text()
    public = pins.check(STORE.pin('docs/testing/ace16_insert_general_height_one/reference.py')).read_text()
    assert public == original.replace('Path(__file__).resolve().parents[2]',
                                       'Path(__file__).resolve().parents[3]')
    original_functions = [ast.dump(node, include_attributes=False) for node in ast.parse(original).body
                          if isinstance(node, ast.FunctionDef)]
    public_functions = [ast.dump(node, include_attributes=False) for node in ast.parse(public).body
                        if isinstance(node, ast.FunctionDef)]
    assert len(original_functions) == 5 and original_functions == public_functions


def commands(pins, file, count, cap, completed=True):
    value = STORE.load(file)
    assert value['completed'] is completed
    assert value.get('failure') == (None if completed else 'AssertionError: ')
    assert value['after_pin_errors'] == [] and len(value['commands']) == count
    for item in value['before'] + value['artifacts']:
        pins.check(item)
    for item in value['commands']:
        assert item['status'] == 0 and item['stopped_for'] is None
        assert item['capture_errors'] == [] and item['timeout_seconds'] == 30
        assert item['stream_limit_bytes'] == cap
        assert item['observed_stream_bytes'] == [item['stdout']['bytes'], item['stderr']['bytes']]
        assert max(item['observed_stream_bytes']) <= cap
        pins.check(item['stdout'])
        pins.check(item['stderr'])
    return value


def historical_path(token, old_root, expected):
    assert Path(token).is_absolute()
    assert Path(token).relative_to(old_root).as_posix() == expected


def git_objects(binding):
    pins = [binding['build_receipt']] + binding['compiled_sources']
    queries = [binding['receipt_revision'] + ':' + pins[0]['path']]
    queries += [binding['revision'] + ':' + item['path'] for item in pins[1:]]
    assert all('\n' not in item and '\r' not in item for item in queries)
    assert sum(item['bytes'] for item in pins) <= 64 * 1024 * 1024
    result = subprocess.run(['git', '--no-replace-objects', 'cat-file', '--batch'], cwd=ROOT,
                            input=('\n'.join(queries) + '\n').encode(),
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    position = 0
    for item in pins:
        stop = result.stdout.index(b'\n', position)
        header = result.stdout[position:stop].split()
        assert len(header) == 3 and header[1] == b'blob'
        size = int(header[2])
        begin = stop + 1
        data = result.stdout[begin:begin + size]
        assert size == item['bytes'] and hashlib.sha256(data).hexdigest() == item['sha256']
        assert result.stdout[begin + size:begin + size + 1] == b'\n'
        position = begin + size + 1
    assert position == len(result.stdout)
    return {'verified': True, 'compiled_sources': len(binding['compiled_sources']),
            'receipt_objects': 1, 'metadata_processes': 1}


def expected(pins):
    for file, digest in FIXED.items():
        pins.fixed(file, digest)
    for file in [MODEL + '/freeze.json', PRODUCER + '/protocol-freeze.json',
                 GETTERS + '/freeze.json', GETTERS + '/controller-freeze.json']:
        pins.freeze(file)
    maintained_sources(pins)
    model = independent_model
    proof = source_reference
    plan = STORE.load(MODEL + '/plan.json')
    protocol = STORE.load(REFERENCE + '/plan.json')
    derived = []
    for case, selector in zip(plan['cases'], protocol['cases'], strict=True):
        assert case['id'] == selector['id'] and case['source']['path'] == selector['source']
        assert [row['ID'] for row in case['requests']] == protocol['final_request_ids'] == [-227, 228]
        state = proof.inspect(selector, STORE.path(case['source']['path']),
                              STORE.path(case['source_getter']['path']),
                              STORE.path(case['source_maps']['path']), 'before')
        proof.original_preserved(selector, state)
        image, facts = model.derive(state, [(row['ID'], bytes.fromhex(row['binary_hex'])) for row in case['requests']])
        assert facts == case['expectation']
        assert image == pins.check(case['expected_image']).read_bytes()
        assert pins.check(case['request_tsv']).read_bytes() == (
            ''.join('int32\t' + str(row['ID']) + '\tbinary\t' + row['binary_hex'] + '\n'
                    for row in case['requests'])).encode()
        derived.append((case, selector, state, image, facts))
    assert len(derived) == 4
    reference = commands(pins, REFERENCE_WORK + '/report.json', 25, 16 * 1024 * 1024)
    assert all(a['label'] == b['label'] and a['argv'] == b['argv']
               for a, b in zip(reference['commands'], protocol['commands'], strict=True))
    assert reference['all_source_gates_passed_before_any_append']
    assert len(reference['sources']) == 4
    for case, selector, state, image, facts in derived:
        result = reference['references'][case['id']]
        assert result['verified']
        public_path = pins.check(result['image'])
        public_state = proof.inspect(selector, public_path,
                                     STORE.path(REFERENCE_WORK + '/' + case['id'] + '-after-getter.stdout'),
                                     STORE.path(REFERENCE_WORK + '/' + case['id'] + '-after-maps.stdout'), 'after')
        raw = proof.transition(selector, state, public_state)
        assert raw == STORE.load(result['proof']['path'])
        public = public_path.read_bytes()
        expected_nodes, expected_leaves, expected_keys = model.DECODE.tree(image, state[2]['root'], state[2]['definition'])
        assert expected_keys == [entry for number in public_state[5]
                                  for entry in public_state[4][number]['entries']]
        assert expected_leaves == public_state[5]
        for role in ['global', 'owned', 'hint', 'index_owned']:
            assert model.SOURCE.map_record(image, state[2][role])['members'] == public_state[3][role]['members']
        assert len(image) == len(public)
        for offset, (native_byte, public_byte) in enumerate(zip(image, public)):
            if native_byte == public_byte:
                continue
            page, within = divmod(offset, 4096)
            assert (page in [facts['root'], state[5][0], state[5][2]]
                    or page in facts['new_INDEX_creation_order'] and within >= 2514)
    return plan, proof, derived


def verify(args):
    pins = Pins()
    plan, proof, derived = expected(pins)
    if args.preflight_only:
        pins.terminal()
        return {'preflight_verified': True, 'scope': 'Prospective saved-proof readiness; no actual candidate claim',
                'expected_images': 4, 'prior_reference_commands_verified': 25,
                'actual_candidate_acceptance': False, 'pin_occurrences': pins.count,
                'distinct_paths': len(pins.items), 'new_native_JVM_Cargo_calls': 0}
    assert args.producer_report_sha256 and args.getter_report_sha256 and args.continuation_report_sha256
    pins.fixed(args.producer_report, args.producer_report_sha256)
    pins.fixed(args.getter_report, args.getter_report_sha256)
    pins.fixed(args.continuation_report, args.continuation_report_sha256)
    pins.fixed(CONTINUATION + '/freeze.json', 'ec45de4f67dfcc74e28622ed7e7796422b515988e92a4a327dba9d474e1eb86e')
    pins.freeze(CONTINUATION + '/freeze.json')
    native = commands(pins, args.producer_report, 4, 8 * 1024 * 1024)
    getter = commands(pins, args.getter_report, 4, 8 * 1024 * 1024, completed=False)
    continuation = commands(pins, args.continuation_report, 4, 8 * 1024 * 1024)
    assert getter['cases'] == [
        {'id': 'V2016-fitted', 'whole_image_before_getters': True, 'matched': True},
        {'id': 'V2016-full84', 'whole_image_before_getters': True, 'matched': True},
        {'id': 'V2019-fitted', 'whole_image_before_getters': True, 'matched': False},
    ]
    assert continuation['stopped_original_report'] == STORE.pin(args.getter_report)
    assert STORE.pin(CONTINUATION + '/freeze.json') in continuation['before']
    continuation_protocol = STORE.load(CONTINUATION + '/protocol.json')
    assert continuation_protocol['stopped_report'] == STORE.pin(args.getter_report)
    assert continuation_protocol['limits'] == {'processes': 4, 'seconds': 30,
                                              'bytes_per_stream': 8388608, 'heap_mib': 512}
    assert len(continuation_protocol['commands']) == len(continuation['commands']) == 4
    assert all(call['argv'] == template['argv'] and call['label'] == template['label']
               for call, template in zip(continuation['commands'], continuation_protocol['commands'], strict=True))
    getter_commands = getter['commands'] + continuation['commands']
    getter_cases = getter['cases'][:2] + continuation['cases']
    assert len(getter_commands) == 8 and len(getter_cases) == 4
    # These lists are a saved-proof view only: no synthetic successful raw report.

    binding_pin = STORE.pin(args.producer_binding)
    assert binding_pin in native['before']
    binding = STORE.load(pins.check(binding_pin).relative_to(ROOT))
    assert binding['revision'] == '8107435d2f1c5957702d9325c9c1d62a5801e668'
    assert binding['receipt_revision'] == '13dc4285a9cb42fe3023fc60b280410eb0e0ca2b'
    assert binding['artifact']['sha256'] == '360426f9c83d1e61bcdc776be0d5d2dad3584b19fee59d6a26c2b590ed6860dc'
    pins.check(binding['artifact'])
    receipt = STORE.load(pins.check(binding['build_receipt']).relative_to(ROOT))
    assert receipt['exit_code'] == 0 and receipt['consumer'] == 'preview_ace16_insert'
    assert receipt['compiled_revision'] == binding['revision']
    assert receipt['frozen_example'] == binding['artifact']
    assert receipt['source_pins'] == binding['compiled_sources']
    assert len(binding['compiled_sources']) == 756
    pins.check(receipt['build_log'])
    pins.check(receipt['dependency_info'])
    assert binding['protocol_freeze'] == STORE.pin(PRODUCER + '/protocol-freeze.json')
    for revision in [binding['revision'], binding['receipt_revision']]:
        assert len(revision) == 40 and all(c in '0123456789abcdef' for c in revision)
    gb_pin = STORE.pin(args.getter_binding)
    assert gb_pin in getter['before']
    gb = STORE.load(pins.check(gb_pin).relative_to(ROOT))
    for key, file in [('getter_protocol_freeze', GETTERS + '/freeze.json'),
                      ('controller_freeze', GETTERS + '/controller-freeze.json'),
                      ('producer_protocol_freeze', PRODUCER + '/protocol-freeze.json'),
                      ('producer_report', args.producer_report)]:
        assert gb[key] == STORE.pin(file)
    assert gb['producer_report'] in getter['before']
    producer_protocol = STORE.load(PRODUCER + '/protocol.json')
    getter_protocol = STORE.load(GETTERS + '/protocol.json')
    old_root = Path(native['commands'][0]['argv'][0]).parents[len(Path(binding['artifact']['path']).parts) - 1]
    results = []
    for ordinal, (case, selector, state, image, facts) in enumerate(derived):
        actual = native['cases'][ordinal]
        command = native['commands'][ordinal]
        request = producer_protocol['cases'][ordinal]
        assert actual['id'] == command['label'] == request['id'] == case['id']
        assert actual['matched'] and actual.get('failure') is None
        assert request['source'] == case['source'] and request['expected_image'] == case['expected_image']
        assert pins.check(request['request']).read_bytes() == pins.check(case['request_tsv']).read_bytes()
        assert len(command['argv']) == 5 and command['argv'][3] == case['table']
        for token, file in [(command['argv'][0], binding['artifact']['path']),
                            (command['argv'][1], case['source']['path']),
                            (command['argv'][2], actual['candidate']['path']),
                            (command['argv'][4], request['request']['path'])]:
            historical_path(token, old_root, file)
        assert pins.check(command['stdout']).read_bytes() == b'candidate_changed_rows=2\n'
        assert pins.check(command['stderr']).read_bytes() == NOTICE
        assert pins.check(actual['candidate']).read_bytes() == image
        assert gb['candidates'][ordinal] == {'id': case['id'], 'image': actual['candidate']}
        assert getter_cases[ordinal] == {'id': case['id'], 'whole_image_before_getters': True, 'matched': True}
        configured = getter_protocol['cases'][ordinal]
        assert configured['id'] == case['id'] and configured['expected_image'] == case['expected_image']
        assert configured['source'] == case['source'] and configured['final_count'] == 1130
        assert configured['inserted_rows'] == [
            {'ID': row['ID'], 'binary_hex': row['binary_hex'], 'page': reference >> 8, 'slot': reference & 255}
            for row, reference in zip(case['requests'], facts['new_ROWS'], strict=True)]
        streams = []
        for offset, label in enumerate(['getter', 'maps']):
            call = getter_commands[ordinal * 2 + offset]
            assert call['label'] == case['id'] + '-' + label
            target = str(old_root / actual['candidate']['path'])
            assert call['argv'] == [target if token == 'CANDIDATE' else token
                                    for token in configured['commands'][offset]]
            assert call['argv'][:3] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8']
            assert pins.check(call['stderr']).read_bytes() == b''
            stream = pins.check(call['stdout'])
            cached = configured['expected_getter' if offset == 0 else 'expected_maps']
            assert stream.read_bytes() == pins.check(cached).read_bytes()
            streams.append(stream)
        candidate_state = proof.inspect(selector, pins.check(actual['candidate']), *streams, 'after')
        result = proof.transition(selector, state, candidate_state)
        for row, reference in zip(case['requests'], facts['new_ROWS'], strict=True):
            actual_row = candidate_state[1]['rows'][row['ID']]
            assert actual_row['page'] << 8 | actual_row['slot'] == reference
        assert result['new_index_pages'] == facts['new_INDEX_creation_order']
        assert result['new_data_pages'] == facts['new_DATA_pages']
        results.append({'id': case['id'], 'candidate': actual['candidate'], 'new_ROWS': facts['new_ROWS'],
                        'whole_image': True, 'old_ROWS_preserved': 1128, 'final_rows': 1130,
                        'new_INDEX_pages': 2, 'new_DATA_pages': len(facts['new_DATA_pages'])})
    git = git_objects(binding) if args.verify_git else {'verified': False, 'metadata_processes': 0}
    pins.terminal()
    return {'verified': True, 'available': True, 'scope': 'Four saved engineering candidates only; no native Access save claim',
            'producer_commands': 4, 'fresh_getter_commands': 8, 'prior_reference_commands': 25,
            'getter_controller_history': {'original_completed': False, 'original_successful_commands': 4,
                                          'failure': 'Four-process capture ceiling stopped before fifth JVM',
                                          'declared_stream_bytes': 16777216, 'actual_stream_bytes': 8388608,
                                          'stream_limit_hit': False, 'continuation_successful_commands': 4,
                                          'completed_calls_rerun': 0},
            'complete_key_ROW_entries': 4520, 'old_ROWS_preserved': 4512, 'cases': results,
            'pin_occurrences': pins.count, 'distinct_paths': len(pins.items),
            'Git': git, 'new_native_JVM_Cargo_calls': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preflight-only', action='store_true')
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--producer-report', default='.cache/ace16-insert-general-height-one-candidates-first-20261027/report.json')
    parser.add_argument('--producer-report-sha256', default='459e60e0f1c674fe98dc3c27832e3827bfb7133b02c50cf2e0080448ad614c2d')
    parser.add_argument('--getter-report', default='.cache/ace16-insert-general-height-one-getters-first-20261027/report.json')
    parser.add_argument('--getter-report-sha256', default='50c5d202cba0855251867b54b5bc820c166007e9440f5c9e43cea4ff2314eb72')
    parser.add_argument('--continuation-report', default='.cache/ace16-insert-general-height-one-getters-continuation-first-20261028/report.json')
    parser.add_argument('--continuation-report-sha256', default='4bc1ff18867929413f61402f3f4de52a1efd8449b0a7934543829648128e29ee')
    parser.add_argument('--producer-binding', default=PRODUCER + '/binding-first.json')
    parser.add_argument('--getter-binding', default=GETTERS + '/binding-first.json')
    args = parser.parse_args()
    try:
        if not args.preflight_only:
            STORE.path(args.producer_report).stat()
            STORE.path(args.getter_report).stat()
            STORE.path(args.continuation_report).stat()
        result = verify(args)
    except FileNotFoundError as error:
        print(json.dumps({'verified': False, 'available': False,
                          'reason': 'Required private evidence is absent',
                          'missing': portable_missing(error.filename)}, indent=2))
        return 2
    except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({'verified': False, 'available': True,
                          'reason': type(error).__name__ + ': ' + str(error)}, indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
