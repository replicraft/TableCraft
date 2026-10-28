"""Saved candidate and historical-build binding, separate from source modeling."""
import hashlib
from pathlib import Path
import subprocess

from ace16_insert_root_split import reference

PRODUCER = '.cache/ace16-insert-root-split-candidates-next-20261027'
GETTERS = '.cache/ace16-insert-root-split-getters-next-20261027'
COMPILED = '0739bc27f6eae90b15942e6171bd61aa1ba171e1'
RECEIPT_REVISION = '10262cc2732008fceaff37c3c2789ea4cdf1d4a0'
NOTICE = b'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
FIXED = {
    PRODUCER + '/protocol-freeze.json': '2bdcdcd97e42bfbcd629e9550b5a6bb69b0e3e042d89066055fd534eadcf6652',
    PRODUCER + '/binding-first.json': '6b0bc7c5d610d99b18d6c847a3eb1f99bab63aaf7d4c23d72d65d9b1a05619c2',
    PRODUCER + '/first-actual/report.json': '1e12ecb7c9e3440e38513a023fb6261d775e7038ce095599e2bb91615ca4ea0e',
    GETTERS + '/freeze.json': '249e641ab3c45b25162371e6575a6e51178e20fcc3750f2cfafb13f925f8c5d4',
    GETTERS + '/controller-freeze.json': '15efe1ec0e651b20f69926672c418af774de851fbdcef3d80054f67a1895977d',
    GETTERS + '/binding-first.json': '29ef42a34e10e5c423690b01d706e6604c713a7740ab643b0de18fbdcc6e4e00',
    GETTERS + '/first-actual/report.json': '8c29b22f1d4d44c97c737b3218e33c79068784171a05902eab6c5a5244407298',
}


def reports(store, pins, prefix, count, stream_limit):
    report = store.load(prefix + '/first-actual/report.json')
    assert report['completed'] is True and report.get('failure') is None
    assert report['after_pin_errors'] == []
    assert len(report['commands']) == count and len(report['cases']) == 4
    pins.nested(report)
    for command in report['commands']:
        assert command['status'] == 0 and command['stopped_for'] is None
        assert command['capture_errors'] == []
        assert command['timeout_seconds'] == 30
        assert command['stream_limit_bytes'] == stream_limit
        assert command['observed_stream_bytes'] == [
            command['stdout']['bytes'], command['stderr']['bytes'],
        ]
        assert max(command['observed_stream_bytes']) <= stream_limit
    return report


def historical_path(token, recorded_root, expected):
    actual = Path(token)
    assert actual.is_absolute()
    assert actual.relative_to(recorded_root).as_posix() == expected


def dependency_info(store, pins, receipt, recorded_root):
    archived = receipt['dependency_info']
    assert archived['path'] == '.cache/ace16-insert-root-split-consumer-20261027/committed-dependency-info.d'
    text = pins.check(archived).read_text()
    assert len(text.splitlines()) == 1
    target, paths = text.strip().split(': ', 1)
    historical_path(target, recorded_root, 'target/debug/examples/preview_ace16_insert')
    assert '\\ ' not in paths
    dependencies = {Path(path).relative_to(recorded_root).as_posix() for path in paths.split()}
    assert len(dependencies) == 737
    sources = {item['path'] for item in receipt['source_pins']}
    assert dependencies <= sources
    remaining = sources - dependencies
    assert len(remaining) == 12
    assert all(path in ('Cargo.lock', 'Cargo.toml', 'rust-toolchain.toml')
               or path.endswith('/Cargo.toml') for path in remaining)
    return {
        'verified': True,
        'archived_path': archived['path'],
        'dependency_source_paths': len(dependencies),
        'additional_manifest_lock_toolchain_paths': len(remaining),
        'disposable_target_dependency_info_required': False,
    }


def git_objects(root, binding, receipt):
    """Verify historical receipt and sources in one bounded local Git process."""
    expected = [binding['build_receipt']] + receipt['source_pins']
    queries = [RECEIPT_REVISION + ':' + expected[0]['path']]
    queries.extend(COMPILED + ':' + item['path'] for item in expected[1:])
    assert len(queries) == 750
    assert all('\n' not in query and '\r' not in query for query in queries)
    assert sum(item['bytes'] for item in expected) < 16 * 1024 * 1024
    result = subprocess.run(
        ['git', '--no-replace-objects', 'cat-file', '--batch'],
        cwd=root,
        input=('\n'.join(queries) + '\n').encode(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=30,
        check=True,
    )
    position = 0
    for query, item in zip(queries, expected, strict=True):
        stop = result.stdout.index(b'\n', position)
        header = result.stdout[position:stop].split()
        if len(header) == 2 and header[1] == b'missing':
            raise FileNotFoundError('Required historical Git object is unavailable: ' + query)
        assert len(header) == 3 and header[1] == b'blob', query
        size = int(header[2])
        begin = stop + 1
        end = begin + size
        blob = result.stdout[begin:end]
        assert size == item['bytes'] and len(blob) == size, query
        assert hashlib.sha256(blob).hexdigest() == item['sha256'], query
        assert result.stdout[end:end + 1] == b'\n'
        position = end + 1
    assert position == len(result.stdout)
    return {'verified': True, 'source_objects': 749, 'receipt_objects': 1, 'processes': 1}


def verify(root, store, pins, plan, derived, verify_git):
    for path, expected in FIXED.items():
        pins.fixed(path, expected)
    for path in [
        PRODUCER + '/protocol-freeze.json',
        GETTERS + '/freeze.json',
        GETTERS + '/controller-freeze.json',
    ]:
        frozen = store.load(path)
        assert frozen['inputs']
        pins.nested(frozen['inputs'])
    native = reports(store, pins, PRODUCER, 4, 8 * 1024 * 1024)
    getters = reports(store, pins, GETTERS, 8, 16 * 1024 * 1024)
    identifiers = [case['id'] for case in plan['cases']]
    assert [case['id'] for case in native['cases']] == identifiers
    assert [case['id'] for case in getters['cases']] == identifiers

    binding = store.load(PRODUCER + '/binding-first.json')
    assert binding['revision'] == COMPILED
    assert binding['receipt_revision'] == RECEIPT_REVISION
    assert binding['protocol_freeze'] == store.pin(PRODUCER + '/protocol-freeze.json')
    pins.check(binding['artifact'])
    receipt = store.load(pins.check(binding['build_receipt']).relative_to(root))
    assert receipt['exit_code'] == 0 and receipt['consumer'] == 'preview_ace16_insert'
    assert receipt['compiled_revision'] == COMPILED
    assert receipt['frozen_example'] == binding['artifact']
    assert receipt['source_pins'] == binding['compiled_sources']
    assert len(receipt['source_pins']) == 749
    pins.check(receipt['build_log'])

    getter_binding = store.load(GETTERS + '/binding-first.json')
    for name, path in [
        ('getter_protocol_freeze', GETTERS + '/freeze.json'),
        ('controller_freeze', GETTERS + '/controller-freeze.json'),
        ('producer_report', PRODUCER + '/first-actual/report.json'),
        ('producer_protocol_freeze', PRODUCER + '/protocol-freeze.json'),
    ]:
        assert getter_binding[name] == store.pin(path)
    assert getter_binding['producer_report'] in getters['before']
    producer_protocol = store.load(PRODUCER + '/protocol.json')
    getter_protocol = store.load(GETTERS + '/protocol.json')
    assert [case['id'] for case in producer_protocol['cases']] == identifiers
    assert [case['id'] for case in getter_protocol['cases']] == identifiers
    assert len(getter_binding['candidates']) == 4

    parts = len(Path(binding['artifact']['path']).parts)
    recorded_root = Path(native['commands'][0]['argv'][0]).parents[parts - 1]
    dependency = dependency_info(store, pins, receipt, recorded_root)
    for ordinal, (original, state, expected, details) in enumerate(derived):
        planned = plan['cases'][ordinal]
        actual = native['cases'][ordinal]
        configured = producer_protocol['cases'][ordinal]
        command = native['commands'][ordinal]
        identifier = planned['id']
        assert actual['id'] == identifier and actual['matched'] is True
        assert actual.get('failure') is None and command['label'] == identifier
        assert configured['source'] == planned['source']
        assert configured['expected_image'] == planned['expected_image']
        assert pins.check(configured['request']).read_bytes() == (
            store.path(planned['request_tsv']['path']).read_bytes()
        )
        argv = command['argv']
        assert len(argv) == 5 and argv[3] == planned['table']
        for token, path in [
            (argv[0], binding['artifact']['path']),
            (argv[1], planned['source']['path']),
            (argv[2], actual['candidate']['path']),
            (argv[4], configured['request']['path']),
        ]:
            historical_path(token, recorded_root, path)
        assert pins.check(command['stdout']).read_bytes() == b'candidate_changed_rows=1\n'
        assert pins.check(command['stderr']).read_bytes() == NOTICE
        assert pins.check(actual['candidate']).read_bytes() == expected
        assert getter_binding['candidates'][ordinal] == {
            'id': identifier, 'image': actual['candidate'],
        }

        # Whole-image equality precedes interpreting either fresh getter stream.
        assert getters['cases'][ordinal] == {
            'id': identifier, 'whole_image_before_getters': True, 'matched': True,
        }
        observed = getter_protocol['cases'][ordinal]
        assert observed['source'] == planned['source']
        assert observed['source_getter'] == planned['source_getter']
        assert observed['source_maps'] == planned['source_maps']
        assert observed['expected_image'] == planned['expected_image']
        assert observed['inserted_ID'] == planned['request']['ID']
        assert observed['inserted_binary_hex'] == planned['request']['binary_hex']
        assert observed['new_page'] == details['new_page']
        assert observed['new_slot'] == details['new_slot']
        assert observed['final_count'] == 452
        streams = []
        for offset, label in enumerate(('getter', 'maps')):
            call = getters['commands'][2 * ordinal + offset]
            candidate = str(recorded_root / actual['candidate']['path'])
            assert call['label'] == identifier + '-' + label
            assert call['argv'] == [candidate if token == 'CANDIDATE' else token
                                    for token in observed['commands'][offset]]
            assert call['argv'][:3] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8']
            assert pins.check(call['stderr']).read_bytes() == b''
            output = pins.check(call['stdout']).read_bytes()
            reference_key = 'expected_getter' if offset == 0 else 'expected_maps'
            assert output == pins.check(observed[reference_key]).read_bytes()
            streams.append(pins.check(call['stdout']))

        candidate_state = reference.inspect(original, pins.check(actual['candidate']), *streams, after=True)
        transition = reference.transition(original, state, candidate_state)
        assert transition['new_reference'] == details['new_ROW']
        assert transition['index_pages_added'] == sorted([details['RIGHT_leaf'], details['LEFT_leaf']])
        assert transition['data_pages_added'] == details['DATA_pages']
        assert candidate_state[1]['count'] == len(candidate_state[1]['rows']) == 452

    git = git_objects(root, binding, receipt) if verify_git else {
        'verified': False, 'source_objects': 0, 'receipt_objects': 0, 'processes': 0,
    }
    return {
        'status': 'passed',
        'verified': True,
        'producer_commands': 4,
        'fresh_getter_commands': 8,
        'whole_image_matches': 4,
        'complete_key_ROW_entries': 1808,
        'old_ROWS_preserved': 1804,
        'compiled_revision': COMPILED,
        'receipt_revision': RECEIPT_REVISION,
        'artifact': binding['artifact'],
        'dependency_info': dependency,
        'Git': git,
    }
