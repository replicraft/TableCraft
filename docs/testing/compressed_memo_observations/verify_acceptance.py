"""Recheck saved148 candidate proofs without native/JVM or database writes."""
import argparse
import csv
import hashlib
import importlib.util
import json
from collections import Counter
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from compressed_memo_observations import capture

MODEL_SHA256 = '499025d5304606b6c3177ba40865b64863dd5aa74bb65c4e62423f97a8960aeb'
NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'


def pin(path):
    path = (ROOT / path).resolve()
    path.relative_to(ROOT)
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'bytes': len(data),
            'sha256': hashlib.sha256(data).hexdigest()}


def collect(value, result):
    if isinstance(value, dict):
        if {'path', 'bytes', 'sha256'} <= value.keys():
            expected = {key: value[key] for key in ('path', 'bytes', 'sha256')}
            if expected['path'] in result:
                assert result[expected['path']] == expected
            result[expected['path']] = expected
        for key, child in value.items():
            # Compiled objects belong to immutable Git, never current source.
            if key != 'historical_git_objects':
                collect(child, result)
    elif isinstance(value, list):
        for child in value:
            collect(child, result)


def load_model(path):
    assert pin(path)['sha256'] == MODEL_SHA256
    spec = importlib.util.spec_from_file_location('reviewed_compressed_model', ROOT / path)
    model = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(model)
    return model


def verify(report_path, verify_git=False):
    report = json.loads((ROOT / report_path).read_text())
    assert report['attempted'] == 148 and report['matched_candidates'] == 132
    assert report['matched_refusals'] == 16 and report['unmet_controls'] == 0
    assert report['fatal'] is None and report['immutable_inputs_unchanged']
    assert report['command_count'] == len(report['commands']) == 280
    before_call = json.loads((ROOT / report['before_first_call']['path']).read_text())
    archives = {Path(item['source']['path']).name: item for item in before_call['archives']}
    model = load_model(archives['model.py']['source']['path'])
    plan = json.loads((ROOT / archives['plan.json']['source']['path']).read_text())
    states = json.loads((ROOT / plan['independent_facts']['path']).read_text())
    freeze = json.loads((ROOT / archives['freeze.json']['source']['path']).read_text())
    runner_freeze = json.loads((ROOT / archives['runner-freeze.json']['source']['path']).read_text())
    pins = {}
    for value in [report, before_call, plan, freeze, runner_freeze]:
        collect(value, pins)
    for expected in pins.values():
        assert pin(expected['path']) == expected

    objects = report['build']['historical_git_objects']
    if verify_git:
        revision = report['build']['compiled_revision']
        for expected in objects:
            result = subprocess.run(['git', 'show', revision + ':' + expected['path']],
                                    cwd=ROOT, capture_output=True, check=True, timeout=30)
            assert len(result.stdout) == expected['bytes']
            assert hashlib.sha256(result.stdout).hexdigest() == expected['sha256']

    counts = Counter()
    modes = Counter()
    parent_modes = Counter()
    global_clears = 0
    old_chunks = 0
    map_proofs = 0
    for selector, actual in zip(plan['cases'], report['cases'], strict=True):
        assert selector == actual['selector'] and actual['complete'] and 'failure' not in actual
        before = states[selector['source_facts_key']]
        source = ROOT / selector['source']['path']
        raw, checked = capture.snapshot(source, ROOT / before['getter']['path'], ROOT / before['metadata']['path'])
        assert json.loads(json.dumps(checked)) == before
        native = actual['native']
        request = ROOT / native['argv'][5]
        assert request.read_bytes() == (ROOT / selector['request']['path']).read_bytes()
        with request.open(newline='', encoding='utf-8') as input_file:
            rows = list(csv.reader(input_file, delimiter='\t'))
        assert len(rows) == 1 and len(rows[0]) == 5 and rows[0][0] == '0'
        old = before['fields'][selector['column']]['logical']
        assert rows[0][1:3] == (['null', ''] if old is None else ['text', old])
        requested = None if rows[0][3] == 'null' else rows[0][4]
        assert requested is None or requested.encode('utf-8') == (ROOT / selector['payload']['path']).read_bytes()
        assert not native['timed_out']
        argv = native['argv']
        assert len(argv) == 6 and argv[0] == report['build']['artifact']['path']
        assert argv[1] == selector['source']['path']
        assert argv[3:] == [selector['table'], selector['column'], str(request.relative_to(ROOT))]
        candidate = ROOT / argv[2]
        assert candidate.parent == (ROOT / report_path).parent
        stdout = (ROOT / native['stdout']['path']).read_bytes()
        stderr = (ROOT / native['stderr']['path']).read_text()
        canonical = model.encode(before['table']['columns'][selector['column']], requested)
        refuse = old != requested and canonical is not None and capture.decode_stream(canonical) != requested
        if refuse:
            assert selector['expected']['outcome'] == 'atomic_refusal'
            assert native['status'] == 1 and stdout == b'' and stderr == 'Error: Unsupported(Column)\n'
            assert actual['atomic_no_output'] and not candidate.exists()
            assert 'getter' not in actual and 'candidate' not in actual
            counts['typed_atomic_refusals'] += 1
            continue

        assert selector['expected']['outcome'] == 'candidate'
        expected, facts = model.expected(raw, before, selector['column'], requested, before['table']['definition'])
        for key, value in facts.items():
            assert selector['expected'][key] == value
        assert facts == actual['whole_image_proof']
        assert len(expected) == selector['expected']['candidate_bytes']
        assert hashlib.sha256(expected).hexdigest() == selector['expected']['candidate_sha256']
        assert native['status'] == 0 and stderr == NOTICE
        assert stdout == b'candidate_changed_rows=1\n'
        assert candidate.read_bytes() == expected
        assert pin(candidate) == actual['candidate']
        getter = actual['getter']
        assert getter['status'] == 0 and not getter['timed_out']
        assert getter['argv'][0:3] == ['java', '-Xmx512m', '-Dstdout.encoding=UTF-8']
        assert getter['argv'][-2:] == ['inspect', argv[2]]
        assert (ROOT / getter['stderr']['path']).read_bytes() == b''
        saved_getter = ROOT / getter['stdout']['path']
        after = capture.snapshot(candidate, saved_getter, saved_getter)
        assert json.loads(json.dumps(after[1])) == actual['after']
        selection = {'column': selector['column'], 'kind': rows[0][3], 'normalization_probe': False}
        comparison = capture.compare((raw, checked), after, selection, (ROOT / selector['payload']['path']).read_bytes())
        assert comparison.pop('public_update_completed') is True
        comparison['candidate_completed'] = True
        assert comparison == actual['comparison']
        assert comparison['requested_value_equal']
        for chunk in comparison['old_chunk_bodies']:
            assert chunk['body_unchanged'] and chunk['slot_word_before'] == chunk['slot_word_after']
            old_chunks += 1
        for proof in comparison['map_wire_proofs'].values():
            assert not proof['representation_changed'] and proof['exact_bit_only_claim']
            map_proofs += 1
        global_clears += len(comparison['map_deltas']['GlobalFree']['removed'])
        modes[facts['storage']] += 1
        parent_modes[facts['parent']] += 1
        counts['complete_model_and_getter_candidates'] += 1

    assert counts == {'complete_model_and_getter_candidates': 132, 'typed_atomic_refusals': 16}
    return {'report': pin(report_path), 'counts': dict(counts), 'commands': 280,
            'recursive_live_pins_verified': len(pins), 'historical_git_objects': len(objects),
            'git_objects_reverified_this_run': verify_git, 'storage_modes': dict(modes),
            'parent_modes': dict(parent_modes), 'source_paths': len(states), 'frozen_requests': 148,
            'old_chunk_body_and_slot_comparisons': old_chunks, 'exact_stable_map_wire_proofs': map_proofs,
            'global_free_clears': global_clears, 'unmet_controls': 0,
            'scope': 'Read-only saved-output/hash/model verification; no native/JVM/database-write calls.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verify-git', action='store_true')
    arguments = parser.parse_args()
    output = (ROOT / arguments.output).resolve()
    output.relative_to(ROOT)
    assert not output.exists(), 'Use a fresh verification output'
    result = verify(arguments.report, arguments.verify_git)
    output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main()
