"""Recheck accepted bytes and cached observations without native/JVM calls."""
import hashlib
import json
from pathlib import Path
import subprocess

from index_split_originals import capture
from root_split_preview_originals import proof


def pins(value, location=()):
    if location == ('build', 'source_pins'):
        return
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            yield value
        for name, item in value.items():
            yield from pins(item, (*location, name))
    elif isinstance(value, list):
        for item in value:
            yield from pins(item, location)


def verify_pin(expected):
    assert capture.pin(expected['path']) == expected


def canonical(value):
    return json.loads(json.dumps(value))


def cached(state):
    verify_pin(state['file'])
    for name in ['getter', 'map_probe']:
        command = state[name]
        assert command['status'] == 0 and not command['timed_out']
        verify_pin(command['stdout'])
        verify_pin(command['stderr'])
        assert not capture.common.path(command['stderr']['path']).read_bytes()
    raw = capture.common.path(state['file']['path']).read_bytes()
    metadata, relations = capture.parse(capture.common.path(state['getter']['stdout']['path']).read_text())
    descriptions = capture.maps.parse(capture.common.path(state['map_probe']['stdout']['path']).read_text())
    decoded_maps = {}
    rows = {}
    for name, description in descriptions['maps'].items():
        decoded_maps[name] = capture.maps.decode_map(raw, description, capture.PAGE_BYTES, len(raw) // capture.PAGE_BYTES)
    for name, table in metadata.items():
        rows[name] = {}
        for identity, location in table['locations'].items():
            rows[name][str(identity)] = capture.body(raw, *location)
    computed = {
        'file': capture.pin(state['file']['path']),
        'getter': state['getter'],
        'map_probe': state['map_probe'],
        'metadata': metadata,
        'relations': relations,
        'map_info': descriptions,
        'maps': decoded_maps,
        'geometry': capture.geometry(raw, metadata, descriptions),
        'raw_rows': rows,
    }
    assert canonical(computed) == state
    return raw


def run(report_path, output_path):
    report_path = capture.common.path(report_path)
    output_path = capture.common.path(output_path)
    output_path.relative_to(capture.ROOT / '.cache')
    assert not output_path.exists(), 'Use a fresh verification receipt'
    report_pin = capture.pin(report_path)
    report = json.loads(report_path.read_text())
    assert report['completed'] == 2 and report['fatal'] is None
    assert report['immutable_inputs_unchanged'] and report['command_count'] == 6
    checked = 0
    for expected in pins(report):
        verify_pin(expected)
        checked += 1
    revision = report['build']['compiled_revision']
    git_objects = 0
    for source in report['build']['source_pins']:
        process = subprocess.run(['git', 'show', revision + ':' + source['path']], cwd=capture.ROOT,
                                 capture_output=True, check=True, timeout=30)
        assert len(process.stdout) == source['bytes']
        assert hashlib.sha256(process.stdout).hexdigest() == source['sha256']
        git_objects += 1
    snapshot = json.loads(capture.common.path(report['before_first_call']['path']).read_text())
    for expected in pins(snapshot):
        verify_pin(expected)
        checked += 1
    plan_source = next(item['executed_source'] for item in snapshot['sources']
                       if item['executed_source']['path'].endswith('/plan.json'))
    frozen_plan = json.loads(capture.common.path(plan_source['path']).read_text())
    public_report_pin = frozen_plan['immutable'][0]
    verify_pin(public_report_pin)
    public_report = json.loads(capture.common.path(public_report_pin['path']).read_text())
    cases = []
    for case in report['cases']:
        assert case['complete'] and 'error' not in case
        selector = case['selector']
        verify_pin(selector['original'])
        original = capture.common.path(selector['original']['path']).read_bytes()
        actual = cached(case['getter_state'])
        facts = proof.prove(selector, original, actual)
        assert facts == case['whole_image_proof']
        assert capture.pin(case['candidate']['path']) == case['candidate']
        assert case['candidate']['sha256'] == facts['whole_expected_image_sha256']
        reference_case = next(item for item in public_report['cases'] if item['profile'] == case['profile'])
        reference = reference_case['after']
        verify_pin(reference['file'])
        assert case['getter_state']['relations'] == reference['relations']
        for name, table in case['getter_state']['metadata'].items():
            assert table == reference['metadata'][name]
            for root, tree in case['getter_state']['geometry'][name]['trees'].items():
                assert tree['entry_hex'] == reference['geometry'][name]['trees'][root]['entry_hex']
        assert case['reference_agreement']['all_typed_rows_locations_logical_indexes_full_key_row_streams_agree']
        command = case['command']
        assert command['status'] == 0 and not command['timed_out']
        request = json.loads(capture.common.path(case['request']['path']).read_text())
        assert request['argv'] == command['argv']
        assert command['argv'][3:] == selector['candidate_arguments_after_source_output']
        cases.append({'profile': case['profile'], 'candidate': case['candidate'],
                      'whole_image_and_cached_getter_map_proof': True})
    assert capture.pin(report_path) == report_pin
    result = {'report': report_pin, 'native_or_jvm_calls': 0, 'read_only_git_object_checks': git_objects,
              'checked_artifact_pins': checked, 'verified_candidates': len(cases), 'cases': cases}
    output_path.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'verification': capture.pin(output_path), 'verified_candidates': len(cases),
                      'native_or_jvm_calls': 0}))
    return 0
