"""Independent framing checks for this original ACE16 corpus, not engine codecs."""

import struct

PAGE_SIZE = 4096
PAGE_KIND_WIDTH = 1
PAGE_VERSION_OFFSET = 1
PAGE_SIGNATURE_WIDTH = 2
RETIRED_HEADER_WIDTH = 4
BITS_PER_BYTE = 8
REFERENCE_SLOT_BITS = 8
REFERENCE_SLOT_MASK = 255
DATA_SIGNATURE = bytes([1, 1])
BITMAP_SIGNATURE = bytes([5, 1, 0, 0])
PAGE_OWNER_OFFSET = 4
DATA_ROW_COUNT_OFFSET = 12
DATA_SLOT_START = 14
SLOT_WIDTH = 2
SLOT_FLAGS_MASK = 0xE000
SLOT_START_MASK = 0x1FFF
DELETED_SLOT_MASK = 0x8000
TOMBSTONE_MASK = 0xC000
INLINE_MAP = 0
INDIRECT_MAP = 1
MAP_BODY_OFFSET = 1
INLINE_BITS_OFFSET = 5
BITMAP_BODY_OFFSET = 4
POINTER_WIDTH = 4
BITMAP_BITS = (PAGE_SIZE - BITMAP_BODY_OFFSET) * BITS_PER_BYTE
TABLE_ROW_COUNT_OFFSET = 16
TABLE_COLUMN_COUNT_OFFSET = 45
TABLE_INDEX_COUNT_OFFSET = 51
TABLE_OWNED_MAP_OFFSET = 55
TABLE_DECLARATIONS_START = 63
INDEX_COUNTER_WIDTH = 12
COLUMN_DECLARATION_WIDTH = 25
NAME_LENGTH_WIDTH = 2
INDEX_DECLARATION_WIDTH = 52
INDEX_USAGE_OFFSET = 34
INDEX_ROOT_OFFSET = 38
NODE_KIND = 3
LEAF_KIND = 4
INDEX_FREE_OFFSET = 2
INDEX_PREVIOUS_OFFSET = 12
INDEX_NEXT_OFFSET = 16
INDEX_TAIL_OFFSET = 20
INDEX_PREFIX_OFFSET = 24
INDEX_HEIGHT_OFFSET = 26
INDEX_MASK_START = 27
INDEX_BODY_START = 480
INDEX_BODY_CAPACITY = PAGE_SIZE - INDEX_BODY_START
LEAF_TRAILER_WIDTH = 4
NODE_TRAILER_WIDTH = 8
GLOBAL_MAP_REFERENCE = 256
HEADER_FORMAT_OFFSET = 20
HEADER_SUBVERSION_OFFSET = 21


def u16(data, offset):
    return struct.unpack_from('<H', data, offset)[0]


def u32(data, offset):
    return struct.unpack_from('<I', data, offset)[0]


def be32(data, offset):
    return struct.unpack_from('>I', data, offset)[0]


def page(raw, number):
    start = number * PAGE_SIZE
    result = raw[start:start + PAGE_SIZE]
    assert len(result) == PAGE_SIZE
    return result


def row(raw, reference):
    number = reference >> REFERENCE_SLOT_BITS
    slot = reference & REFERENCE_SLOT_MASK
    data = page(raw, number)
    count = u16(data, DATA_ROW_COUNT_OFFSET)
    assert data[:len(DATA_SIGNATURE)] == DATA_SIGNATURE and slot < count
    word = u16(data, DATA_SLOT_START + SLOT_WIDTH * slot)
    assert word & SLOT_FLAGS_MASK == 0
    start = word & SLOT_START_MASK
    end = PAGE_SIZE if slot == 0 else u16(
        data, DATA_SLOT_START + SLOT_WIDTH * (slot - 1)
    ) & SLOT_START_MASK
    assert DATA_SLOT_START + SLOT_WIDTH * count <= start < end <= PAGE_SIZE
    return data[start:end]


def members(raw, reference):
    payload = row(raw, reference)
    result = []
    if payload[0] == INLINE_MAP:
        base = u32(payload, MAP_BODY_OFFSET)
        for byte, mask in enumerate(payload[INLINE_BITS_OFFSET:]):
            result.extend(base + byte * BITS_PER_BYTE + bit
                          for bit in range(BITS_PER_BYTE) if mask & (1 << bit))
    elif payload[0] == INDIRECT_MAP:
        assert (len(payload) - MAP_BODY_OFFSET) % POINTER_WIDTH == 0
        for block in range((len(payload) - MAP_BODY_OFFSET) // POINTER_WIDTH):
            pointer = u32(payload, MAP_BODY_OFFSET + block * POINTER_WIDTH)
            if pointer:
                bitmap = page(raw, pointer)
                assert bitmap[:BITMAP_BODY_OFFSET] == BITMAP_SIGNATURE
                for byte, mask in enumerate(bitmap[BITMAP_BODY_OFFSET:]):
                    result.extend(block * BITMAP_BITS + byte * BITS_PER_BYTE + bit
                                  for bit in range(BITS_PER_BYTE) if mask & (1 << bit))
    else:
        raise AssertionError('unknown map')

    assert result == sorted(set(result))
    assert all(number < len(raw) // PAGE_SIZE for number in result)
    return result


def physical(raw, definition, number):
    data = page(raw, definition)
    columns = u16(data, TABLE_COLUMN_COUNT_OFFSET)
    count = u32(data, TABLE_INDEX_COUNT_OFFSET)
    assert number < count
    offset = TABLE_DECLARATIONS_START + count * INDEX_COUNTER_WIDTH
    offset += columns * COLUMN_DECLARATION_WIDTH
    for _ in range(columns):
        offset += NAME_LENGTH_WIDTH + u16(data, offset)
    declaration = offset + number * INDEX_DECLARATION_WIDTH
    return (u32(data, declaration + INDEX_USAGE_OFFSET),
            u32(data, declaration + INDEX_ROOT_OFFSET))


def decode_tree(raw, number, owner, visited=None):
    visited = set() if visited is None else visited
    assert number not in visited
    visited.add(number)
    data = page(raw, number)
    assert data[0] in [NODE_KIND, LEAF_KIND] and data[1] == 1
    assert u32(data, PAGE_OWNER_OFFSET) == owner
    leaf = data[0] == LEAF_KIND
    ends = [byte * BITS_PER_BYTE + bit
            for byte, mask in enumerate(data[INDEX_MASK_START:INDEX_BODY_START])
            for bit in range(BITS_PER_BYTE) if mask & (1 << bit)]
    prefix = u16(data, INDEX_PREFIX_OFFSET)
    previous = 0
    first = None
    entries, leaves, nodes, children = [], [], [], []
    for end in ends:
        assert previous < end <= INDEX_BODY_CAPACITY
        fragment = data[INDEX_BODY_START + previous:INDEX_BODY_START + end]
        full = fragment if first is None else first[:prefix] + fragment
        if first is None:
            first = full
        assert prefix <= len(first)
        trailer = LEAF_TRAILER_WIDTH if leaf else NODE_TRAILER_WIDTH
        assert len(full) > trailer
        reference = be32(full, len(full) - trailer)
        if leaf:
            entries.append((full[:-trailer].hex(), reference >> REFERENCE_SLOT_BITS,
                            reference & REFERENCE_SLOT_MASK))
        else:
            children.append(be32(full, len(full) - POINTER_WIDTH))
        previous = end

    assert u16(data, INDEX_FREE_OFFSET) == INDEX_BODY_CAPACITY - previous
    assert ends or prefix == 0
    links = {'page': number, 'kind': data[0], 'height': data[INDEX_HEIGHT_OFFSET],
             'previous': u32(data, INDEX_PREVIOUS_OFFSET),
             'next': u32(data, INDEX_NEXT_OFFSET),
             'tail': u32(data, INDEX_TAIL_OFFSET)}
    if leaf:
        assert links['tail'] == 0
        leaves.append(links | {'entries': len(entries)})
    else:
        if links['tail']:
            children.append(links['tail'])
        nodes.append(links | {'children': children[:]})
        for child in children:
            keys, child_leaves, child_nodes = decode_tree(raw, child, owner, visited)
            entries.extend(keys)
            leaves.extend(child_leaves)
            nodes.extend(child_nodes)
    return entries, leaves, nodes
