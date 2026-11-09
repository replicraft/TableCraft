#!/usr/bin/env python3
"""Frozen example versus original associations, public action copies and raw preservation."""
import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path

import cascade_delete_model as MODEL

ROOT = Path.cwd()
REFERENCE = ROOT / '.cache/ace16-delete-cascade-next-20261026'
REFERENCE_SUMMARY_SHA256 = '626ea595b000fcc8b32fd6fcec3b87c54c00c11b8c5224b1dc15f156342f0199'
JAR_SHA256 = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
NOTICE = 'Engineering candidate: data-macro semantics and native Access qualification pending.\n'
TIMEOUT_SECONDS = 30


def digest(data):
    return hashlib.sha256(data).hexdigest()


def label(path):
    return str(Path(path).resolve().relative_to(ROOT))


def pin(path):
    path = Path(path)
    data = path.read_bytes()
    return {'path': label(path), 'bytes': len(data), 'sha256': digest(data)}


def check_pin(record):
    assert pin(ROOT / record['path']) == record


def compiled_sources(receipt, revision):
    assert receipt['compiled_revision'] == revision
    result = []
    for expected in receipt['source_pins']:
        source = subprocess.run(
            ['git', 'show', revision + ':' + expected['path']], cwd=ROOT,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            timeout=TIMEOUT_SECONDS, check=True,
        ).stdout
        actual = {'path': expected['path'], 'bytes': len(source), 'sha256': digest(source)}
        assert actual == expected
        result.append(actual)
    return result


class Replay:
    def __init__(self, work):
        self.work = work
        self.commands = []

    def command(self, argv):
        number = len(self.commands) + 1
        stdout = self.work / f'command-{number:03d}.stdout'
        stderr = self.work / f'command-{number:03d}.stderr'
        timed_out = False
        with stdout.open('xb') as out, stderr.open('xb') as err:
            try:
                process = subprocess.run(
                    argv, cwd=ROOT, stdout=out, stderr=err,
                    timeout=TIMEOUT_SECONDS, check=False,
                )
                status = process.returncode
            except subprocess.TimeoutExpired:
                status = None
                timed_out = True
        result = {
            'argv': argv,
            'status': status,
            'timed_out': timed_out,
            'timeout_seconds': TIMEOUT_SECONDS,
            'stdout': pin(stdout),
            'stderr': pin(stderr),
        }
        self.commands.append(result)
        with (self.work / 'commands.jsonl').open('a') as output:
            output.write(json.dumps(result) + '\n')
        return result

    def getters(self, path, classpath):
        ordinary = self.command([
            'java', '-Dstdout.encoding=UTF-8', '-cp', classpath,
            'CascadeDeleteFixture', 'oracle', label(path),
        ])
        indexes = self.command([
            'java', '-Dstdout.encoding=UTF-8', '-cp', classpath,
            'CascadeDeleteIndexInspection', label(path),
        ])
        for command in (ordinary, indexes):
            assert command['status'] == 0 and not command['timed_out']
            assert command['stderr']['bytes'] == 0
        metadata = MODEL.parse_rows((ROOT / ordinary['stdout']['path']).read_text())
        orders = MODEL.parse_orders((ROOT / indexes['stdout']['path']).read_text())
        return metadata, orders, [ordinary, indexes]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--example', type=Path, required=True)
    parser.add_argument('--example-sha256', required=True)
    parser.add_argument('--compiled-revision', required=True)
    parser.add_argument('--build-receipt', type=Path, required=True)
    parser.add_argument('--jar', type=Path, required=True)
    parser.add_argument('--workdir', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    for name in ('example', 'build_receipt', 'jar', 'workdir', 'report'):
        setattr(args, name, getattr(args, name).resolve())
    assert not args.workdir.exists() and not args.report.exists()
    args.workdir.mkdir(parents=True)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    replay = Replay(args.workdir)
    report = {
        'scope': '24 acyclic candidate/model/public comparisons and eight held self/cycle typed refusals; no native Access SQL qualification.',
        'compiled_revision': args.compiled_revision,
        'build_receipt': pin(args.build_receipt),
        'frozen_example': pin(args.example),
        'cases': [],
    }
    immutable = []
    requests = []
    fatal = None
    try:
        assert report['frozen_example']['sha256'] == args.example_sha256
        build = json.loads(args.build_receipt.read_text())
        assert build['frozen_example'] == report['frozen_example']
        report['compiled_source_pins_before'] = compiled_sources(build, args.compiled_revision)
        summary_path = REFERENCE / 'public-action-observations-initial.json'
        assert pin(summary_path)['sha256'] == REFERENCE_SUMMARY_SHA256
        summary = json.loads(summary_path.read_text())
        for key in ('raw_report', 'original_preparation', 'original_metadata', 'log', 'commands'):
            check_pin(summary[key])
        original_report = json.loads((ROOT / summary['raw_report']['path']).read_text())
        preparation = json.loads((ROOT / summary['original_preparation']['path']).read_text())
        models = json.loads((ROOT / summary['original_metadata']['path']).read_text())
        assert pin(args.jar)['sha256'] == JAR_SHA256
        immutable.extend([args.example, args.jar, args.build_receipt, summary_path])
        immutable.extend(ROOT / summary[key]['path'] for key in (
            'raw_report', 'original_preparation', 'original_metadata', 'log', 'commands'))
        for expected in preparation['tools'] + preparation['inputs']:
            check_pin(expected)
            immutable.append(ROOT / expected['path'])
        for expected in original_report['supplemental_before']:
            check_pin(expected)
            immutable.append(ROOT / expected['path'])
        for case in original_report['cases']:
            check_pin(case['copy_after_readonly_getters'])
            immutable.append(ROOT / case['copy_after_readonly_getters']['path'])
            for name in ('action', 'getter', 'index_getter'):
                for stream in ('stdout', 'stderr'):
                    expected = case[name][stream]
                    check_pin(expected)
                    immutable.append(ROOT / expected['path'])
        immutable.extend([Path(__file__).resolve(), Path(MODEL.__file__).resolve()])
        immutable = list(dict.fromkeys(immutable))
        report['immutable_pins_before'] = [pin(path) for path in immutable]
        classes = args.workdir / 'classes'
        classes.mkdir()
        compiled = replay.command([
            'javac', '-cp', label(args.jar), '-d', label(classes),
            'docs/testing/CascadeDeleteFixture.java',
            'docs/testing/CascadeDeleteIndexInspection.java',
        ])
        assert compiled['status'] == 0 and not compiled['timed_out']
        classpath = label(args.jar) + ':' + label(classes)
        original_models = {model['profile']: model for model in models}
        baselines = {}
        for profile, model in original_models.items():
            source = ROOT / model['input']['path']
            metadata, orders, commands = replay.getters(source, classpath)
            empty = {name: set() for name in metadata[0]}
            proof = MODEL.verify(source.read_bytes(), source.read_bytes(), metadata, metadata, orders, orders, empty)
            baselines[profile] = (source, metadata, orders)
            report.setdefault('original_getter_receipts', {})[profile] = commands
            report.setdefault('original_model_selfchecks', {})[profile] = proof
        for reference in original_report['cases']:
            profile = reference['profile']
            name = profile + '-' + reference['label']
            source, original, original_orders = baselines[profile]
            model = original_models[profile]
            selected = MODEL.selected_roots(model, reference['seed_table'], reference['seed_order'])
            expected_total = sum(len(rows) for rows in selected.values())
            case = {
                'profile': profile,
                'label': reference['label'],
                'scope': reference['scope'],
                'seed_table': reference['seed_table'],
                'seed_order': reference['seed_order'],
                'original_association_selected_roots': {table: sorted(rows) for table, rows in selected.items() if rows},
                'reference_action_status': reference['action']['status'],
                'reference_stack_overflow': reference['stack_overflow'],
                'reference_matches_finite_closure': reference['typed_rows_locations_match_closure'],
                'reference_copy': reference['copy_after_readonly_getters'],
            }
            report['cases'].append(case)
            candidate = args.workdir / (name + '.accdb')
            try:
                raw = source.read_bytes()
                records = MODEL.payloads(raw, original[0])[reference['seed_table']]
                identities = list(original[0][reference['seed_table']]['rows'])
                request = args.workdir / (name + '.tsv')
                lines = []
                for identity in reference['seed_order']:
                    ordinal = identities.index(identity)
                    _, _, start, end = records[identity]
                    lines.append(f'{ordinal}\t{raw[start:end].hex()}\n')
                request.write_text(''.join(lines))
                requests.append(pin(request))
                case['request'] = pin(request)
                action = replay.command([
                    label(args.example), label(source), label(candidate), '--cascade',
                    '--group', reference['seed_table'], label(request),
                ])
                case['candidate_command'] = action
                stdout = (ROOT / action['stdout']['path']).read_text()
                stderr = (ROOT / action['stderr']['path']).read_text()
                if reference['scope'] != 'acyclic':
                    assert action['status'] != 0 and not action['timed_out']
                    assert 'ConstraintCoverage' in stderr
                    assert not stdout and not candidate.exists()
                    case['typed_refusal_without_output'] = True
                else:
                    assert action['status'] == 0 and not action['timed_out']
                    assert stdout == f'candidate_deleted_rows={expected_total}\n'
                    assert stderr == NOTICE
                    case['candidate_before_getters'] = pin(candidate)
                    actual, orders, commands = replay.getters(candidate, classpath)
                    case['candidate_getter_receipts'] = commands
                    case['candidate_after_getters'] = pin(candidate)
                    assert case['candidate_before_getters'] == case['candidate_after_getters']
                    case['byte_preservation_proof'] = MODEL.verify(
                        raw, candidate.read_bytes(), original, actual,
                        original_orders, orders, selected,
                    )
                    public_rows = MODEL.parse_rows((ROOT / reference['getter']['stdout']['path']).read_text())
                    public_orders = MODEL.parse_orders((ROOT / reference['index_getter']['stdout']['path']).read_text())
                    assert actual == public_rows
                    assert orders == public_orders
                    case['public_action_typed_rows_index_order_agree'] = True
                case['matches'] = True
            except Exception as error:
                case['matches'] = False
                case['error'] = type(error).__name__ + ': ' + str(error)
            finally:
                if candidate.exists():
                    case['created_candidate'] = pin(candidate)
                args.report.write_text(json.dumps(report, indent=2) + '\n')
            print(name, 'MATCH' if case['matches'] else 'DISAGREEMENT', case.get('error', ''), flush=True)
        report['immutable_pins_after'] = [pin(path) for path in immutable]
        report['requests_before'] = requests
        report['requests_after'] = [pin(ROOT / item['path']) for item in requests]
        report['compiled_source_pins_after'] = compiled_sources(build, args.compiled_revision)
    except Exception as error:
        fatal = type(error).__name__ + ': ' + str(error)
        print('FATAL', fatal, flush=True)
    report['fatal_error'] = fatal
    report['command_count'] = len(replay.commands)
    report['case_count'] = len(report['cases'])
    report['matches'] = sum(case.get('matches', False) for case in report['cases'])
    report['disagreements'] = sum(not case.get('matches', False) for case in report['cases'])
    report['created_candidate_inventory'] = [pin(path) for path in sorted(args.workdir.glob('*.accdb'))]
    report['pins_unchanged'] = report.get('immutable_pins_before') == report.get('immutable_pins_after')
    report['requests_unchanged'] = report.get('requests_before') == report.get('requests_after')
    report['compiled_sources_unchanged'] = report.get('compiled_source_pins_before') == report.get('compiled_source_pins_after')
    args.report.write_text(json.dumps(report, indent=2) + '\n')
    print('TERMINAL', report['case_count'], 'controls', report['matches'], 'matches',
          report['disagreements'], 'disagreements', report['command_count'], 'commands', flush=True)
    if fatal or report['case_count'] != 32 or report['disagreements']:
        return 1
    if not all(report[key] for key in ('pins_unchanged', 'requests_unchanged', 'compiled_sources_unchanged')):
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
