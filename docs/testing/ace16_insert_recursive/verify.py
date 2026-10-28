"""Saved recursive INSERT proof; no child process unless --verify-git is explicit."""
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
from ace16_insert_recursive import model, source, source_geometry, reference_verify

STORE = ArtifactStore(ROOT)
ORACLE = '.cache/ace16-insert-recursive-originals-next-20261028'
EXPECTED = '.cache/ace16-insert-recursive-expected-second-20261028'
NATIVE = '.cache/ace16-insert-recursive-candidates-next-20261028'
GETTERS = '.cache/ace16-insert-recursive-getters-next-20261028'
SOURCE = '.cache/ace16-insert-parent-growth-reference-next-20261028'
SOURCE_CONTINUED = '.cache/ace16-insert-parent-growth-continuation-next-20261028'
SOURCE_WORK = '.cache/ace16-insert-parent-growth-reference-first-20261028'
SOURCE_CONTINUED_WORK = '.cache/ace16-insert-parent-growth-continuation-first-20261028'
SELECTORS = ['V2016-root', 'V2016-internal', 'V2019-root', 'V2019-internal']
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    EXPECTED + '/freeze.json': '4a8ee59f32d76ec22bb5ccf06c9483916bf18dc8e53c772f192c5d2f0b216d0d',
    EXPECTED + '/plan.json': '94d841c8e393729be8804836b6b2e7d85e12adf37105355a3fe25efb612c1b68',
    ORACLE + '/source-binding-v2.json': '2f52a93492766180988bef2a0fc82e07a60cfa34f025769a31ef3fee645422ef',
    NATIVE + '/protocol-freeze.json': '30aca4635745ac3ac9732742c812706fd6755801f98fa5392c438cbd813bfbc7',
    NATIVE + '/binding-first.json': '0bd0051ec4e362af6230637132cc3a2756862c59f14aa90735773cc766b5d724',
    GETTERS + '/controller-freeze.json': '3450cdb872a81fa7f679ecc3c2341739548ddcfc2ca33388f8e2376010d6d918',
    GETTERS + '/binding-first.json': '2be5f1d89f28c28cce9efe227289ed09e41b68eba8366a42bac3216c4b308f3f',
}


class Pins:
    def __init__(self):
        self.items = {}
        self.count = 0

    def check(self, item):
        self.count += 1
        assert self.items.setdefault(item['path'], item) == item, item['path']
        assert STORE.pin(item['path']) == item, item['path']
        return STORE.path(item['path'])

    def fixed(self, file, digest):
        item = STORE.pin(file)
        assert item['sha256'] == digest, file
        return self.check(item)

    def closure(self, file):
        value = STORE.load(file)
        assert value['inputs']
        for item in value['inputs']:
            self.check(item)
        return value

    def terminal(self):
        for item in list(self.items.values()):
            self.check(item)


def maintained(pins):
    pairs = [
        (ORACLE + '/model.py', 'model.py', '93aa145ba8406badd869aad31d9f448ead85be371f4baf4275a9840c9922b6ef'),
        (SOURCE_CONTINUED + '/snapshot_deleted.py', 'source.py', '783aec629b15db090c2052dab5890b11596e0745bbd11764c10bd67930f4ac14'),
        (SOURCE + '/source_geometry.py', 'source_geometry.py', None),
    ]
    for original, name, digest in pairs:
        private = pins.check(STORE.pin(original)).read_text()
        if digest is not None:
            assert hashlib.sha256(private.encode()).hexdigest() == digest
        public = pins.check(STORE.pin('docs/testing/ace16_insert_recursive/' + name)).read_text()
        assert public == private.replace('Path(__file__).resolve().parents[2]',
                                          'Path(__file__).resolve().parents[3]')
        old_functions = [ast.dump(node, include_attributes=False) for node in ast.parse(private).body
                         if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
        new_functions = [ast.dump(node, include_attributes=False) for node in ast.parse(public).body
                         if isinstance(node, (ast.FunctionDef, ast.ClassDef))]
        assert old_functions == new_functions


def commands(pins, file, count, completed):
    value = STORE.load(file)
    assert value['completed'] is completed
    assert value.get('failure') == (None if completed else 'AssertionError: ')
    assert value.get('pin_errors', value.get('after_pin_errors')) == []
    assert len(value['commands']) == count
    inputs = value['inputs'] if 'inputs' in value else value['before']
    for item in inputs + value['artifacts']:
        pins.check(item)
    for call in value['commands']:
        assert call['status'] == 0 and call['stopped_for'] is None and call['capture_errors'] == []
        assert call['observed_stream_bytes'] == [call['stdout']['bytes'], call['stderr']['bytes']]
        assert max(call['observed_stream_bytes']) <= call['stream_limit_bytes']
        pins.check(call['stdout'])
        pins.check(call['stderr'])
    return value


def source_states(pins, binding):
    original = commands(pins, binding['original_source_report']['path'], 5, False)
    continued = commands(pins, binding['continued_source_report']['path'], 12, True)
    assert continued['completed_commands_repeated'] == 0
    protocol = STORE.load(binding['source_plan']['path'])
    actual = original['commands'] + continued['commands']
    for call, template in zip(actual, protocol['commands'][:17], strict=True):
        assert call['label'] == template['label'] and call['argv'] == template['argv']
        assert call['timeout_seconds'] == template['timeout_seconds']
        assert call['stream_limit_bytes'] == 16 * 1024 * 1024
        assert pins.check(call['stderr']).read_bytes() == b''
    assert [case['id'] for case in binding['cases']] == SELECTORS
    states = []
    for case in binding['cases']:
        before = source.inspect(case, pins.check(case['source']), pins.check(case['before_getter']),
                                pins.check(case['before_maps']))
        raw, table, layout, maps, nodes, leaves = before
        geometry = source_geometry.certify(pins.check(case['large']).read_bytes(), raw,
                                           pins.check(case['selection']), layout['definition'])
        recorded = STORE.load(pins.check(case['gate_receipt']).relative_to(ROOT))
        assert all(recorded[key] == value for key, value in geometry.items())
        assert all(recorded[key] == layout[key]
                   for key in ['historical_before', 'visited_slots', 'deleted_slots'])
        assert (table['count'], layout['historical_before'], nodes[layout['root']]['height']) == (
            (782, 99424, 1) if case['mode'] == 'root' else (963, 149184, 2))
        states.append((case, before, source.parsed(pins.check(case['before_getter']).read_text())))
    return states


def derive(pins):
    for file, digest in FIXED.items():
        pins.fixed(file, digest)
    for file in [EXPECTED + '/freeze.json', NATIVE + '/protocol-freeze.json', GETTERS + '/controller-freeze.json']:
        pins.closure(file)
    maintained(pins)
    plan = STORE.load(EXPECTED + '/plan.json')
    binding = STORE.load(plan['source_binding']['path'])
    for item in binding['inputs']:
        pins.check(item)
    states = source_states(pins, binding)
    derived = []
    for expected, (selector, before, all_tables) in zip(plan['cases'], states, strict=True):
        assert expected['id'] == selector['id'] and expected['source'] == selector['source']
        assert expected['requests'] == [{'ID': 2147483647, 'binary_hex': ''}]
        image, details = model.derive(before, [(2147483647, b'')])
        assert details == expected['expectation']
        assert pins.check(expected['expected_image']).read_bytes() == image
        assert pins.check(expected['request_tsv']).read_bytes() == b'int32\t2147483647\tbinary\t\n'
        derived.append((expected, before, all_tables, image, details))
    assert len(derived) == 4
    return plan, derived


def git_objects(binding):
    objects = [binding['build_receipt']] + binding['compiled_sources']
    queries = [binding['receipt_revision'] + ':' + objects[0]['path']]
    queries += [binding['revision'] + ':' + item['path'] for item in objects[1:]]
    assert len(objects) == 759 and sum(item['bytes'] for item in objects) <= 64 * 1024 * 1024
    assert all('\n' not in query and '\r' not in query for query in queries)
    output = subprocess.run(['git', '--no-replace-objects', 'cat-file', '--batch'], cwd=ROOT,
                            input=('\n'.join(queries) + '\n').encode(), stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=30, check=True).stdout
    position = 0
    for item in objects:
        end = output.index(b'\n', position)
        header = output[position:end].split()
        assert len(header) == 3 and header[1] == b'blob'
        size = int(header[2])
        begin = end + 1
        assert size == item['bytes']
        assert hashlib.sha256(output[begin:begin + size]).hexdigest() == item['sha256']
        assert output[begin + size:begin + size + 1] == b'\n'
        position = begin + size + 1
    assert position == len(output)
    return {'verified': True, 'compiled_source_objects': 758, 'receipt_objects': 1, 'metadata_child_processes': 1}


def verify(args):
    pins = Pins()
    plan, derived = derive(pins)
    if args.preflight_only:
        pins.terminal()
        return {'verified': True, 'preflight_only': True, 'candidate_qualification': False,
                'expected_images': 4, 'source_commands': 17, 'pin_occurrences': pins.count,
                'distinct_paths': len(pins.items), 'native_JVM_Cargo_calls': 0}
    public_reference = reference_verify.verify(STORE, pins.check)
    pins.fixed(args.producer_report, args.producer_report_sha256)
    pins.fixed(args.getter_report, args.getter_report_sha256)
    native = commands(pins, args.producer_report, 4, True)
    getters = commands(pins, args.getter_report, 8, True)
    binding = STORE.load(NATIVE + '/binding-first.json')
    getter_binding = STORE.load(GETTERS + '/binding-first.json')
    assert STORE.pin(NATIVE + '/binding-first.json') in native['before']
    assert STORE.pin(GETTERS + '/binding-first.json') in getters['before']
    assert binding['protocol_freeze'] in native['before']
    assert binding['artifact'] in native['before'] and binding['build_receipt'] in native['before']
    receipt = STORE.load(pins.check(binding['build_receipt']).relative_to(ROOT))
    assert receipt['exit_code'] == 0 and receipt['consumer'] == 'preview_ace16_insert'
    assert receipt['compiled_revision'] == binding['revision']
    assert receipt['frozen_example'] == binding['artifact']
    assert receipt['source_pins'] == binding['compiled_sources'] and len(binding['compiled_sources']) == 758
    assert binding['artifact']['sha256'] == '3646e843828e880b7fc176142c50bf314fcae35cba46362387e1ab611a7e6f29'
    pins.check(binding['artifact'])
    assert getter_binding['producer_report'] == STORE.pin(args.producer_report)
    assert getter_binding['producer_binding'] == STORE.pin(NATIVE + '/binding-first.json')
    assert getter_binding['controller_freeze'] == STORE.pin(GETTERS + '/controller-freeze.json')
    assert all(len(binding[key]) == 40 and all(c in '0123456789abcdef' for c in binding[key])
               for key in ['revision', 'receipt_revision'])
    native_protocol = STORE.load(NATIVE + '/protocol.json')
    getter_protocol = STORE.load(GETTERS + '/protocol.json')
    assert len(native_protocol['cases']) == 4 and len(getter_protocol['commands']) == 8
    assert getter_protocol['limits'] == {'processes': 8, 'seconds': 30, 'stream_bytes': 16777216,
                                        'heap_mib': 512, 'image_bytes': 67108864}
    # First compare ALL complete candidates. Only then interpret any fresh getter.
    for ordinal, (case, before, all_tables, image, details) in enumerate(derived):
        outcome = native['cases'][ordinal]
        assert outcome['id'] == case['id'] and outcome['matched'] is True and outcome.get('failure') is None
        assert pins.check(outcome['candidate']).read_bytes() == image
        assert getter_binding['candidates'][ordinal] == {'id': case['id'], 'image': outcome['candidate']}
        call = native['commands'][ordinal]
        assert call['label'] == case['id'] and call['argv'] == binding['commands'][ordinal]
        assert call['timeout_seconds'] == 30 and call['stream_limit_bytes'] == 8 * 1024 * 1024
        assert pins.check(call['stdout']).read_bytes() == b'candidate_changed_rows=1\n'
        assert pins.check(call['stderr']).read_bytes() == NOTICE
        assert native_protocol['cases'][ordinal]['source'] == case['source']
        assert native_protocol['cases'][ordinal]['request'] == case['request_tsv']
    results = []
    for ordinal, (case, before, all_tables, image, details) in enumerate(derived):
        streams = []
        recorded = getters['cases'][ordinal]
        assert recorded['id'] == case['id'] and recorded['matched'] is True
        assert recorded['whole_image_before_getters'] is True
        for offset, label in enumerate(['getter', 'maps']):
            index = ordinal * 2 + offset
            call = getters['commands'][index]
            assert call['argv'] == getter_binding['commands'][index]['argv']
            assert call['label'] == getter_binding['commands'][index]['label'] == case['id'] + '-' + label
            assert call['argv'][:3] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8']
            assert call['timeout_seconds'] == 30 and call['stream_limit_bytes'] == 16 * 1024 * 1024
            assert pins.check(call['stderr']).read_bytes() == b''
            streams.append(pins.check(call['stdout']).read_text())
        result = model.verify_fresh(image, details, all_tables, case['table'], *streams)
        assert recorded['proof'] == result and result['verified']
        results.append({'id': case['id'], 'whole_image': True, 'fresh': result,
                        'source_live_rows': details['source_live_rows'], 'final_live_rows': details['final_live_rows'],
                        'history_before': details['history_before'], 'history_after': details['history_after'],
                        'new_ROWS': details['new_ROWS'], 'fresh_INDEX_creation_order': details['fresh_INDEX_creation_order']})
    git = git_objects(binding) if args.verify_git else {'verified': False, 'metadata_child_processes': 0}
    pins.terminal()
    return {'verified': True, 'available': True, 'scope': 'Four engineering recursive INSERT candidates; no native Access save claim',
            'source_commands': 17, 'producer_commands': 4, 'fresh_readonly_commands': 8,
            'public_reference': public_reference,
            'complete_primary_key_ROW_pairs': sum(case[4]['final_key_ROW_pairs'] for case in derived),
            'preserved_old_ROWS': sum(case[4]['source_old_ROWS_preserved'] for case in derived),
            'all_table_fresh_rows': sum(case['fresh']['typed_rows'] for case in results),
            'cases': results, 'pin_occurrences': pins.count, 'distinct_paths': len(pins.items),
            'Git': git, 'native_JVM_Cargo_calls': 0}


def portable_missing(filename):
    try:
        return Path(filename).resolve().relative_to(ROOT).as_posix()
    except (TypeError, ValueError):
        return 'required private evidence'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preflight-only', action='store_true')
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--producer-report', default='.cache/ace16-insert-recursive-candidates-first-20261028/report.json')
    parser.add_argument('--producer-report-sha256', default='c5875beda8a3c65db08ec77e02ee323d97a15789fdfd2ad61f04545277d13cba')
    parser.add_argument('--getter-report', default='.cache/ace16-insert-recursive-getters-first-20261028/report.json')
    parser.add_argument('--getter-report-sha256', default='5097cf9fe2ee7d792a0e82e0ecf1d2dfa82ddfad9707f0fcaeba75f804831a24')
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
