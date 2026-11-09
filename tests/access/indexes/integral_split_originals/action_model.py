"""Observe exact typed rows, keys, topology and counters; no process side effects."""
from decimal import Decimal
import hashlib
from pathlib import Path

from integral_split_originals import model as source
from delete_reference_maps import decode_map

PAGE = 4096
TABLE_HEADER = 63
COUNTER_BYTES = 12
COLUMN_BYTES = 25
PHYSICAL_BYTES = 52
COLUMN_NAME_BYTES = 2
PHYSICAL_COUNT_OFFSET = 51
USAGE_OFFSET = 34
ROOT_OFFSET = 38
FLAGS_OFFSET = 46
FIXED_DATA_OFFSET = 21
RECORD_HEADER_BYTES = 2
VALUE_INDEX = 1
MAP_INLINE = 0
MAP_INDIRECT = 1
MAP_BASE = 1
MAP_BITMAP = 5
BITMAP_DATA = 4
POINTER_BYTES = 4
BITS_PER_BYTE = 8


def digest(data):
    return hashlib.sha256(data).hexdigest()


def units(value, native_type):
    carrier, text = value.split(':', 1)
    expected = {2: 'Byte', 3: 'Short', 4: 'Integer', 19: 'Long', 5: 'BigDecimal'}
    assert carrier == expected[native_type]
    if native_type == 5:
        scaled = Decimal(text) * 10000
        assert scaled == scaled.to_integral_value()
        result = int(scaled)
        assert -(1 << 63) <= result < 1 << 63
        return result
    result = int(text)
    if native_type == 2:
        assert -128 <= result <= 127
        return result if result >= 0 else result + 256
    return result


def physical_start(raw, table):
    definition = raw[table['definition'] * PAGE:(table['definition'] + 1) * PAGE]
    count = source.number(definition, PHYSICAL_COUNT_OFFSET)
    position = TABLE_HEADER + count * COUNTER_BYTES + table['columns_count'] * COLUMN_BYTES
    for name in table['columns']:
        length = source.number(definition, position, COLUMN_NAME_BYTES)
        position += COLUMN_NAME_BYTES
        assert definition[position:position + length].decode('utf-16le') == name
        position += length
    return definition, position


def key_offset(raw, table):
    definition = raw[table['definition'] * PAGE:(table['definition'] + 1) * PAGE]
    count = source.number(definition, PHYSICAL_COUNT_OFFSET)
    field = TABLE_HEADER + count * COUNTER_BYTES + table['columns']['KeyValue']['position'] * COLUMN_BYTES
    return RECORD_HEADER_BYTES + source.number(definition, field + FIXED_DATA_OFFSET, 2)


def inspect_indexes(raw, tables):
    result = {}
    for name, table in tables.items():
        definition, start = physical_start(raw, table)
        references = {str(identity): row['page'] << 8 | row['slot']
                      for identity, row in table['rows'].items()}
        current = {}
        for label, index in table['indexes'].items():
            assert len(index['columns']) == 1
            column, ascending = index['columns'][0]
            metadata = table['columns'][column]
            width = metadata['declared_bytes']
            native_type = metadata['native_type']
            assert native_type in (2, 3, 4, 19, 5)
            position = metadata['position']
            expected = []
            for identity, row in table['rows'].items():
                key = units(row['values'][position], native_type)
                expected.append(source.wire(key, references[str(identity)], width,
                                            native_type == 2, ascending == 'false'))
            expected.sort()
            tree = source.tree(raw, index['root'], table['definition'], width)
            assert tree['keys_hex'] == [entry.hex() for entry in expected]
            by_reference = {reference: int(identity) for identity, reference in references.items()}
            assert index['order'] == [by_reference[int.from_bytes(entry[-4:], 'big')]
                                      for entry in expected]
            at = start + index['physical'] * PHYSICAL_BYTES
            assert source.number(definition, at + ROOT_OFFSET) == index['root']
            metadata = source.map_metadata(raw, source.number(definition, at + USAGE_OFFSET),
                                           tree['pages'], index['owned_pages'])
            maps = decode_map(raw, metadata, PAGE, len(raw) // PAGE)
            counter_start = TABLE_HEADER + index['physical'] * COUNTER_BYTES
            current[label] = {'physical': index['physical'], 'root': index['root'],
                              'flags': definition[at + FLAGS_OFFSET], 'tree': tree,
                              'owned_map': maps, 'public_entry_count': index['entries'],
                              'counter_hex': definition[counter_start:counter_start + COUNTER_BYTES].hex()}
        result[name] = current
    return result


def normalized_tables(tables):
    return {name: {**table, 'rows': {str(key): value for key, value in table['rows'].items()},
                   'raw_rows': {str(key): value for key, value in table['raw_rows'].items()}}
            for name, table in tables.items()}



def complete_members(raw, metadata):
    start = metadata['row_start']
    end = metadata['row_end']
    row = raw[start:end]
    ranges = []
    if row[0] == MAP_INLINE:
        ranges.append((source.number(row, MAP_BASE), row[MAP_BITMAP:]))
    else:
        assert row[0] == MAP_INDIRECT
        per_bitmap = (PAGE - BITMAP_DATA) * BITS_PER_BYTE
        for offset in range(MAP_BASE, len(row), POINTER_BYTES):
            pointer = source.number(row, offset)
            if pointer:
                body = raw[pointer * PAGE:(pointer + 1) * PAGE]
                assert body[:2] == bytes.fromhex('0501')
                slot = (offset - MAP_BASE) // POINTER_BYTES
                ranges.append((slot * per_bitmap, body[BITMAP_DATA:]))
    members = set()
    for base, bitmap in ranges:
        for position, value in enumerate(bitmap):
            if value:
                for bit in range(BITS_PER_BYTE):
                    if value & (1 << bit):
                        members.add(base + position * BITS_PER_BYTE + bit)
    assert len(members) == metadata['complete_set_bit_count']
    return members

def analyze(before_path, after_path, original, target, description):
    before = Path(before_path).read_bytes()
    after = Path(after_path).read_bytes()
    assert len(after) <= source.MAX_BYTES and len(after) % PAGE == 0
    source.qualify_header(after, original['profile'])
    before_tables = normalized_tables(original['facts']['all_tables'])
    tables, map_reports = source.split_description(description)
    tables = normalized_tables(tables)
    assert set(tables) == set(before_tables)
    native_type, width, count, kind, maximum = source.KINDS[target]
    reference_changes = []
    raw_body_changes = []
    field = key_offset(before, before_tables[target])
    for name, table in tables.items():
        prior = before_tables[name]
        assert table['columns'] == prior['columns']
        assert table['properties'] == prior['properties']
        assert table['count'] == prior['count']
        assert set(table['rows']) == set(prior['rows'])
        assert set(table['indexes']) == set(prior['indexes'])
        for identity, row in table['rows'].items():
            old = prior['rows'][identity]
            expected = list(old['values'])
            if name == target and identity == '1':
                assert units(old['values'][VALUE_INDEX], native_type) == 0
                assert units(row['values'][VALUE_INDEX], native_type) == maximum
                expected[VALUE_INDEX] = row['values'][VALUE_INDEX]
            assert row['values'] == expected
            if (row['page'], row['slot']) != (old['page'], old['slot']):
                reference_changes.append({'table': name, 'identity': identity,
                                          'before': [old['page'], old['slot']],
                                          'after': [row['page'], row['slot']]})
            old_body = bytes.fromhex(prior['raw_rows'][identity])
            new_body = bytes.fromhex(table['raw_rows'][identity])
            expected_body = bytearray(old_body)
            selected = name == target and identity == '1'
            if selected:
                expected_body[field:field + width] = maximum.to_bytes(width, 'little', signed=native_type != 2)
            if old_body != new_body:
                raw_body_changes.append({'table': name, 'identity': identity, 'selected': selected,
                                         'before_sha256': digest(old_body), 'after_sha256': digest(new_body),
                                         'exact_selected_field_only': selected and new_body == expected_body})
        for label, index in table['indexes'].items():
            old = prior['indexes'][label]
            for key in ['physical', 'columns', 'primary', 'unique', 'required', 'ignored_null']:
                assert index[key] == old[key]
    before_indexes = inspect_indexes(before, before_tables)
    after_indexes = inspect_indexes(after, tables)
    changes = []
    for page in range(min(len(before), len(after)) // PAGE):
        first = before[page * PAGE:(page + 1) * PAGE]
        second = after[page * PAGE:(page + 1) * PAGE]
        if first != second:
            changes.append({'page': page, 'bytes_changed': sum(a != b for a, b in zip(first, second)),
                            'before_sha256': digest(first), 'after_sha256': digest(second)})
    changed_pages = {entry['page'] for entry in changes}
    index_effects = {}
    counter_effects = []
    unaffected_rewrites = []
    for name, indexes in after_indexes.items():
        for label, index in indexes.items():
            old = before_indexes[name][label]
            after_tree = index['tree']
            old_tree = old['tree']
            roots = [old['root'], index['root']]
            affected = name == target and tables[name]['indexes'][label]['columns'][0][0] == 'KeyValue'
            if old['counter_hex'] != index['counter_hex'] or old['public_entry_count'] != index['public_entry_count']:
                counter_effects.append({'table': name, 'index': label, 'physical': index['physical'],
                                        'before_counter_hex': old['counter_hex'], 'after_counter_hex': index['counter_hex'],
                                        'before_getter_count': old['public_entry_count'],
                                        'after_getter_count': index['public_entry_count']})
            if not affected:
                rewritten = sorted(set(old_tree['pages']) & changed_pages)
                if rewritten:
                    unaffected_rewrites.append({'table': name, 'index': label, 'physical': index['physical'],
                                                'pages': rewritten,
                                                'full_stream_unchanged': old_tree['keys_hex'] == after_tree['keys_hex']})
                continue
            root = after_tree['pages'][index['root']]
            leaves = [{'page': page, 'count': len(body['entries_hex']), 'prefix': body['prefix'],
                       'free': body['free'], 'previous': body['previous'], 'next': body['next']}
                      for page, body in after_tree['pages'].items() if body['height'] == 0]
            index_effects[label] = {'physical': index['physical'], 'before_root': roots[0],
                                   'after_root': roots[1], 'root_height': root['height'],
                                   'root_tail': root['tail'], 'root_entries_hex': root['entries_hex'],
                                   'leaves': leaves, 'owned_added': sorted(set(after_tree['pages']) - set(old_tree['pages'])),
                                   'owned_removed': sorted(set(old_tree['pages']) - set(after_tree['pages'])),
                                   'counter_before': old['counter_hex'], 'counter_after': index['counter_hex'],
                                   'all_after_pages': after_tree['pages']}
    decoded_maps = {}
    for name, report in map_reports.items():
        decoded_maps[name] = {label: decode_map(after, metadata, PAGE, len(after) // PAGE)
                              for label, metadata in report['maps'].items()}

    map_changes = {}
    for name, maps in decoded_maps.items():
        old_maps = original['facts']['tables'][name]['maps']
        map_changes[name] = {}
        for label, metadata in maps.items():
            old = old_maps[label]
            first = complete_members(before, old)
            second = complete_members(after, metadata)
            map_changes[name][label] = {
                'added_complete_members': sorted(second - first),
                'removed_complete_members': sorted(first - second),
                'before_representation': old['payload_type'],
                'after_representation': metadata['payload_type'],
                'before_extent': [old['map_page'], old['row_start'], old['row_end']],
                'after_extent': [metadata['map_page'], metadata['row_start'], metadata['row_end']],
            }
    for label, effect in index_effects.items():
        old = before_indexes[target][label]['owned_map']
        current = after_indexes[target][label]['owned_map']
        first = complete_members(before, old)
        second = complete_members(after, current)
        effect['complete_owned_added'] = sorted(second - first)
        effect['complete_owned_removed'] = sorted(first - second)
    return {'profile': original['profile'], 'target': target, 'kind': kind, 'typed_max_units': maximum,
            'all_typed_rows_markers_opaque_values_equal_except_selected_key': True,
            'all_key_row_streams_and_public_orders_verified': True,
            'before_bytes': len(before), 'after_bytes': len(after),
            'eof_page_delta': (len(after) - len(before)) // PAGE,
            'original_page_changes': changes, 'row_reference_changes': reference_changes,
            'raw_row_body_changes': raw_body_changes, 'counter_changes': counter_effects,
            'unaffected_index_rewrites': unaffected_rewrites,
            'selected_index_effects': index_effects, 'after_maps': decoded_maps,
            'complete_map_membership_changes': map_changes,
            'all_after_index_facts': after_indexes}
