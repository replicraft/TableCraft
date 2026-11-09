"""Original/request-only public row order, histories and exact warning records."""
import copy
from collections import Counter
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / '.cache/ace16-nullable-split-batch-source-next-20261029'
sys.path.insert(0, str(SOURCE))
import source

LOGGER = 'com.healthmarketscience.jackcess.Index'
MAX_WARNING_LINE_BYTES = 2048
MAX_WARNING_RECORDS = 200


def key(row, columns, ignored):
    values = tuple(row[name] for name, _ in columns)
    if ignored and all(value is None for value in values):
        return None
    return b''.join(source.previous.component(value, ascending)
                    for value, (_, ascending) in zip(values, columns, strict=True))


def updates(case):
    assert case['table'] in ('Items', 'UniqueItems')
    assert case['recipe'] in ('Full', 'Clear', 'DuplicateFamily')
    if case['recipe'] == 'DuplicateFamily':
        assert case['table'] == 'Items'
        return [(1, {'Group': 42, 'Score': 777}), (2, {'Group': 42, 'Score': 777})]
    result = []
    for ordinal in range(500):
        if case['recipe'] == 'Clear' and ordinal >= 300:
            values = {'Group': None, 'Score': None}
        else:
            values = {'Group': source.signed(ordinal), 'Score': source.signed(ordinal + 500)}
        result.append((ordinal + 1, values))
    return result


def derive(original, case):
    rows = copy.deepcopy(original['values'])
    assert sorted(rows) == list(range(1, 501))
    histories = {name: original['indexes'][name]['history'] for name in ('LeftOrder', 'PairOrder')}
    families = {}
    for name in histories:
        index = original['indexes'][name]
        ignored = bool(index['flags'] & 2)
        keys = (key(row, index['columns'], ignored) for row in rows.values())
        families[name] = Counter(value for value in keys if value is not None)
    warnings = []
    steps = []
    assignments = updates(case)
    assert len(assignments) == case['updates']
    for identity, changes in assignments:
        old = rows[identity]
        updated = old | changes
        for name in ('LeftOrder', 'PairOrder'):
            index = original['indexes'][name]
            columns = index['columns']
            ignored = bool(index['flags'] & 2)
            old_key = key(old, columns, ignored)
            new_key = key(updated, columns, ignored)
            counts = families[name]
            if old_key is not None:
                assert counts[old_key] > 0
                counts[old_key] -= 1
                if counts[old_key] == 0:
                    del counts[old_key]
            all_null = all(updated[column] is None for column, _ in columns)
            if index['flags'] & 1 and not all_null:
                assert new_key not in counts, 'Unexpected public UNIQUE refusal in positive recipe'
            if new_key is None:
                assert name == 'PairOrder' and old_key is not None
                logical = original['rows'][identity]
                warnings.append(dict(identity=identity, page=logical['page'],
                                     slot=logical['slot'], old_key=old_key.hex()))
            elif old_key is None and new_key not in counts:
                histories[name] += 1
            if new_key is not None:
                counts[new_key] += 1
        rows[identity] = updated
        steps.append(dict(identity=identity, histories=dict(histories)))
    assert histories['LeftOrder'] == 201
    assert histories['PairOrder'] == (201 if case['recipe'] == 'DuplicateFamily' else 500)
    assert len(warnings) == case['warning_records'] <= MAX_WARNING_RECORDS
    return dict(rows=rows, histories=histories, warnings=warnings, steps=steps)


def entry_text(warning):
    payload = bytes.fromhex(warning['old_key'])
    assert len(payload) == 6, 'Original partial-NULL two-I32 key, without ROW bytes'
    text = ''
    for position, byte in enumerate(payload):
        if position:
            text += '  ' if position % 4 == 0 else ' '
        text += f'{byte:02X}'
    return ('Entry[rowId=' + str(warning['page']) + ':' + str(warning['slot'])
            + ',bytes=(' + str(len(payload)) + ') ' + text + ']')


def check_streams(stdout, stderr, case, intent):
    expected = ('ACTION\t' + case['id'] + '\tsuccess\t' + str(case['updates']) + '\n').encode()
    assert stdout == expected
    assert len(stderr) <= 16 * 1024 * 1024
    lines = stderr.splitlines(keepends=True)
    assert len(lines) == len(intent['warnings'])
    for line, warning in zip(lines, intent['warnings'], strict=True):
        assert len(line) <= MAX_WARNING_LINE_BYTES
        expected = ('WARNING\t' + LOGGER + '\tAdded duplicate index entry ' + entry_text(warning)
                    + ' (Db=' + Path(case['output']).name + ';Table=' + case['table']
                    + ';Index=[PairOrder, PairAlias])\n').encode()
        assert line == expected, 'Unexpected preserved public warning'
