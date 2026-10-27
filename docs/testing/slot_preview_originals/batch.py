"""Separate original-value permutation recipe and final-image batch byte proof."""
import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from slot_preview_originals import common, model
from slot_preview_originals import proof as shared

ROOT = common.ROOT

SCENARIOS = [
    (
        'unique-text-swap',
        'UniqueLegacy',
        'Label',
        [1, 2],
        'text16:' + 'uniq-9999'.encode('utf-16-le').hex(),
    ),
    ('unique-binary-swap', 'UniqueModern', 'Bytes', [1, 2], 'binary:' + (b'\xff' * 8).hex()),
    ('indexed-text-pair', 'IndexedLegacy', 'Label', [4, 5], None),
    ('indexed-binary-cycle', 'IndexedModern', 'Bytes', [3, 9, 15], None),
]


def token(getter):
    if getter.startswith('String16:'):
        return 'text16:' + getter[len('String16:'):]
    if getter.startswith('Binary:'):
        return 'binary:' + getter[len('Binary:'):]
    raise AssertionError('Selected batch fixture value must be nonNULL Text/Binary')


def prepare_plan(facts_path, build_receipt):
    scenarios = []
    for original in json.loads(facts_path.read_text()):
        assert common.pin(ROOT / original['input']['path']) == original['input']
        for label, table_name, column, identities, temporary in SCENARIOS:
            table = original['tables'][table_name]
            field = table['columns'][column]
            assert field['flags'] == 2
            assert not any(entry[1] == 'ValidationRule' for entry in table['properties'])
            physical = sorted(table['rows'], key=lambda identity: (
                table['rows'][identity]['page'], table['rows'][identity]['slot']))
            values = [table['rows'][str(identity)]['values'][field['position']] for identity in identities]
            assert len(set(values)) == len(values)
            widths = [table['rows'][str(identity)]['slots'][column]['byte_width'] for identity in identities]
            assert len(set(widths)) == 1
            requests = []
            for position, identity in enumerate(identities):
                requests.append({'ID': identity, 'ordinal': physical.index(str(identity)),
                                 'expected': token(values[position]),
                                 'replacement': token(values[(position + 1) % len(values)]),
                                 'original_span': widths[position]})
            if temporary is not None:
                assert temporary not in [token(row['values'][field['position']]) for row in table['rows'].values()]
                reference_actions = [
                    {'ID': identities[0], 'value': temporary},
                    {'ID': identities[1], 'value': token(values[0])},
                    {'ID': identities[0], 'value': token(values[1])},
                ]
                direct_unique_refusal = {'ID': identities[0], 'value': token(values[1])}
            else:
                reference_actions = [{'ID': request['ID'], 'value': request['replacement']} for request in requests]
                direct_unique_refusal = None
            scenarios.append({'profile': original['profile'], 'label': label,
                              'table': table_name, 'column': column,
                              'requests': requests, 'expected_candidate': True,
                              'request_orders': ['physical', 'reversed'],
                              'expected_changed_rows': len(requests),
                              'reference_actions': reference_actions,
                              'public_direct_unique_refusal': direct_unique_refusal,
                              'oracle_scope': (
                                  'Final typed rows/key orders after explicit safe public staging; not atomic '
                                  'SQL or per-row unique-intermediate parity.'
                              )})
    record = {'scope': (
        'Separate supplement, not part of original162: two genuine unique swaps plus '
        'indexed two/three-row permutations per profile, each native '
        'physical/reversed request order.'
    ),
              'scenarios': scenarios, 'scenario_count': 8, 'native_candidates': 16,
              'public_expected_successful_staging_calls': 22,
              'public_expected_direct_unique_refusals': 4,
              'expected_disagreements': 0,
              'source_facts': common.pin(facts_path), 'preparation': common.pin(Path(__file__)),
              'frozen_consumer': None,
              'admission': (
                  'All assignments permute existing distinct valid values within equal original'
                  ' spans. Unique tables retain the complete original unique-value multiset; '
                  'indexed tuple uniqueness includes unchanged primary IDs. No changed '
                  'properties, NULLs, macro scope or wider carrier/profile.'
              ),
              'requirements': [(
                  'All native candidates originate from untouched originals with exact original'
                  ' expected values.'
              ),
                               (
                                   'Complete final typed-row model plus independent public final index order/key'
                                   ' oracle.'
                               ),
                               (
                                   'Original leaf populations/full ancestors/aliases/maps/counters/opaque bytes '
                                   'and exact allowed slot spans.'
                               ),
                               'Physical/reversed native requests must yield identical complete candidate bytes.',
                               (
                                   'Public staging is explicit, copy-only, with no claim native Access or atomic'
                                   ' statement timing; direct unique-first-call failures remain separate.'
                               )]}
    build = json.loads(build_receipt.read_text())
    record['frozen_consumer'] = {
        'path': build['frozen_example']['path'],
        'sha256': build['frozen_example']['sha256'],
        'revision': build['compiled_revision'],
    }
    return record


def expected_rows(before, actual, scenario):
    shared.unchanged_metadata(before, actual)
    name = scenario['table']
    column = scenario['column']
    position = before[name]['columns'][column]['position']
    replacements = {str(request['ID']): model.getter(model.value(request['replacement']))
                    for request in scenario['requests']}
    for table_name, table in before.items():
        assert list(table['rows']) == list(actual[table_name]['rows'])
        for identity, row in table['rows'].items():
            expected = list(row['values'])
            if table_name == name and identity in replacements:
                expected[position] = replacements[identity]
            assert actual[table_name]['rows'][identity]['values'] == expected


def verify(before_raw, after_raw, original, actual, scenario, reference, reference_raw):
    assert len(before_raw) == len(after_raw)
    tables = original['tables']
    expected_rows(tables, actual, scenario)
    expected_rows(tables, reference, scenario)
    table = tables[scenario['table']]
    column = scenario['column']
    field = table['columns'][column]
    selected = {request['ID']: request for request in scenario['requests']}
    allowed = []
    for identity, request in selected.items():
        row = table['rows'][str(identity)]
        slot = row['slots'][column]
        wire = after_raw[slot['absolute_start']:slot['absolute_end']].hex()
        assert wire in model.representations(model.value(request['replacement']), slot['byte_width'])
        # All supplement values are nonNULL before/after: no presence bit or
        # other record byte can change outside the selected payload span.
        assert slot['present']
        allowed.append((slot['absolute_start'], slot['absolute_end']))
    old_pages = shared.pages(before_raw, table)
    new_pages = shared.pages(after_raw, table)
    reference_table = reference[scenario['table']]
    reference_pages = shared.pages(reference_raw, reference_table)
    affected = {index['root'] for index in table['indexes'].values()
                if any(component[0] == column for component in index['columns'])}
    orders = {}
    for index in table['indexes'].values():
        root = index['root']
        if root in orders:
            continue
        old_entries, visited = shared.tree_entries(root, old_pages)
        new_entries, current = shared.tree_entries(root, new_pages, old_pages)
        assert current == visited
        old_keys, old_order = shared.by_identity(table, old_entries)
        new_keys, new_order = shared.by_identity(table, new_entries)
        expected_keys = dict(old_keys)
        expected_order = old_order
        if root in affected:
            reference_index = next(entry for entry in reference_table['indexes'].values()
                                   if entry['physical'] == index['physical'])
            reference_entries, _ = shared.tree_entries(reference_index['root'], reference_pages)
            reference_keys, expected_order = shared.by_identity(reference_table, reference_entries)
            for identity in selected:
                expected_keys[identity] = (
                    reference_keys[identity][:-shared.REFERENCE_BYTES]
                    + old_keys[identity][-shared.REFERENCE_BYTES:]
                )
            for page in visited:
                offset = page * shared.PAGE_BYTES
                kind = slice(offset, offset + shared.FREE_SPACE_OFFSET)
                assert after_raw[kind] == before_raw[kind]
                header = slice(offset + shared.OWNER_OFFSET, offset + shared.COMMON_PREFIX_OFFSET)
                assert after_raw[header] == before_raw[header]
                assert after_raw[offset + shared.HEIGHT_OFFSET] == before_raw[offset + shared.HEIGHT_OFFSET]
                allowed.extend([
                    (
                        offset + shared.FREE_SPACE_OFFSET,
                        offset + shared.FREE_SPACE_OFFSET + shared.FREE_SPACE_BYTES,
                    ),
                    (
                        offset + shared.COMMON_PREFIX_OFFSET,
                        offset + shared.COMMON_PREFIX_OFFSET + shared.COMMON_PREFIX_BYTES,
                    ),
                    (offset + shared.ENTRY_MAP_OFFSET, offset + shared.PAGE_BYTES),
                ])
        assert new_keys == expected_keys
        assert new_order == expected_order
        orders[root] = new_order
    for name, definition in tables.items():
        for label, index in definition['indexes'].items():
            expected = orders[index['root']] if name == scenario['table'] else index['order']
            assert actual[name]['indexes'][label]['order'] == expected
    allowed.sort()
    cursor = 0
    changed = 0
    for offset, (old, new) in enumerate(zip(before_raw, after_raw)):
        if old == new:
            continue
        changed += 1
        while cursor < len(allowed) and allowed[cursor][1] <= offset:
            cursor += 1
        assert cursor < len(allowed) and allowed[cursor][0] <= offset, offset
    assert changed > 0
    return {'changed_bytes': changed, 'all_typed_rows_and_logical_orders_match': True,
            'all_retained_full_key_row_entries_exact': True, 'full_ancestor_maxima_and_original_leaf_populations': True,
            'all_other_original_bytes_preserved': True, 'original_file_length_and_mask_bits_preserved': True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--facts', type=Path, required=True)
    parser.add_argument('--build-receipt', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = common.path(args.output)
    assert not output.exists(), 'Plan output must be fresh'
    record = prepare_plan(common.path(args.facts), common.path(args.build_receipt))
    output.write_text(json.dumps(record, indent=2) + '\n')
    print(common.pin(output))
if __name__ == '__main__':
    main()
