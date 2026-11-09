"""Prospective source/request-only recursive Int32/Binary INSERT images.

No fixture process, production serializer or public AFTER bytes are used.
A mathematical image is not engine admission or a full foreign-catalog census.
"""
from copy import deepcopy
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_candidates import model as WIRE
from slot_preview_originals import facts as GETTER
import delete_reference_maps as MAPS

PAGE = 4096
CAPACITY = 3616
MAX_IMAGE = 64 * 1024 * 1024
MAX_DEPTH = 16
MAX_SOURCE_ROWS = 4096
MAX_REQUEST_ROWS = 4096
BITMAP_BITS = 32736


class Ineligible(ValueError):
    """The actual source/request exceeds this explicit prospective model."""


def require(condition, message):
    if not condition:
        raise Ineligible(message)


def number(raw, offset, width=4):
    value = raw[offset:offset + width]
    require(len(value) == width, 'Incomplete scalar')
    return int.from_bytes(value, 'little')


def put(raw, offset, value, width=4):
    raw[offset:offset + width] = value.to_bytes(width, 'little')


def key(identity, reference):
    require(type(identity) is int and -(1 << 31) <= identity < 1 << 31, 'Int32 key required')
    require(0 <= reference < 1 << 32, 'ROW width')
    return b'\x7f' + ((identity & 0xffffffff) ^ 0x80000000).to_bytes(4, 'big') + reference.to_bytes(4, 'big')


def record(identity, binary):
    key(identity, 0)
    require(binary is None or type(binary) is bytes and len(binary) <= 255, 'Binary255 or NULL required')
    payload = b'\x02\x00' + identity.to_bytes(4, 'little', signed=True)
    if binary is not None:
        payload += binary
    return payload + len(payload).to_bytes(2, 'little') + b'\x06\x00\x01\x00' + bytes([1 if binary is None else 3])


def common(entries):
    if not entries:
        return 0
    width = 9
    for entry in entries[1:]:
        equal = 0
        while equal < width and entries[0][equal] == entry[equal]:
            equal += 1
        width = equal
    return width


class Node:
    def __init__(self, number_, height, entries, children, tail, seed, original):
        self.number = number_
        self.height = height
        self.entries = list(entries)
        self.children = list(children)
        self.tail = tail
        self.seed = seed
        self.original = original

    def maximum(self):
        return self.entries[-1] if self.height == 0 else self.children[-1].maximum()

    def bounds(self):
        if self.height == 0:
            return self.entries
        count = len(self.children) - int(self.tail)
        return [child.maximum() for child in self.children[:count]]

    def measurement(self):
        entries = self.bounds()
        if not entries:
            return 0, 0
        best = common(entries)
        seed = min(self.seed, best)
        size = len(entries) * (9 if self.height == 0 else 13) - (len(entries) - 1) * seed
        if size > CAPACITY:
            seed = best
            size = len(entries) * (9 if self.height == 0 else 13) - (len(entries) - 1) * seed
        return seed, size


def decode_tree(raw, root, owner):
    """Decode complete 9-byte key/ROW wires and every height's sibling chain."""
    nodes = {}
    levels = {}

    def walk(number_, depth):
        require(depth < MAX_DEPTH and number_ not in nodes, 'Tree depth/cycle/duplicate child')
        require(0 < number_ < len(raw) // PAGE, 'Tree child outside source')
        page = raw[number_ * PAGE:(number_ + 1) * PAGE]
        height = page[26]
        require(page[:2] == bytes([4 if height == 0 else 3, 1]), 'Index signature/height')
        require(number(page, 4) == owner and page[8:12] == bytes(4), 'Index owner/stamp')
        prefix = number(page, 24, 2)
        require(prefix <= 9, 'Full key/ROW prefix width')
        active = CAPACITY - number(page, 2, 2)
        require(0 <= active <= CAPACITY, 'Index free word')
        entries = []
        child_numbers = []
        start = 0
        for end in range(1, CAPACITY + 1):
            if page[27 + end // 8] & (1 << (end % 8)) == 0:
                continue
            require(end <= active and end > start, 'Index endpoint outside active range')
            chunk = page[480 + start:480 + end]
            if height:
                require(len(chunk) >= 4, 'Missing child reference')
                child_numbers.append(int.from_bytes(chunk[-4:], 'big'))
                chunk = chunk[:-4]
            full = chunk if not entries else entries[0][:prefix] + chunk
            require(len(full) == 9 and full[0] == 127, 'Int32/ROW wire')
            entries.append(full)
            start = end
        require(start == active, 'Unframed active bytes')
        require(all(left < right for left, right in zip(entries, entries[1:])), 'Index wire order')
        tail_number = number(page, 20)
        require(height or tail_number == 0, 'Leaf tail')
        require(entries or height and tail_number and prefix == 0, 'Empty index shape')
        node = Node(number_, height, entries, [], bool(tail_number), prefix, number_)
        nodes[number_] = node
        levels.setdefault(height, []).append(number_)
        if height:
            for bound, child_number in zip(entries, child_numbers, strict=True):
                child = walk(child_number, depth + 1)
                require(child.height + 1 == height and child.maximum() == bound, 'Original bound/height')
                node.children.append(child)
            if tail_number:
                child = walk(tail_number, depth + 1)
                require(child.height + 1 == height, 'Tail height')
                node.children.append(child)
        return node

    top = walk(root, 0)
    for level in levels.values():
        for index, page in enumerate(level):
            previous = level[index - 1] if index else 0
            following = level[index + 1] if index + 1 < len(level) else 0
            require(number(raw, page * PAGE + 12) == previous and number(raw, page * PAGE + 16) == following,
                    'Per-height original sibling chain')
    return top, nodes, levels


def geometry(page, owner):
    require(len(page) == PAGE and page[:2] == b'\x01\x01' and number(page, 4) == owner, 'Ordinary DATA owner/signature')
    count = number(page, 12, 2)
    require(count <= 255, 'Physical slot count')
    directory_end = 14 + 2 * count
    end = PAGE
    for slot in range(count):
        word = number(page, 14 + slot * 2, 2)
        require(word & 0x2000 == 0, 'Unsupported slot flag')
        start = word & 0x1fff
        require(directory_end <= start <= end and (start < end or word & 0x8000), 'Physical slot range')
        end = start
    require(number(page, 2, 2) == end - directory_end, 'Exact tail/free word; no compaction assumption')
    return count, end


def route(node, wire):
    while node.height:
        boundaries = zip(node.entries[:len(node.children) - 1], node.children[:-1], strict=True)
        node = next((child for bound, child in boundaries if wire <= bound), node.children[-1])
    return node


def divide(right, creation):
    entries = right.entries if right.height == 0 else right.children
    middle = len(entries) // 2
    require(0 < middle < len(entries), 'Indivisible overflow')
    left = Node(None, right.height, entries[:middle] if right.height == 0 else [],
                entries[:middle] if right.height else [], bool(right.height), 0, None)
    if right.height:
        right.children = entries[middle:]
    else:
        right.entries = entries[middle:]
    creation.append(left)
    return left


def finish_local(right, left, creation):
    work = [right, left]
    logical = [left, right]
    while True:
        changed = False
        position = 0
        while position < len(work):
            node = work[position]
            if node.measurement()[1] > CAPACITY:
                added = divide(node, creation)
                logical.insert(logical.index(node), added)
                work.append(added)
                changed = True
            position += 1
        if not changed:
            return logical


def normalize(node, root, creation):
    if node.height:
        original_children = node.children
        node.children = []
        for child in original_children:
            node.children.extend(normalize(child, root, creation))
    if node.measurement()[1] <= CAPACITY:
        return [node]
    if node is not root:
        return finish_local(node, divide(node, creation), creation)
    old_height = node.height
    require(old_height + 1 < MAX_DEPTH, 'Root growth depth ceiling')
    right = Node(None, old_height, node.entries, node.children, bool(old_height), 0, None)
    creation.append(right)
    left = divide(right, creation)
    children = finish_local(right, left, creation)
    node.height += 1
    node.seed = 0
    node.tail = old_height == 0
    node.entries = []
    node.children = children
    require(node.measurement()[1] <= CAPACITY, 'One growth leaves a non-fitting new top')
    return [node]


def levels_of(top):
    levels = {}

    def visit(node):
        levels.setdefault(node.height, []).append(node)
        for child in node.children:
            visit(child)

    visit(top)
    return levels


def render(node, original, owner, previous, following):
    page = bytearray(PAGE) if node.original is None else bytearray(original[node.original * PAGE:(node.original + 1) * PAGE])
    entries = node.bounds()
    prefix, active = node.measurement()
    require(active <= CAPACITY, 'Unnormalized output')
    page[:2] = bytes([4 if node.height == 0 else 3, 1])
    put(page, 2, CAPACITY - active, 2)
    put(page, 4, owner)
    put(page, 12, previous)
    put(page, 16, following)
    put(page, 20, node.children[-1].number if node.height and node.tail else 0)
    put(page, 24, prefix, 2)
    page[26] = node.height
    page[27:480] = bytes(453)
    end = 0
    for position, entry in enumerate(entries):
        suffix = entry if position == 0 else entry[prefix:]
        page[480 + end:480 + end + len(suffix)] = suffix
        end += len(suffix)
        if node.height:
            page[480 + end:484 + end] = node.children[position].number.to_bytes(4, 'big')
            end += 4
        page[27 + end // 8] |= 1 << (end % 8)
    require(end == active, 'Rendered size')
    if node.original is None:
        require(page[480 + active:] == bytes(CAPACITY - active), 'Fresh unused bytes')
    else:
        source = original[node.original * PAGE:(node.original + 1) * PAGE]
        require(page[480 + active:] == source[480 + active:], 'Original residue')
    return page


class MapOverlay:
    def __init__(self, raw, reference, role):
        source = WIRE.map_record(raw, reference)
        self.reference = reference
        self.role = role
        self.members = set(source['members'])
        self.start, self.end = source['range']
        self.row = bytearray(raw[self.start:self.end])
        self.base = number(self.row, 1) if self.row[0] == 0 else None
        self.bitmap = number(self.row, 1) if self.row[0] else None
        self.before = source
        self.rewritten = False

    def represented(self, number_):
        return (self.base <= number_ < self.base + 512) if self.base is not None else number_ < BITMAP_BITS

    def set_represented(self, number_, present):
        require(self.represented(number_), self.role + ' lacks represented future cell')
        if present:
            self.members.add(number_)
        else:
            self.members.discard(number_)

    def add_growing(self, number_, allocate):
        if self.represented(number_):
            self.members.add(number_)
            return
        require(self.base is not None, 'Indirect map span expansion is not modeled')
        wanted = self.members | {number_}
        aligned = min(wanted) // 8 * 8
        if max(wanted) - aligned + 1 < 512:
            self.base = aligned
            self.members = wanted
            self.rewritten = True
        else:
            purposes = {'hint': 'OrdinaryCandidateBitmap', 'index_owned': 'IndexOwnedBitmap'}
            require(self.role in purposes, 'This role cannot grow a map')
            self.bitmap = allocate(purposes[self.role])
            require(max(wanted) < BITMAP_BITS, 'First bitmap span only')
            self.base = None
            self.members = wanted
            self.rewritten = True

    def write(self, image):
        if self.base is not None:
            row = bytearray(69)
            put(row, 1, self.base)
            for number_ in self.members:
                require(self.base <= number_ < self.base + 512, 'Inline membership range')
                relative = number_ - self.base
                row[5 + relative // 8] |= 1 << (relative % 8)
            image[self.start:self.end] = row
            return
        if self.rewritten:
            row = bytearray(69)
            row[0] = 1
            put(row, 1, self.bitmap)
            image[self.start:self.end] = row
            page = bytearray(PAGE)
            page[:4] = b'\x05\x01\x00\x00'
            for number_ in self.members:
                page[4 + number_ // 8] |= 1 << (number_ % 8)
            image[self.bitmap * PAGE:(self.bitmap + 1) * PAGE] = page
        else:
            for number_ in self.before['members'] ^ self.members:
                offset, mask = self.before['locations'][number_]
                image[offset] = image[offset] | mask if number_ in self.members else image[offset] & ~mask


def derive(state, requests):
    """Use immutable source state and ordered (Int32, Binary|None) requests only."""
    original, table, layout, source_maps, source_nodes, source_leaves = state
    require(0 < len(original) <= MAX_IMAGE and len(original) % PAGE == 0, 'Source image bound')
    require(0 < table['count'] <= MAX_SOURCE_ROWS, 'Live source row bound')
    require(0 < len(requests) <= MAX_REQUEST_ROWS, 'Request bound')
    require(len(table['rows']) == len(table['raw_rows']) == table['count'], 'Complete source rows')
    require(len({identity for identity, _ in requests}) == len(requests), 'Repeated requested identity')
    require(not set(table['rows']) & {identity for identity, _ in requests}, 'Duplicate primary identity')
    owner = layout['definition']
    definition = owner * PAGE
    require(number(original, definition + 41, 2) == number(original, definition + 45, 2) == 2,
            'Exact physical/historical column geometry')
    require(number(original, definition + 43, 2) == 1 and number(original, definition + 47) == number(original, definition + 51) == 1,
            'One variable slot and one logical/physical primary')
    require(layout['table_count'] == definition + 16 and layout['history'] == definition + 67,
            'Actual native counter spans')
    for position, (kind, width, flags) in enumerate([(4, 4, 3), (9, 255, 2)]):
        column = definition + 75 + position * 25
        require(original[column] == kind and number(original, column + 5, 2) == position,
                'Actual native Int32/Binary order')
        require(original[column + 15:column + 17] == bytes([flags, 0]) and number(original, column + 23, 2) == width,
                'Closed field flags/width')
        require(number(original, column + (21 if position == 0 else 7), 2) == 0,
                'Original fixed offset/variable ordinal')
    require(number(original, layout['table_count']) == table['count'], 'Actual live counter')
    history = number(original, layout['history'])
    require(history + len(requests) < 1 << 32, 'Historical u32 increment')
    top, nodes, source_levels = decode_tree(original, layout['root'], owner)
    require(set(nodes) == set(source_nodes), 'Complete original tree inventory')
    require(source_levels[0] == source_leaves, 'Original leaf order')
    source_refs = {}
    for identity, row in table['rows'].items():
        binary_value = row['values'][1]
        require(row['values'][0] == 'Integer:' + str(identity), 'Source Int32 carrier')
        require(binary_value == 'NULL' or binary_value.startswith('Binary:'), 'Source Binary carrier')
        binary = None if binary_value == 'NULL' else bytes.fromhex(binary_value[7:])
        begin, end = WIRE.frame(original, row['page'], row['slot'])
        require(original[begin:end] == record(identity, binary), 'Original record/footer/mask')
        require(original[begin:end].hex() == table['raw_rows'][identity], 'Raw getter source record')
        reference = row['page'] << 8 | row['slot']
        require(reference not in source_refs, 'Duplicate source ROW')
        source_refs[reference] = identity
    source_wires = [entry for number_ in source_levels[0] for entry in nodes[number_].entries]
    require(source_wires == sorted(key(identity, reference) for reference, identity in source_refs.items()), 'Source key/ROW membership')
    image = bytearray(original)
    overlays = {role: MapOverlay(original, layout[role], role)
                for role in ['global', 'owned', 'hint', 'index_owned']}
    for role, overlay in overlays.items():
        require(overlay.before['members'] == source_maps[role]['members'], 'Original map membership')
        require(overlay.before['range'] == source_maps[role]['range'], 'Original map row identity')
    require(len({overlay.reference for overlay in overlays.values()}) == 4, 'Distinct selected map roles')
    storage = list(overlays.values())
    for position, left in enumerate(storage):
        for right in storage[position + 1:]:
            require(left.end <= right.start or right.end <= left.start, 'Selected map record overlap')
            require(left.bitmap is None or right.bitmap is None or left.bitmap != right.bitmap,
                    'Selected role bitmap alias')
    require(overlays['index_owned'].members == set(nodes), 'Original exact physical-index ownership')
    for page_number in sorted(overlays['owned'].members):
        require(page_number < len(original) // PAGE, 'Ordinary-owned future membership')
        page = original[page_number * PAGE:(page_number + 1) * PAGE]
        physical_count, _ = geometry(page, owner)
        for slot in range(physical_count):
            word = number(page, 14 + slot * 2, 2)
            if word & 0x8000 == 0:
                require(word & ~0x1fff == 0 and (page_number << 8 | slot) in source_refs,
                        'Complete normal live slot inventory')
    require(overlays['hint'].members <= overlays['owned'].members, 'Original candidate subset')
    require(not overlays['global'].members & (overlays['owned'].members | set(nodes)), 'Original selected global-free collision')
    purposes = {}

    def allocate(purpose):
        number_ = len(image) // PAGE
        require((number_ + 1) * PAGE <= MAX_IMAGE, 'Output image bound')
        overlays['global'].set_represented(number_, False)
        require(number_ in source_maps['global']['members'], 'EOF must originally be free')
        image.extend(bytes(PAGE))
        purposes[number_] = purpose
        return number_

    references = []
    for identity, binary in requests:
        payload = record(identity, binary)
        destination = None
        count, end = 0, PAGE
        if overlays['hint'].members:
            highest = max(overlays['hint'].members)
            require(highest < len(image) // PAGE, 'Hint outside actual image')
            count, end = geometry(image[highest * PAGE:(highest + 1) * PAGE], owner)
            if count < 255 and len(payload) + 2 <= end - (14 + 2 * count):
                destination = highest
            else:
                overlays['hint'].set_represented(highest, False)
        if destination is None:
            destination = allocate('DATA')
            overlays['owned'].set_represented(destination, True)
            overlays['hint'].add_growing(destination, allocate)
            start_page = destination * PAGE
            image[start_page:start_page + 2] = b'\x01\x01'
            put(image, start_page + 4, owner)
            count, end = 0, PAGE
        start_page = destination * PAGE
        start = end - len(payload)
        require(count < 255 and start >= 16 + 2 * count, 'Visible append capacity')
        image[start_page + start:start_page + end] = payload
        put(image, start_page + 14 + 2 * count, start, 2)
        put(image, start_page + 12, count + 1, 2)
        put(image, start_page + 2, start - 16 - 2 * count, 2)
        reference = destination << 8 | count
        references.append(reference)
        target = route(top, key(identity, reference))
        target.entries.append(key(identity, reference))
    for node in nodes.values():
        node.entries.sort()
    creation = []
    require(normalize(top, top, creation) == [top], 'Retained root identity')
    for node in creation:
        node.number = allocate('INDEX')
        overlays['index_owned'].add_growing(node.number, allocate)
    levels = levels_of(top)
    for level in levels.values():
        for position, node in enumerate(level):
            previous = level[position - 1].number if position else 0
            following = level[position + 1].number if position + 1 < len(level) else 0
            page = render(node, original, owner, previous, following)
            image[node.number * PAGE:(node.number + 1) * PAGE] = page
    for overlay in overlays.values():
        overlay.write(image)
    put(image, layout['table_count'], table['count'] + len(requests))
    put(image, layout['history'], history + len(requests))
    after_top, after_nodes, after_levels = decode_tree(image, layout['root'], owner)
    final_refs = dict(source_refs)
    final_refs.update({reference: identity for (identity, _), reference in zip(requests, references, strict=True)})
    require(len(final_refs) == len(source_refs) + len(requests), 'New logical ROW identity')
    after_wires = [entry for number_ in after_levels[0] for entry in after_nodes[number_].entries]
    require(after_wires == sorted(key(identity, reference) for reference, identity in final_refs.items()), 'Final full key/ROW membership')
    require(set(after_nodes) == overlays['index_owned'].members, 'Exact index-owned nodes; no bitmap member')
    require(not set(purposes) & set(nodes), 'No original-page reservation')
    for reference, identity in source_refs.items():
        before_begin, before_end = WIRE.frame(original, reference >> 8, reference & 255)
        after_begin, after_end = WIRE.frame(image, reference >> 8, reference & 255)
        require((before_begin, before_end) == (after_begin, after_end), 'Original record range')
        require(image[after_begin:after_end] == original[before_begin:before_end], 'Original record bytes')
    for (identity, binary), reference in zip(requests, references, strict=True):
        begin, end = WIRE.frame(image, reference >> 8, reference & 255)
        require(image[begin:end] == record(identity, binary), 'Inserted normal record and ROW')
    require(set(purposes) == set(range(len(original) // PAGE, len(image) // PAGE)),
            'Complete consecutive suffix-purpose partition')
    require(not set(purposes) & overlays['global'].members, 'Allocated suffix cannot remain globally free')
    for number_, role in purposes.items():
        signature = image[number_ * PAGE:number_ * PAGE + 2]
        if role == 'DATA':
            require(signature == b'\x01\x01' and number_ in overlays['owned'].members,
                    'DATA purpose and ordinary ownership')
            require(number_ not in overlays['index_owned'].members, 'DATA cannot be an index member')
        elif role == 'INDEX':
            require(signature in (b'\x03\x01', b'\x04\x01') and number_ in overlays['index_owned'].members,
                    'INDEX purpose and membership')
            require(number_ not in overlays['owned'].members and number_ not in overlays['hint'].members,
                    'INDEX cannot be ordinary owned/candidate membership')
        else:
            require(role in ('OrdinaryCandidateBitmap', 'IndexOwnedBitmap')
                    and image[number_ * PAGE:number_ * PAGE + 4] == b'\x05\x01\x00\x00',
                    'Closed metadata purpose/signature')
    metadata = {number_ for number_, role in purposes.items() if role.endswith('Bitmap')}
    require(not metadata & overlays['owned'].members and not metadata & overlays['hint'].members,
            'Metadata is never ordinary owned/hint membership')
    require(not metadata & overlays['index_owned'].members, 'Metadata is never index membership')
    return bytes(image), {
        'definition': owner, 'map_references': {role: layout[role] for role in overlays},
        'requested_rows': [{'ID': identity, 'binary_hex': None if binary is None else binary.hex(),
                            'reference': reference}
                           for (identity, binary), reference in zip(requests, references, strict=True)],
        'source_live_rows': table['count'], 'final_live_rows': table['count'] + len(requests),
        'history_before': history, 'history_after': history + len(requests),
        'root': layout['root'], 'root_height_before': source_nodes[layout['root']]['height'],
        'root_height_after': after_top.height, 'new_ROWS': references,
        'new_record_bytes': [len(record(identity, binary)) for identity, binary in requests],
        'fresh_INDEX_creation_order': [node.number for node in creation],
        'logical_pages_per_height': {str(height): [node.number for node in level] for height, level in sorted(levels.items())},
        'suffix_purposes': {str(number_): role for number_, role in sorted(purposes.items())},
        'source_old_ROWS_preserved': len(source_refs), 'final_key_ROW_pairs': len(after_wires),
        'bytes_added': len(image) - len(original),
    }


def parsed_getter(text):
    """Reject duplicate/unknown boolean facts before shared pure interpretation."""
    seen = set()
    for line in text.splitlines():
        fields = line.split('\t')
        tag = fields[0]
        if tag in ('TABLE', 'COLUMN', 'ROW', 'RAWROW', 'INDEX'):
            width = 2 if tag == 'TABLE' else 3
            identity = tuple(fields[:width])
            require(identity not in seen, 'Duplicate typed/raw getter identity')
            seen.add(identity)
        elif tag in ('PROPERTY', 'INDEX_COLUMN', 'ENTRY'):
            identity = tuple(fields)
            require(identity not in seen, 'Duplicate getter detail')
            seen.add(identity)
        if tag == 'COLUMN':
            require(fields[7] in ('true', 'false'), 'Unknown compression Boolean')
        if tag == 'INDEX':
            require(all(value in ('true', 'false') for value in fields[7:11]), 'Unknown index Boolean')
        if tag == 'INDEX_COLUMN':
            require(fields[-1] in ('true', 'false'), 'Unknown direction Boolean')
    return GETTER.parse(text)


def verify_fresh(image, details, before_tables, target_name, getter_text, maps_text):
    """Check model ROW/value/tree/maps, never public AFTER stream equality.

    Callers must first compare the complete actual candidate against `image`.
    This function then interprets the fresh read-only getter for that candidate.
    """
    tables = parsed_getter(getter_text)
    require(set(tables) == set(before_tables), 'Complete fresh table inventory')
    for name, before in before_tables.items():
        if name != target_name:
            require(tables[name] == before, 'Unselected table typed/raw/index preservation')
    expected = deepcopy(before_tables[target_name])
    require(expected['count'] == details['source_live_rows'], 'Original selected table count')
    expected['count'] = details['final_live_rows']
    for request in details['requested_rows']:
        identity = request['ID']
        binary = None if request['binary_hex'] is None else bytes.fromhex(request['binary_hex'])
        reference = request['reference']
        expected['rows'][identity] = {'page': reference >> 8, 'slot': reference & 255,
                                      'values': ['Integer:' + str(identity),
                                                 'NULL' if binary is None else 'Binary:' + binary.hex()]}
        expected['raw_rows'][identity] = record(identity, binary).hex()
    top, nodes, levels = decode_tree(image, details['root'], details['definition'])
    require(top.height == details['root_height_after'], 'Expected root height')
    require({str(height): level for height, level in sorted(levels.items())} == details['logical_pages_per_height'],
            'Complete fresh hierarchy')
    for label, index in expected['indexes'].items():
        require(index['root'] == details['root'] and index['physical'] == 0,
                'Unmodeled affected physical index')
        index['entries'] += len(details['requested_rows'])
        index['owned_pages'] = len(nodes)
        index['order'] = sorted(expected['rows'])
    require(tables[target_name] == expected, 'Complete selected metadata/values/ROWs/raw bodies/index order')
    lines = maps_text.splitlines()
    locations = [line.split('\t')[1:] for line in lines if line.startswith('LOCATION\t')]
    require(len(locations) <= 1, 'Duplicate fresh LOCATION')
    if locations:
        fields = list(map(int, locations[0]))
        wanted = [[reference >> 8, reference & 255] * 2 for reference in details['new_ROWS']]
        require(fields in wanted, 'Fresh LOCATION must use model-derived new ROW')
    description = MAPS.parse('\n'.join(line for line in lines if not line.startswith('LOCATION\t')))
    require(description['page_bytes'] == PAGE and description['page_count'] == len(image) // PAGE,
            'Actual candidate map geometry')
    require(set(description['maps']) == {'JackcessGlobalFree', 'TableOwned', 'TableFreeSpace'}, 'Complete selected map inventory')
    for label, role in [('JackcessGlobalFree', 'global'), ('TableOwned', 'owned'), ('TableFreeSpace', 'hint')]:
        decoded = MAPS.decode_map(image, description['maps'][label], PAGE, len(image) // PAGE)
        direct = WIRE.map_record(image, details['map_references'][role])
        require(decoded['row_start'] == direct['range'][0], 'Fresh map record identity')
        require(set(decoded['file_members']) == {number_ for number_ in direct['members'] if number_ < len(image) // PAGE},
                'Fresh complete represented membership')
    index_owned = WIRE.map_record(image, details['map_references']['index_owned'])
    require(index_owned['members'] == set(nodes), 'Index membership excludes DATA/bitmap purposes')
    return {'verified': True, 'typed_rows': sum(table['count'] for table in tables.values()),
            'selected_rows': expected['count'], 'primary_key_ROW_pairs': expected['count'],
            'new_ROWS': details['new_ROWS'], 'table_inventory': sorted(tables),
            'reader_equality_to_public_AFTER_required': False}
