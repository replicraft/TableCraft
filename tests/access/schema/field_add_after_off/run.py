"""Execute only already frozen predictions, once per fresh-copy SDK mutation."""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess
import time
import model

REPO = Path.cwd()
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--sources-dir', type=Path, required=True)
parser.add_argument('--output-dir', type=Path, required=True)
parser.add_argument('--jar', type=Path, required=True)
parser.add_argument('--java-bin', type=Path, required=True)
args = parser.parse_args()
ROOT = args.output_dir
JAR = args.jar
JAVA = args.java_bin
MODEL_SOURCE = Path(__file__).with_name('model.py')
JAR_HASH = '19a7494a56f439a57fda85c059b019b665802085cec5ae8186d1e028cc1bdfbc'
NUMBER_HELPER = REPO / 'docs/testing/CreatedNumberColumnOracle.java'
TEXT_HELPER = REPO / 'docs/testing/short_text_add/CreatedShortTextColumnOracle.java'
OBSERVER = Path(__file__).with_name('AfterOffProperties.java')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def execute(command, label):
    result = subprocess.run([str(value) for value in command], capture_output=True)
    (ROOT / (label + '.stdout')).write_bytes(result.stdout)
    (ROOT / (label + '.stderr')).write_bytes(result.stderr)
    return {'command': [str(value) for value in command], 'exit': result.returncode}


def actual_database_role(data):
    catalog = data[4 * model.PAGE:5 * model.PAGE]
    slot = model.u16(catalog, 12) - 1
    start = model.u16(catalog, 14 + 2 * slot) & 0x1FFF
    end = model.u16(catalog, 14 + 2 * (slot - 1)) & 0x1FFF
    record = catalog[start:end]
    assert model.u32(record, 2) == 0x0F000004
    assert model.u32(record, 6) == 0x0F000002
    assert model.u16(record, 10) == 2
    assert model.u32(record, 28) == 0x80000000
    tagged = model.u32(record, 44)
    reference = model.u32(record, 48)
    assert tagged == 0x40000098 and reference >> 8 == 10
    return {'catalog_slot': slot, 'property_row': reference & 255,
            'descriptor_hex': record[44:56].hex()}


assert not (ROOT / 'execution.json').exists()
assert digest(JAR.read_bytes()) == JAR_HASH
frozen_bytes = (ROOT / 'predictions-before-sdk.json').read_bytes()
frozen = json.loads(frozen_bytes)
assert digest(MODEL_SOURCE.read_bytes()) == frozen['model_sha256']
for owner in frozen['owners']:
    assert digest((REPO / owner['source']).read_bytes()) == owner['sha256']
report = {'date': '2026-11-09', 'started_unix': time.time(), 'sdk_sha256': JAR_HASH,
          'prediction_sha256': digest(frozen_bytes),
          'observer_sha256': digest(OBSERVER.read_bytes()),
          'runner_sha256': digest(Path(__file__).read_bytes()),
          'expression_evaluation': False, 'cases': []}
report['compile'] = execute([JAVA / 'javac', '-cp', JAR, '-d', ROOT / 'classes', NUMBER_HELPER, TEXT_HELPER, OBSERVER], 'compile')
(ROOT / 'execution.json').write_text(json.dumps(report, indent=2) + '\n')
if report['compile']['exit'] != 0:
    raise SystemExit('Compile failed; retained logs and execution record')
classpath = str(ROOT / 'classes') + ':' + str(JAR)
for case in frozen['cases']:
    original = args.sources_dir / case['source']
    source = ROOT / case['input']
    output = ROOT / 'outputs' / source.name
    assert not output.exists()
    before = source.read_bytes()
    expected = (ROOT / 'expected' / source.name).read_bytes()
    checks = {'original_sha256': digest(original.read_bytes()), 'input_sha256': digest(before),
              'expected_sha256': digest(expected), 'prediction_sha256': digest(frozen_bytes),
              'model_sha256': digest(MODEL_SOURCE.read_bytes()),
              'number_helper_sha256': digest(NUMBER_HELPER.read_bytes()),
              'text_helper_sha256': digest(TEXT_HELPER.read_bytes()),
              'observer_sha256': digest(OBSERVER.read_bytes())}
    assert checks['original_sha256'] == checks['input_sha256'] == case['source_sha256']
    assert checks['expected_sha256'] == case['expected_sha256']
    role = actual_database_role(before)
    assert role['property_row'] == case['prediction_facts']['database_row']
    entry = {**case, 'pre_mutation_checks': checks, 'database_before': role}
    report['cases'].append(entry)
    (ROOT / 'execution.json').write_text(json.dumps(report, indent=2) + '\n')
    if case['kind'] == 'LONG':
        command = [JAVA / 'java', '-Xmx256m', '-cp', classpath, 'CreatedNumberColumnOracle', source, output, 'Table1', case['column'], '--default-zero']
    else:
        command = [JAVA / 'java', '-Xmx256m', '-cp', classpath, 'CreatedShortTextColumnOracle', source, output, 'Table1', case['column'], '255', 'true']
    entry['execution'] = execute(command, case['case'])
    if output.exists():
        actual = output.read_bytes()
        entry.update({'after_sha256': digest(actual), 'after_bytes': len(actual),
                      'complete_image_match': actual == expected,
                      'original_unchanged': original.read_bytes() == before,
                      'input_unchanged': source.read_bytes() == before,
                      'facts': model.observations(before, actual, REPO),
                      'database_after': actual_database_role(actual),
                      'mismatch_offsets_first_64': [index for index, pair in enumerate(zip(actual, expected)) if pair[0] != pair[1]][:64]})
        if entry['execution']['exit'] == 0:
            entry['read_only_observer'] = execute([JAVA / 'java', '-Xmx256m', '-cp', classpath, 'AfterOffProperties', output, case['column'], case['kind']], case['case'] + '-properties')
            entry['observer_bytes_unchanged'] = output.read_bytes() == actual
    (ROOT / 'execution.json').write_text(json.dumps(report, indent=2) + '\n')
    print(case['case'], entry['execution']['exit'], entry.get('complete_image_match'), entry.get('after_sha256'), flush=True)
report['finished_unix'] = time.time()
(ROOT / 'execution.json').write_text(json.dumps(report, indent=2) + '\n')
