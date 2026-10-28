"""Reprove saved source/reference observations and prospective INSERT images.

This tool reads saved artifacts only. It starts no compiler, native producer,
JVM or fixture process. Optional historical Git checking uses one local batch.
"""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / 'docs/testing'))
from scripts.compat_artifacts import ArtifactStore
from ace16_insert_root_split import acceptance, model, reference

STORE = ArtifactStore(ROOT)
ORACLE = '.cache/ace16-insert-root-split-independent-next-20261027'
PROTOCOL = '.cache/ace16-insert-root-split-reference-next-20261027'
WORK = '.cache/ace16-insert-root-split-reference-first-20261027'
CASE_IDS = ['V2016-fitted', 'V2016-full84', 'V2019-fitted', 'V2019-full84']
MODEL_SHA = '1f98c274b406b7b3602e28647210a24b64f5e49f2eda36bd2b8116dafd4b3a75'
FIXED = {
    ORACLE + '/freeze.json': '6b2f764aad26a4c5de1037b6264835506b007675ff7ad4325e6e45dc6242db7f',
    ORACLE + '/plan.json': '5e0ee62a85cf2f628c08b1009156cee34084666cf59ad8f76b3ff74aae88a773',
    PROTOCOL + '/plan.json': '5c4fd6f10ce82208ec498d14cd5e5df0c0b8b004897552a0e96ab709300a13e7',
    PROTOCOL + '/controller-freeze.json': '6a81dc4fa8b8f4bd5ff00c13a5a6e4a161d8b55f6150d50dcfcf98bb49e52dd6',
    WORK + '/report.json': '817740cf99876188e4bd5aae50babe52f33b173dd0385d36f7481f358bde27e5',
}


class Pins:
    """Fresh checks for every occurrence, followed by a final identity rehash."""

    def __init__(self):
        self.occurrences = 0
        self.identities = {}

    def check(self, item):
        self.occurrences += 1
        prior = self.identities.setdefault(item['path'], item)
        assert prior == item, 'Contradictory saved identity: ' + item['path']
        assert STORE.pin(item['path']) == item, item['path']
        return STORE.path(item['path'])

    def fixed(self, path, expected):
        item = STORE.pin(path)
        assert item['sha256'] == expected, path
        self.check(item)

    def nested(self, value):
        if isinstance(value, dict):
            if set(value) == {'path', 'bytes', 'sha256'}:
                self.check(value)
            else:
                for child in value.values():
                    self.nested(child)
        elif isinstance(value, list):
            for child in value:
                self.nested(child)


def maintained_sources(pins):
    """The maintained copies differ only in their repository-root bootstrap."""
    files = [
        ('model.py', ORACLE + '/model.py'),
        ('reference.py', PROTOCOL + '/proof.py'),
    ]
    for maintained, executed in files:
        private = STORE.pin(executed)
        frozen = pins.check(private).read_text()
        if maintained == 'model.py':
            assert hashlib.sha256(frozen.encode()).hexdigest() == MODEL_SHA
        public_path = 'docs/testing/ace16_insert_root_split/' + maintained
        public = pins.check(STORE.pin(public_path)).read_text()
        assert frozen.count('Path(__file__).resolve().parents[2]') == 1
        assert public == frozen.replace(
            'Path(__file__).resolve().parents[2]',
            'Path(__file__).resolve().parents[3]',
        ), maintained


def source_paths(case, stage):
    return (
        STORE.path(case['source'] if stage == 'before' else case['reference']),
        STORE.path(WORK + '/' + case['id'] + '-' + stage + '-getter.stdout'),
        STORE.path(WORK + '/' + case['id'] + '-' + stage + '-maps.stdout'),
    )


def commands(report, protocol):
    assert report['completed'] is True and report.get('failure') is None
    # The frozen controller retains its last attempted label after completion.
    assert report['active_command'] == report['commands'][-1]['label']
    assert report['after_pin_errors'] == []
    assert report['all_source_gates_passed_before_any_append'] is True
    assert list(report['sources']) == list(report['references']) == CASE_IDS
    assert len(report['commands']) == len(protocol['commands']) == 25
    assert protocol['limits'] == {
        'fixture_processes': 25,
        'source_rows_each': 451,
        'after_rows_each': 452,
        'inspection_rows': 4096,
        'file_bytes': 64 * 1024 * 1024,
        'heap_mib': 512,
        'child_seconds': 30,
        'stream_bytes': 16 * 1024 * 1024,
    }
    for actual, planned in zip(report['commands'], protocol['commands'], strict=True):
        assert actual['argv'] == planned['argv'] and actual['label'] == planned['label']
        assert actual['status'] == 0 and actual['stopped_for'] is None
        assert actual['capture_errors'] == []
        assert actual['timeout_seconds'] == 30
        assert actual['stream_limit_bytes'] == 16 * 1024 * 1024
        assert actual['observed_stream_bytes'] == [
            actual['stdout']['bytes'], actual['stderr']['bytes'],
        ]
        assert max(actual['observed_stream_bytes']) <= 16 * 1024 * 1024
        assert actual['argv'][0] in ('javac', 'java')
        heap = '-J-Xmx512m' if actual['argv'][0] == 'javac' else '-Xmx512m'
        assert heap in actual['argv']
        assert STORE.path(actual['stderr']['path']).read_bytes() == b''


def public_reference(case, state, expected, details, report):
    identifier = case['id']
    before = state[0]
    after = reference.inspect(case, *source_paths(case, 'after'), after=True)
    actual = after[0]
    transition = reference.transition(case, state, after)
    assert transition == STORE.load(WORK + '/' + identifier + '-transition.json')
    assert reference.describe(after) == STORE.load(WORK + '/' + identifier + '-after-gates.json')
    assert report['references'][identifier]['verified'] is True
    assert report['references'][identifier]['image'] == STORE.pin(case['reference'])
    assert len(expected) == len(actual)

    layout = state[2]
    expected_nodes, expected_leaves, expected_keys = model.DECODE.tree(
        expected, layout['root'], layout['definition'],
    )
    actual_nodes, actual_leaves, actual_keys = model.DECODE.tree(
        actual, layout['root'], layout['definition'],
    )
    assert expected_keys == actual_keys and expected_leaves == actual_leaves
    assert len(expected_keys) == 452 and len(expected_nodes) == len(actual_nodes) == 3
    for label in ['global', 'owned', 'hint', 'index_owned']:
        assert model.SOURCE.map_record(expected, layout[label])['members'] == (
            model.SOURCE.map_record(actual, layout[label])['members']
        )
    for field in ['table_count', 'history']:
        at = layout[field]
        assert expected[at:at + 4] == actual[at:at + 4] == (452).to_bytes(4, 'little')
    for row in after[1]['rows'].values():
        start, end = model.SOURCE.frame(actual, row['page'], row['slot'])
        assert expected[start:end] == actual[start:end]

    # Every difference is classified after the complete independent image exists.
    # Public scratch bytes are observations, never new engineering goldens.
    scratch = []
    allowed = {details['RIGHT_leaf'], details['LEFT_leaf']}
    for offset, (left, right) in enumerate(zip(expected, actual, strict=True)):
        if left == right:
            continue
        page, within = divmod(offset, 4096)
        assert page in allowed and within >= 2514, ('Unclassified difference', identifier, offset)
        assert left == 0
        scratch.append(offset)
    assert expected[layout['root'] * 4096 + 493:(layout['root'] + 1) * 4096] == (
        before[layout['root'] * 4096 + 493:(layout['root'] + 1) * 4096]
    )
    return {
        'id': identifier,
        'new_ROW': details['new_ROW'],
        'DATA_pages': details['DATA_pages'],
        'RIGHT_leaf': details['RIGHT_leaf'],
        'LEFT_leaf': details['LEFT_leaf'],
        'expected_image': hashlib.sha256(expected).hexdigest(),
        'public_reference': hashlib.sha256(actual).hexdigest(),
        'public_unused_INDEX_differences': len(scratch),
        'public_difference_offsets': scratch,
        'full_keys_ROWS_records_maps_counters_agree': True,
    }


def verify(verify_git=False):
    pins = Pins()
    for path, expected in FIXED.items():
        pins.fixed(path, expected)
    frozen = STORE.load(ORACLE + '/freeze.json')
    assert len(frozen['inputs']) == 145
    pins.nested(frozen['inputs'])
    maintained_sources(pins)

    protocol = STORE.load(PROTOCOL + '/plan.json')
    controller = STORE.load(PROTOCOL + '/controller-freeze.json')
    report = STORE.load(WORK + '/report.json')
    plan = STORE.load(ORACLE + '/plan.json')
    pins.nested(controller)
    pins.nested(report)
    pins.nested(plan)
    commands(report, protocol)
    assert [case['id'] for case in plan['cases']] == CASE_IDS
    assert [case['id'] for case in protocol['cases']] == CASE_IDS

    # Complete every independent expected image before inspecting reference afters.
    derived = []
    for original, planned in zip(protocol['cases'], plan['cases'], strict=True):
        assert original['id'] == planned['id']
        assert original['request'] == planned['request']
        assert original['source'] == planned['source']['path']
        assert report['sources'][original['id']]['admitted'] is True
        assert report['sources'][original['id']]['image'] == planned['source']
        state = reference.inspect(original, *source_paths(original, 'before'))
        assert reference.describe(state) == STORE.load(WORK + '/' + original['id'] + '-source-gates.json')
        request = planned['request']
        expected, details = model.derive(state, request['ID'], bytes.fromhex(request['binary_hex']))
        assert details == planned['expectation']
        assert expected == STORE.path(planned['expected_image']['path']).read_bytes()
        assert STORE.pin(planned['expected_image']['path']) == planned['expected_image']
        assert STORE.path(planned['request_tsv']['path']).read_bytes() == (
            STORE.path(request['tsv']).read_bytes()
        )
        derived.append((original, state, expected, details))

    cases = [public_reference(case, state, expected, details, report)
             for case, state, expected, details in derived]
    candidates = acceptance.verify(ROOT, STORE, pins, plan, derived, verify_git)
    proof_occurrences = pins.occurrences
    for identity in list(pins.identities.values()):
        pins.check(identity)
    return {
        'available': True,
        'verified': True,
        'reference_commands': 25,
        'admitted_sources': 4,
        'frozen_input_pins': 145,
        'derived_expected_images': 4,
        'complete_key_ROW_entries': 1808,
        'old_ROWS_preserved': 1804,
        'public_unused_INDEX_differences': sum(case['public_unused_INDEX_differences'] for case in cases),
        'proof_pin_occurrences': proof_occurrences,
        'final_rehashed_identities': len(pins.identities),
        'candidate_acceptance': candidates,
        'new_native_JVM_Cargo_fixture_calls': 0,
        'cases': cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        '--verify-git', action='store_true',
        help='Verify 749 historical sources and one receipt in one local Git process',
    )
    args = parser.parse_args()
    try:
        result = verify(args.verify_git)
    except FileNotFoundError as error:
        print(json.dumps({'available': False, 'verified': False, 'reason': str(error),
                          'new_native_JVM_Cargo_fixture_calls': 0}, indent=2))
        return 2
    except (AssertionError, ValueError, KeyError) as error:
        print(json.dumps({'available': True, 'verified': False, 'reason': str(error),
                          'new_native_JVM_Cargo_fixture_calls': 0}, indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
