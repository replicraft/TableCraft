"""SOURCE/request-only fitted records; no Rust, Java NEW or AFTER authority."""
import hashlib

from ace16_insert_extended_datetime import source
from ace16_insert_external_long_values import model as ordinary

PAGE = 4096


def encoded_timestamp(text):
    assert len(text) == 27
    year, month, day = int(text[:4]), int(text[5:7]), int(text[8:10])
    hour, minute, second = int(text[11:13]), int(text[14:16]), int(text[17:19])
    fraction = int(text[20:])
    completed = year - 1
    days = completed * 365 + completed // 4 - completed // 100 + completed // 400
    leap = year % 4 == 0 and (year % 100 != 0 or year % 400 == 0)
    lengths = [31, 29 if leap else 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    days += sum(lengths[:month - 1]) + day - 1
    seconds = hour * 3600 + minute * 60 + second
    payload = f'{days:019}:{seconds:012}{fraction:07}:7\0'.encode('ascii')
    assert source.timestamp(payload) == text
    return payload


def record(row):
    identity, group, stamp = row
    body = b'\x03\0' + identity.to_bytes(4, 'little', signed=True)
    body += (0 if group is None else group).to_bytes(4, 'little', signed=True)
    body += bytes(42) if stamp is None else encoded_timestamp(stamp)
    mask = 1 | (group is not None) * 2 | (stamp is not None) * 4
    result = body + bytes([mask])
    assert len(result) == 53
    return result


def derive(state, requests):
    original = state['raw']
    table = state['table']
    owner = 'Table:' + str(table['definition'])
    assert len(requests) == 6 and len({row[0] for row in requests}) == 6
    assert not {row[0] for row in requests}.intersection(table['rows'])
    hints = state['maps'][owner + ':Hint']['file_members']
    assert len(hints) == 1
    data = hints[-1]
    count, end = ordinary.geometry(original, data)
    records = [record(row) for row in requests]
    assert count + len(records) <= 255
    assert sum(len(row) + 2 for row in records) <= end - 14 - count * 2

    image = ordinary.Image(state)
    for payload in records:
        image.new_rows.append(image.append(owner, payload, False))
    assert image.allocations == [] and not image.map_offsets
    assert all(event['phase'] == 'DATA' and event['fitted'] and event['page'] == data
               for event in image.events)
    for physical, column in [(0, 'ID'), (1, 'Group')]:
        image.index(physical, column, requests)
    ordinary.put(image.bytes, table['definition'] * PAGE + 16, table['count'] + len(requests), 4)
    result = bytes(image.bytes)
    assert len(result) == len(original)

    allowed = bytearray(len(original))

    def allow(start, finish):
        assert 0 <= start <= finish <= len(original)
        allowed[start:finish] = bytes([255]) * (finish - start)

    allow(table['definition'] * PAGE + 16, table['definition'] * PAGE + 20)
    allow(data * PAGE + 2, data * PAGE + 4)
    allow(data * PAGE + 12, data * PAGE + 14)
    for position, (row, placed) in enumerate(zip(requests, image.new_rows, strict=True)):
        slot = count + position
        finish = end - position * 53
        start = finish - 53
        assert placed == dict(reference=data << 8 | slot, start=data * PAGE + start,
                              end=data * PAGE + finish, role=owner)
        frame = source.native.long_wire.frame(result, data, slot)
        assert frame['word'] & 0xe000 == 0
        body = result[frame['start']:frame['end']]
        assert len(body) == 53 and source.native.number(body, 0) == 3
        assert body[-1] == 1 | (row[1] is not None) * 2 | (row[2] is not None) * 4
        assert int.from_bytes(body[2:6], 'little', signed=True) == row[0]
        assert int.from_bytes(body[6:10], 'little', signed=True) == (0 if row[1] is None else row[1])
        if row[2] is None:
            assert body[10:52] == bytes(42)
        else:
            assert source.timestamp(body[10:52]) == row[2]
        allow(data * PAGE + 14 + slot * 2, data * PAGE + 16 + slot * 2)
        allow(placed['start'], placed['end'])
    for row in table['rows'].values():
        frame, chain = source.native.long_wire.final_frame(result, row)
        assert result[frame['start']:frame['end']] == row['raw']
    after_free = source.native.number(result, data * PAGE + 2)
    assert after_free == source.native.number(original, data * PAGE + 2) - 6 * 55
    for physical, column, name in [(0, 'ID', 'PrimaryID'), (1, 'Group', 'GroupOrder')]:
        index = table['indexes'][name]
        nodes, leaves, entries = source.native.index_wire.tree(result, index['root'], table['definition'])
        expected = list(state['trees'][physical]['entries'])
        expected.extend(source.native.index_wire.key(row[0 if physical == 0 else 1], placed['reference'])
                        for row, placed in zip(requests, image.new_rows, strict=True))
        assert entries == sorted(expected)
        assert set(nodes) == set(state['trees'][physical]['nodes'])
        before_values = {values[column] for values in table['values'].values()}
        added = {row[0 if physical == 0 else 1] for row in requests} - before_values
        history = index['history'] + len(added)
        at = state['trees'][physical]['history_at']
        assert source.native.number(result, at, 4) == history
        allow(at, at + 4)
        allow(index['root'] * PAGE, (index['root'] + 1) * PAGE)
    for claim in state['map_claims'].values():
        assert result[claim['row_start']:claim['row_end']] == original[claim['row_start']:claim['row_end']]
        for page in claim['bitmaps']:
            assert result[page * PAGE:(page + 1) * PAGE] == original[page * PAGE:(page + 1) * PAGE]
    assert all(old == new or allowed[position] for position, (old, new) in enumerate(zip(original, result)))
    return result, dict(sha256=hashlib.sha256(result).hexdigest(), bytes=len(result),
                        definition=table['definition'], data_page=data, source_count=count,
                        final_count=count + 6, free_before=source.native.number(original, data * PAGE + 2),
                        free_after=after_free, new_rows=image.new_rows, events=image.events,
                        allocations=[], map_changes=0, table_rows_before=table['count'],
                        table_rows_after=table['count'] + 6,
                        histories_before=[table['indexes'][name]['history'] for name in ('PrimaryID', 'GroupOrder')],
                        histories_after=[source.native.number(result, state['trees'][physical]['history_at'], 4)
                                         for physical in (0, 1)])
