"""Repeated after-OFF predictions reuse the maintained descriptor-bound owner."""
import importlib.util

PAGE = 4096
CATALOG_PAGE = 4
PROPERTY_PAGE = 10
ROW_COUNT_AT = 12
DIRECTORY_AT = 14
ROW_WORD_BYTES = 2
ROW_MASK = 0x1FFF
OBJECT_ID_AT = 2
PARENT_ID_AT = 6
OBJECT_TYPE_AT = 10
OBJECT_FLAGS_AT = 28
PROPERTY_AT = 44
REFERENCE_AT = PROPERTY_AT + 4
RESERVED_AT = REFERENCE_AT + 4
DESCRIPTOR_BYTES = 12
DATABASE_ID = 0x0F000004
DATABASE_PARENT = 0x0F000002
DATABASE_TYPE = 2
DATABASE_FLAGS = 0x80000000
SINGLE = 0x40000000
DATABASE_BODY_BYTES = 152
REFERENCE_PAGE_SHIFT = 8
REFERENCE_SLOT_MASK = 255


def owner(repository):
    path = repository / 'docs/testing/field_add_after_off/model.py'
    specification = importlib.util.spec_from_file_location('repeat_off_owned_model', path)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def prediction(before, kind, column, repository):
    return owner(repository).prediction(before, kind, column, repository)


def observations(before, after, repository):
    facts = owner(repository).observations(before, after, repository)
    old = database_role(before)
    new = database_role(after)
    facts.update({
        'database_row': old['property_row'],
        'database_body_bytes': old['body_end'] - old['body_start'],
        'database_body_unchanged': before[old['body_start']:old['body_end']] == after[new['body_start']:new['body_end']],
        'database_catalog_record_unchanged': before[old['record_start']:old['record_end']] == after[new['record_start']:new['record_end']],
        'database_reference_unchanged': old['descriptor_hex'] == new['descriptor_hex'],
        'database_before': old,
        'database_after': new,
        'chronology': 'actual DB ROW remains earlier than current user ROW',
    })
    assert facts['database_row'] < facts['before_user_row'] < facts['after_user_row']
    assert facts['database_body_bytes'] == DATABASE_BODY_BYTES
    return facts


def u16(data, at):
    return int.from_bytes(data[at:at + 2], 'little')


def u32(data, at):
    return int.from_bytes(data[at:at + 4], 'little')


def database_role(data):
    catalog_start = CATALOG_PAGE * PAGE
    catalog = data[catalog_start:catalog_start + PAGE]
    slot = u16(catalog, ROW_COUNT_AT) - 1
    start = u16(catalog, DIRECTORY_AT + ROW_WORD_BYTES * slot) & ROW_MASK
    end = u16(catalog, DIRECTORY_AT + ROW_WORD_BYTES * (slot - 1)) & ROW_MASK
    record = catalog[start:end]
    assert u32(record, OBJECT_ID_AT) == DATABASE_ID
    assert u32(record, PARENT_ID_AT) == DATABASE_PARENT
    assert u16(record, OBJECT_TYPE_AT) == DATABASE_TYPE and u32(record, OBJECT_FLAGS_AT) == DATABASE_FLAGS
    assert u32(record, PROPERTY_AT) == SINGLE | DATABASE_BODY_BYTES and u32(record, RESERVED_AT) == 0
    reference = u32(record, REFERENCE_AT)
    assert reference >> REFERENCE_PAGE_SHIFT == PROPERTY_PAGE
    property_row = reference & REFERENCE_SLOT_MASK
    page = data[PROPERTY_PAGE * PAGE:(PROPERTY_PAGE + 1) * PAGE]
    assert property_row < u16(page, ROW_COUNT_AT)
    word = u16(page, DIRECTORY_AT + ROW_WORD_BYTES * property_row)
    assert word & ~ROW_MASK == 0
    body_start = word & ROW_MASK
    body_end = PAGE if property_row == 0 else u16(page, DIRECTORY_AT + ROW_WORD_BYTES * (property_row - 1)) & ROW_MASK
    assert body_end - body_start == DATABASE_BODY_BYTES
    return {
        'catalog_slot': slot,
        'property_row': property_row,
        'descriptor_hex': record[PROPERTY_AT:PROPERTY_AT + DESCRIPTOR_BYTES].hex(),
        'record_start': catalog_start + start,
        'record_end': catalog_start + end,
        'body_start': PROPERTY_PAGE * PAGE + body_start,
        'body_end': PROPERTY_PAGE * PAGE + body_end,
    }
