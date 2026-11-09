"""Recheck saved non-root images/getters/maps without native or JVM calls."""
import hashlib
import json
import subprocess

from nonroot_index_split_originals import capture as evidence
from nonroot_split_preview_originals import proof
from slot_preview_originals import common


def verify(pin):
    assert common.pin(pin['path']) == pin


def pins(value, route=()):
    if route == ('build', 'source_pins'):
        return
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            yield value
        for name, item in value.items():
            yield from pins(item, (*route, name))
    elif isinstance(value, list):
        for item in value:
            yield from pins(item, route)


def canonical(value):
    return json.loads(json.dumps(value))


def cached(state):
    verify(state['file'])
    for name in ['getter', 'map_probe']:
        command = state[name]
        assert command['status'] == 0 and not command['timed_out']
        verify(command['stdout'])
        verify(command['stderr'])
        assert not common.path(command['stderr']['path']).read_bytes()
    raw = common.path(state['file']['path']).read_bytes()
    metadata = evidence.trees.parse(common.path(state['getter']['stdout']['path']).read_text())
    descriptions = evidence.maps.parse(common.path(state['map_probe']['stdout']['path']).read_text())
    assert canonical(metadata) == state['metadata']
    assert canonical(descriptions) == state['map_info']
    assert canonical(evidence.geometry(raw, metadata, descriptions)) == state['geometry']
    for name, description in descriptions['maps'].items():
        actual = evidence.maps.decode_map(raw, description, 4096, len(raw) // 4096)
        assert canonical(actual) == state['maps'][name]
    for name, table in metadata.items():
        assert canonical(evidence.trees.pages(raw, table)) == state['decoded_pages'][name]
        for identity, location in table['locations'].items():
            assert evidence.body(raw, *location) == state['raw_rows'][name][str(identity)]
    return raw


def run(report_path, output_path):
    output = common.path(output_path)
    output.relative_to(common.ROOT / '.cache')
    assert not output.exists(), 'Use a fresh read-only verification receipt'
    report_pin = common.pin(report_path)
    report = json.loads(common.path(report_path).read_text())
    assert report['completed'] == 2 and report['fatal'] is None
    assert report['command_count'] == 6 and report['immutable_inputs_unchanged']
    checked = 0
    for pin in pins(report):
        verify(pin)
        checked += 1
    snapshot = json.loads(common.path(report['before_first_call']['path']).read_text())
    for pin in snapshot['immutable']:
        verify(pin)
        checked += 1
    git_objects = 0
    for source in report['build']['source_pins']:
        process = subprocess.run(['git', 'show', report['build']['compiled_revision'] + ':' + source['path']],
                                 cwd=common.ROOT, check=True, capture_output=True, timeout=30)
        assert len(process.stdout) == source['bytes']
        assert hashlib.sha256(process.stdout).hexdigest() == source['sha256']
        git_objects += 1
    frozen = next(item['source'] for item in snapshot['archives'] if item['source']['path'].endswith('/plan.json'))
    plan = json.loads(common.path(frozen['path']).read_text())
    verify(plan['source'])
    reference = json.loads(common.path(plan['source']['path']).read_text())
    results = []
    for case, observed in zip(report['cases'], reference['cases'], strict=True):
        assert case['complete'] and 'error' not in case
        selector = case['selector']
        raw = cached(case['after'])
        original = common.path(selector['original']['path']).read_bytes()
        facts = proof.prove(selector, original, raw)
        assert facts == case['whole_image_proof']
        expected_metadata = observed['after']['metadata']
        assert case['after']['metadata'] == expected_metadata
        for name, owner in case['after']['geometry'].items():
            for root, tree in owner['trees'].items():
                assert tree['entry_hex'] == observed['after']['geometry'][name]['trees'][root]['entry_hex']
        request = json.loads(common.path(case['request']['path']).read_text())
        assert request['argv'] == case['command']['argv']
        assert request['argv'][3:] == ['Tree', 'ID', '27', '1000', '2147483647']
        results.append({'profile': case['profile'], 'candidate': case['candidate'],
                        'whole_image_cached_getters_maps': True})
    assert common.pin(report_path) == report_pin
    result = {'report': report_pin, 'verified_candidates': 2, 'checked_artifact_pins': checked,
              'read_only_Git_objects': git_objects, 'native_or_jvm_calls': 0, 'cases': results}
    output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'verification': common.pin(output), 'verified_candidates': 2, 'native_or_jvm_calls': 0}))
    return 0
