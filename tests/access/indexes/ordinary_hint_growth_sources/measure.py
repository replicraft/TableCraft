"""Saved-byte prerequisites and role-specific hypothetical transition inventory."""
import hashlib
import json
from pathlib import Path

PAGE = 4096
MAP_BYTES = 69
INLINE_BITS = 512
BITMAP_BITS = 32736
TARGETS = ('TextLegacy', 'TextModern', 'BinaryOne')
MAX_FILE_BYTES = 8 * 1024 * 1024


def original_page_inventory(original, original_raw):
    """Protect user payload pages and non-global bitmap storage, not containers."""
    assert len(original_raw) <= MAX_FILE_BYTES and len(original_raw) % PAGE == 0
    page_count = len(original_raw) // PAGE
    pages = {}
    references = 0
    reference_limit = 2 * len(original['maps']) * page_count

    def add(page, role, purpose):
        nonlocal references
        references += 1
        assert references <= reference_limit
        assert isinstance(page, int) and 0 <= page < page_count
        entry = pages.setdefault(page, {'page': page, 'roles': [], 'purposes': []})
        if role not in entry['roles']:
            entry['roles'].append(role)
        if purpose not in entry['purposes']:
            entry['purposes'].append(purpose)

    for name, table in original['tables'].items():
        ordinary = name + ':Owned'
        for page in original['maps'][ordinary]['file_members']:
            add(page, ordinary, 'ordinary-data')
        for field, column in table['columns'].items():
            if column['native'] not in (11, 12):
                continue
            role = name + ':' + field + ':Owned'
            for page in original['maps'][role]['file_members']:
                add(page, role, 'long-value-data')

    for role, metadata in original['maps'].items():
        if role == 'GlobalFree':
            continue
        for page in metadata['decoded']['bitmap_pages']:
            add(page, role, 'non-global-bitmap')

    assert len(pages) <= page_count
    return {
        'pages': [pages[page] for page in sorted(pages)],
        'inventory_references_checked': references,
        'inventory_reference_bound': reference_limit,
        'unique_pages': len(pages),
        'comparison_bytes': len(pages) * PAGE,
    }


def preserve_original_pages(original, original_raw, raw):
    assert len(raw) <= MAX_FILE_BYTES and len(raw) % PAGE == 0
    assert len(raw) >= len(original_raw)
    inventory = original_page_inventory(original, original_raw)
    compared = 0
    for entry in inventory['pages']:
        start = entry['page'] * PAGE
        end = start + PAGE
        compared += PAGE
        assert compared <= MAX_FILE_BYTES
        assert raw[start:end] == original_raw[start:end], entry
    assert compared == inventory['comparison_bytes']
    inventory['comparison_bytes_checked'] = compared
    return inventory


def transition(kind, base, members, target):
    if kind == 1:
        assert target < BITMAP_BITS
        return {'kind': 'represented-indirect'}
    assert kind == 0
    if base <= target < base + INLINE_BITS:
        return {'kind': 'represented-inline', 'base': base}
    combined = [*members, target]
    aligned = min(combined) // 8 * 8
    span = max(combined) - aligned + 1
    return {
        'kind': 'inline-rebase' if span < INLINE_BITS else 'inline-to-indirect',
        'base': aligned,
        'span': span,
        'rule': 'outside-current-window then aligned strict-span<512',
    }


def payload(raw, metadata):
    decoded = metadata['decoded']
    row = raw[decoded['row_start']:decoded['row_end']]
    assert len(row) == MAP_BYTES
    return row


def qualify(actual, raw, original, original_raw, padding_rows):
    assert len(raw) <= MAX_FILE_BYTES
    assert sum(table['count'] for table in actual['tables'].values()) == 2701 + padding_rows
    assert sum(table['count'] for table in actual['tables'].values()) <= 4096
    old_property_owners = set(original['tables'])
    for name, table in original['tables'].items():
        old_property_owners.update(name + ':' + column for column in table['columns'])
    assert [item for item in actual['properties'] if item[0] in old_property_owners] == original['properties']
    for name, table in original['tables'].items():
        assert json.loads(json.dumps(actual['tables'][name])) == table, name
        definition = table['definition'] * PAGE
        assert raw[definition:definition + PAGE] == original_raw[definition:definition + PAGE]
        for index in table['indexes'].values():
            for page in index['pages']:
                assert raw[page * PAGE:(page + 1) * PAGE] == original_raw[page * PAGE:(page + 1) * PAGE]
    for name, before in original['maps'].items():
        if name == 'GlobalFree':
            continue
        after = actual['maps'][name]
        before_payload = payload(original_raw, before)
        after_payload = payload(raw, after)
        expected = bytearray(before_payload)
        if name in ('TextModern:Hint', 'BinaryOne:Hint'):
            table = name.split(':')[0]
            low = min(original['maps'][table + ':Owned']['file_members'])
            assert before_payload[0] == 0
            base = int.from_bytes(before_payload[1:5], 'little')
            relative = low - base
            assert 0 <= relative < INLINE_BITS
            assert not expected[5 + relative // 8] & (1 << (relative % 8))
            expected[5 + relative // 8] |= 1 << (relative % 8)
        assert after_payload == expected, name
        assert after['decoded']['row_start'] == before['decoded']['row_start']
        assert after['decoded']['row_end'] == before['decoded']['row_end']

    original_pages = preserve_original_pages(original, original_raw, raw)

    eof = len(raw) // PAGE
    observations = {}
    for table in TARGETS:
        owned = actual['maps'][table + ':Owned']
        hint = actual['maps'][table + ':Hint']
        owned_row = payload(raw, owned)
        hint_row = payload(raw, hint)
        assert owned_row[0] == 1 and owned['decoded']['bitmap_pages']
        assert hint_row[0] == 0
        base = int.from_bytes(hint_row[1:5], 'little')
        assert not base <= eof < base + INLINE_BITS
        high = max(owned['file_members'])
        assert high == max(hint['file_members'])
        remaining = [member for member in hint['file_members'] if member != high]
        assert remaining == ([] if table == 'TextLegacy' else [min(owned['file_members'])])
        required_record_bytes = 297 if table == 'BinaryOne' else 547
        assert required_record_bytes > 32 + 39
        hint_effect = transition(0, base, remaining, eof)
        cursor = eof + 1 + int(hint_effect['kind'] == 'inline-to-indirect')
        groups = []
        for label in ('Ascending', 'Descending'):
            metadata = actual['maps'][table + ':Index:' + label]
            row = payload(raw, metadata)
            kind = row[0]
            current_base = int.from_bytes(row[1:5], 'little') if kind == 0 else 0
            members = list(metadata['file_members'])
            steps = []
            for _ in range(3):
                number = cursor
                effect = transition(kind, current_base, members, number)
                cursor += 1
                if effect['kind'] == 'inline-to-indirect':
                    steps.append({'index_page': number, 'effect': effect, 'bitmap_page': cursor})
                    cursor += 1
                    kind = 1
                else:
                    steps.append({'index_page': number, 'effect': effect})
                    current_base = effect.get('base', current_base)
                members.append(number)
            groups.append({'index': label, 'physical': actual['tables'][table]['indexes'][label]['physical'], 'steps': steps})
        observations[table] = {
            'mode': 'natural-highest-only' if table == 'TextLegacy' else 'controlled-public-usage-map-lower',
            'source_hint_base': base,
            'source_hint_members': hint['file_members'],
            'failed_highest': high,
            'remaining_after_actual_required_miss': remaining,
            'required_record_bytes': required_record_bytes,
            'hypothetical_data_page': eof,
            'hypothetical_hint_transition': hint_effect,
            'hypothetical_index_groups': groups,
            'hypothetical_final_eof': cursor,
            'scope': 'source-only primary-policy classification, not observed UPDATE geometry or candidate golden',
        }
    return {
        'eof': eof,
        'bytes': len(raw),
        'sha256': hashlib.sha256(raw).hexdigest(),
        'padding_rows': padding_rows,
        'rows_verified': 2701 + padding_rows,
        'original_page_preservation': original_pages,
        'target_observations': observations,
        'future_actions_executed': 0,
    }


def selfcheck():
    assert transition(0, 0, [95], 598)['kind'] == 'inline-rebase'
    assert transition(0, 0, [95], 599)['kind'] == 'inline-to-indirect'
    assert transition(0, 88, [95], 599)['kind'] == 'represented-inline'
    assert transition(0, 0, [], 600) == {
        'kind': 'inline-rebase', 'base': 600, 'span': 1,
        'rule': 'outside-current-window then aligned strict-span<512',
    }
    return {'checks': 4, 'saved_original_page_checks': saved_original_selfcheck(),
            'fixture_processes': 0}


def saved_original_selfcheck():
    """Use pinned before bytes only; no database writer or fixture child runs."""
    root = Path(__file__).resolve().parents[3]
    directory = root / '.cache/ace16-variable-encoded-source-first-20261027'
    expected_pages = {
        'V2016': {
            94, 95, 96, 97, 98, 99, 100, 101, 104, 105, 106,
            112, 113, 114, 115, 116, 117, 118, 119, 122, 123, 124,
            130, 131, 132, 133, 134, 135, 136, 137, 140, 141, 142,
            146, 147, 148,
        },
        'V2019': {
            111, 112, 113, 114, 115, 116, 117, 118, 121, 122, 123,
            129, 130, 131, 132, 133, 134, 135, 136, 139, 140, 141,
            148, 149, 150, 151, 152, 153, 154, 155, 158, 159, 160,
            164, 165, 166,
        },
    }
    result = []
    for profile in ('V2016', 'V2019'):
        facts = json.loads((directory / (profile + '-source-gates.json')).read_text())
        before = (directory / (profile + '-original.accdb')).read_bytes()
        healthy = preserve_original_pages(facts, before, before)
        assert {entry['page'] for entry in healthy['pages']} == expected_pages[profile]
        assert healthy['unique_pages'] == 36 and healthy['comparison_bytes_checked'] == 147456

        target = facts['maps']['TextLegacy:Owned']['file_members'][0]
        rows = int.from_bytes(before[target * PAGE + 12:target * PAGE + 14], 'little')
        gap = target * PAGE + 14 + rows * 2
        assert int.from_bytes(before[target * PAGE + 2:target * PAGE + 4], 'little') == 32
        last_slot = target * PAGE + 14 + (rows - 1) * 2
        assert gap + 32 == target * PAGE + int.from_bytes(before[last_slot:last_slot + 2], 'little')
        long_value = facts['maps']['LongSentinel:Memo:Owned']['file_members'][0]
        long_rows = int.from_bytes(before[long_value * PAGE + 12:long_value * PAGE + 14], 'little')
        assert 1 <= long_rows <= 255 and 14 + 2 * long_rows <= 20
        long_first_payload = min(
            int.from_bytes(before[long_value * PAGE + 14 + slot * 2:
                                  long_value * PAGE + 16 + slot * 2], 'little') & 0x1fff
            for slot in range(long_rows)
        )
        assert 20 < long_first_payload
        bitmap = facts['maps']['TextLegacy:Owned']['decoded']['bitmap_pages'][0]
        future = len(before) // PAGE + 100
        offsets = {
            'ordinary-free-gap': gap,
            'long-value-free-gap': long_value * PAGE + 20,
            'owned-bitmap-future-bit': bitmap * PAGE + 4 + future // 8,
        }
        refusals = []
        for label, offset in offsets.items():
            forged = bytearray(before)
            mask = 1 << (future % 8) if label == 'owned-bitmap-future-bit' else 1
            if label == 'owned-bitmap-future-bit':
                assert not before[offset] & mask
            forged[offset] ^= mask
            try:
                preserve_original_pages(facts, before, forged)
            except AssertionError:
                refusals.append(label)
            else:
                raise AssertionError('Opaque source-byte mutation was not refused: ' + label)
        assert refusals == list(offsets)
        result.append({'profile': profile, 'protected_pages': healthy['unique_pages'],
                       'healthy_bytes_compared': healthy['comparison_bytes_checked'],
                       'in_memory_tamper_refusals': refusals})
    return result
