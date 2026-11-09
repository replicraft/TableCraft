"""Independent role-bound adaptation of the maintained first-party ADD oracle.

Only body selection changes: the user's actual descriptor selects its MR2 row,
which need not be the final column-owned row after a database-option insertion.
All AFTER encoding remains in the unchanged maintained model/raw owners.
"""
import importlib.util
from pathlib import Path
import struct

PAGE = 4096
PROPERTY_PAGE = 10
DIRECTORY = 14
OFFSET_MASK = 0x1FFF
SINGLE = 0x40000000
LENGTH_MASK = 0x3FFFFFFF


def u16(data, at):
    return struct.unpack_from('<H', data, at)[0]


def u32(data, at):
    return struct.unpack_from('<I', data, at)[0]


def selected_body(before, row, begin, finish):
    descriptor = row[begin:finish]
    assert len(descriptor) == 12
    tagged, reference, reserved = struct.unpack('<III', descriptor)
    assert tagged & ~LENGTH_MASK == SINGLE and reserved == 0
    assert reference >> 8 == PROPERTY_PAGE
    page = before[PROPERTY_PAGE * PAGE:(PROPERTY_PAGE + 1) * PAGE]
    selected = reference & 255
    assert selected < u16(page, 12)
    word = u16(page, DIRECTORY + selected * 2)
    assert word & ~OFFSET_MASK == 0
    start = word & OFFSET_MASK
    end = PAGE if selected == 0 else u16(page, DIRECTORY + (selected - 1) * 2) & OFFSET_MASK
    assert end - start == tagged & LENGTH_MASK
    return page[start:end]


def maintained(repo):
    source = repo / 'docs/testing/short_text_add/model.py'
    spec = importlib.util.spec_from_file_location('owned_after_off_add_reference', source)
    owner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owner)
    owner.RAW_SOURCE = repo / 'docs/testing/number_column_repeat/raw.py'
    # Specialize this isolated module instance's read-only selector. No maintained
    # file, encoding function, original fixture or SDK output is rewritten.
    owner.body_before = selected_body
    return owner


def prediction(before, kind, column, repo):
    assert len(before) == 11 * PAGE
    assert u32(before, 2 * PAGE + 16) == 9
    owner = maintained(repo)
    raw = owner.prior_raw()
    if kind == 'LONG':
        return bytes(owner.number_again(before, column, raw))
    assert kind == 'TEXT'
    return bytes(owner.short_text(before, column, 255, True, raw))


def observations(before, after, repo):
    owner = maintained(repo)
    old_parent = owner.parent(before)
    new_parent = owner.parent(after)
    old_descriptor = bytes.fromhex(old_parent['property_descriptor_hex'])
    new_descriptor = bytes.fromhex(new_parent['property_descriptor_hex'])
    old_reference = u32(old_descriptor, 4)
    new_reference = u32(new_descriptor, 4)
    page_before = before[10 * PAGE:11 * PAGE]
    page_after = after[10 * PAGE:11 * PAGE]
    old_count = u16(page_before, 12)
    new_count = u16(page_after, 12)
    db_slot = old_count - 1
    db_start = u16(page_before, DIRECTORY + db_slot * 2) & OFFSET_MASK
    db_end = PAGE if db_slot == 0 else u16(page_before, DIRECTORY + (db_slot - 1) * 2) & OFFSET_MASK
    old_low = u16(page_before, DIRECTORY + (old_count - 1) * 2) & OFFSET_MASK
    catalog = before[4 * PAGE:5 * PAGE]
    db_catalog_slot = u16(catalog, 12) - 1
    db_catalog_start = u16(catalog, DIRECTORY + db_catalog_slot * 2) & OFFSET_MASK
    db_catalog_end = u16(catalog, DIRECTORY + (db_catalog_slot - 1) * 2) & OFFSET_MASK
    actual_db_catalog = after[4 * PAGE + db_catalog_start:4 * PAGE + db_catalog_end]
    new_field_count = u16(after, 7 * PAGE + 45)
    field = 7 * PAGE + 75 + (new_field_count - 1) * 25
    return {
        'before_user_row': old_reference & 255,
        'after_user_row': new_reference & 255,
        'database_row': db_slot,
        'database_catalog_slot': db_catalog_slot,
        'before_column_rows': old_count,
        'after_column_rows': new_count,
        'before_user_descriptor': old_descriptor.hex(),
        'after_user_descriptor': new_descriptor.hex(),
        'database_body_bytes': db_end - db_start,
        'database_body_unchanged': page_after[db_start:db_end] == page_before[db_start:db_end],
        'database_catalog_record_unchanged': actual_db_catalog == catalog[db_catalog_start:db_catalog_end],
        'all_old_property_rows_unchanged': page_after[old_low:] == page_before[old_low:],
        'all_old_property_directory_unchanged': page_after[DIRECTORY:DIRECTORY + old_count * 2] == page_before[DIRECTORY:DIRECTORY + old_count * 2],
        'maps_unchanged': after[3 * PAGE:4 * PAGE] == before[3 * PAGE:4 * PAGE],
        'index_page_unchanged': after[9 * PAGE:10 * PAGE] == before[9 * PAGE:10 * PAGE],
        'catalog_logical_count': u32(after, 2 * PAGE + 16),
        'catalog_physical_count': u16(after, 4 * PAGE + 12),
        'new_field_hex': after[field:field + 25].hex(),
        'before_definition': owner.definition_observation(before),
        'after_definition': owner.definition_observation(after),
        'changed_pages': [number for number in range(len(before) // PAGE)
                          if before[number * PAGE:(number + 1) * PAGE] != after[number * PAGE:(number + 1) * PAGE]],
    }
