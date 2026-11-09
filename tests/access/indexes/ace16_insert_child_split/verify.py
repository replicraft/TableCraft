"""Saved source/reference and child-split candidates; optional Git is the only child."""
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
from ace16_insert_child_split import model as independent_model, reference as source_reference

STORE = ArtifactStore(ROOT)
MODEL = '.cache/ace16-insert-child-split-independent-next-20261027'
REFERENCE = '.cache/ace16-insert-child-split-reference-next-20261027'
REFERENCE_WORK = '.cache/ace16-insert-child-split-reference-first-20261027'
PRODUCER = '.cache/ace16-insert-child-split-candidates-next-20261027'
GETTERS = '.cache/ace16-insert-child-split-getters-next-20261027'
FIXED = {
    MODEL + '/freeze.json': '59a0704eafe7fe347860bf46b4ef238d82ce832f59683acd4574b70a067e8d7d',
    REFERENCE_WORK + '/report.json': 'c609c29df9be74de0276e87a97adac2f5332283f12722407577ea215a928aa45',
    PRODUCER + '/protocol-freeze.json': 'd6ccfced8e16b98cd44e5b7b99e54d771e16ee2ed4a3f1f88f79f14280ff7584',
    GETTERS + '/freeze.json': 'fa4eee16c35b61398d677fd2826d24a83d87f8529a8e017e8323aff848b2f9c7',
    GETTERS + '/controller-freeze.json': '9f019d7fd13caa1043b208ba1409cee3a2194e85868bf408dbd850029ae6c11c',
}
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
    assert hashlib.sha256(private.encode()).hexdigest() == (
        'd23acef9b51867f727fa296fabc8e64440d46d5226cfbb62f9f3695d5803a058')
    public = pins.check(STORE.pin('docs/testing/ace16_insert_child_split/model.py')).read_text()
    assert public == private.replace('Path(__file__).resolve().parents[2]',
                                      'Path(__file__).resolve().parents[3]')
    old = pins.check(STORE.pin(REFERENCE + '/proof.py')).read_text()
    public = pins.check(STORE.pin('docs/testing/ace16_insert_child_split/reference.py')).read_text()

    class OwnerNames(ast.NodeTransformer):
        def visit_Name(self, node):
            if node.id == 'H':
                return ast.copy_location(ast.Name(id='root_reference', ctx=node.ctx), node)
            return node

    private_ast = ast.parse(old)
    public_ast = ast.parse(public)
    old_functions = [ast.dump(OwnerNames().visit(node), include_attributes=False)
                     for node in private_ast.body if isinstance(node, ast.FunctionDef)]
    new_functions = [ast.dump(node, include_attributes=False)
                     for node in public_ast.body if isinstance(node, ast.FunctionDef)]
    assert old_functions == new_functions and len(old_functions) == 5
    for name in ['PAGE', 'EXTRA', 'ORIGINAL']:
        def assignment(tree):
            return [ast.dump(node.value, include_attributes=False) for node in tree.body
                    if isinstance(node, ast.Assign) and any(
                        isinstance(target, ast.Name) and target.id == name for target in node.targets)]
        assert assignment(private_ast) == assignment(public_ast)
    old_owner = STORE.path('.cache/ace16-insert-root-split-reference-next-20261027/proof.py').read_text()
    owner = pins.check(STORE.pin('docs/testing/ace16_insert_root_split/reference.py')).read_text()
    assert owner == old_owner.replace('Path(__file__).resolve().parents[2]',
                                       'Path(__file__).resolve().parents[3]')


def commands(pins, file, count, cap):
    value = STORE.load(file)
    assert value['completed'] is True and value.get('failure') is None
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
        assert case['request']['ID'] == protocol['final_request']['ID'] == 227
        state = proof.inspect(selector, STORE.path(case['source']['path']),
                              STORE.path(case['source_getter']['path']),
                              STORE.path(case['source_maps']['path']), 'before')
        proof.original_preserved(selector, state)
        image, facts = model.derive(state, case['request']['ID'], bytes.fromhex(case['request']['binary_hex']))
        assert facts == case['expectation']
        assert image == pins.check(case['expected_image']).read_bytes()
        assert pins.check(case['request_tsv']).read_bytes() == (
            'int32\t227\tbinary\t' + case['request']['binary_hex'] + '\n').encode()
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
            assert (page in [facts['root'], facts['retained_RIGHT']]
                    or page == facts['new_INDEX_LEFT'] and within >= 2514)
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
    assert args.producer_report_sha256 and args.getter_report_sha256
    pins.fixed(args.producer_report, args.producer_report_sha256)
    pins.fixed(args.getter_report, args.getter_report_sha256)
    native = commands(pins, args.producer_report, 4, 8 * 1024 * 1024)
    getter = commands(pins, args.getter_report, 8, 16 * 1024 * 1024)
    binding_pin = STORE.pin(args.producer_binding)
    assert binding_pin in native['before']
    binding = STORE.load(pins.check(binding_pin).relative_to(ROOT))
    assert binding['revision'] == 'f5168aa65085fb76cfbb979f2d7c1b8f7a8a52d7'
    assert binding['receipt_revision'] == 'b683fb1efacb73729bfbffba070d34b9e6cbb0e4'
    assert binding['artifact']['sha256'] == 'c868c08c0ba8f52c5322e6c6b97e609910ad0f2f868ea4c36e2063d96be8ba2f'
    pins.check(binding['artifact'])
    receipt = STORE.load(pins.check(binding['build_receipt']).relative_to(ROOT))
    assert receipt['exit_code'] == 0 and receipt['consumer'] == 'preview_ace16_insert'
    assert receipt['compiled_revision'] == binding['revision']
    assert receipt['frozen_example'] == binding['artifact']
    assert receipt['source_pins'] == binding['compiled_sources']
    assert len(binding['compiled_sources']) == 753
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
        assert pins.check(command['stdout']).read_bytes() == b'candidate_changed_rows=1\n'
        assert pins.check(command['stderr']).read_bytes() == NOTICE
        assert pins.check(actual['candidate']).read_bytes() == image
        assert gb['candidates'][ordinal] == {'id': case['id'], 'image': actual['candidate']}
        assert getter['cases'][ordinal] == {'id': case['id'], 'whole_image_before_getters': True, 'matched': True}
        configured = getter_protocol['cases'][ordinal]
        assert configured['id'] == case['id'] and configured['expected_image'] == case['expected_image']
        assert configured['source'] == case['source'] and configured['inserted_ID'] == 227
        assert configured['inserted_binary_hex'] == case['request']['binary_hex']
        assert (configured['new_page'], configured['new_slot']) == (facts['new_page'], facts['new_slot'])
        streams = []
        for offset, label in enumerate(['getter', 'maps']):
            call = getter['commands'][ordinal * 2 + offset]
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
        actual_row = candidate_state[1]['rows'][227]
        assert actual_row['page'] << 8 | actual_row['slot'] == facts['new_ROW']
        assert result['new_index_pages'] == [facts['new_INDEX_LEFT']]
        assert result['new_data_pages'] == facts['new_DATA_pages']
        results.append({'id': case['id'], 'candidate': actual['candidate'], 'new_ROW': facts['new_ROW'],
                        'whole_image': True, 'old_ROWS_preserved': 677, 'final_rows': 678,
                        'new_INDEX_pages': 1, 'new_DATA_pages': len(facts['new_DATA_pages'])})
    git = git_objects(binding) if args.verify_git else {'verified': False, 'metadata_processes': 0}
    pins.terminal()
    return {'verified': True, 'available': True, 'scope': 'Four saved engineering candidates only; no native Access save claim',
            'producer_commands': 4, 'fresh_getter_commands': 8, 'prior_reference_commands': 25,
            'complete_key_ROW_entries': 2712, 'old_ROWS_preserved': 2708, 'cases': results,
            'pin_occurrences': pins.count, 'distinct_paths': len(pins.items),
            'Git': git, 'new_native_JVM_Cargo_calls': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preflight-only', action='store_true')
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--producer-report', default='.cache/ace16-insert-child-split-candidates-first-20261027/report.json')
    parser.add_argument('--producer-report-sha256', default='338e696136c3f51015e133a84ac8d9e1b6e092fe2eb6b17ed1f389deaa7fbe15')
    parser.add_argument('--getter-report', default='.cache/ace16-insert-child-split-getters-first-20261027/report.json')
    parser.add_argument('--getter-report-sha256', default='b91f00b4336962dcc4111a51402ca4a626768dff17a86a15a6476e2bd4a240ab')
    parser.add_argument('--producer-binding', default=PRODUCER + '/binding-first.json')
    parser.add_argument('--getter-binding', default=GETTERS + '/binding-first.json')
    args = parser.parse_args()
    try:
        if not args.preflight_only:
            STORE.path(args.producer_report).stat()
            STORE.path(args.getter_report).stat()
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
