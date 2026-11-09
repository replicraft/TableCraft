"""Prospective ordinary-schema INSERT composition from gated BEFORE facts.

No public after-image, producer byte, or production serializer is consumed.
The source adapter must qualify catalog, properties and all-table scalar facts
before passing its immutable six-tuple here.
"""
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'docs/testing'))
from ace16_insert_recursive import model as tree
from ace16_insert_ordinary import row_codec, record_decode


def derive(state, operands, primary_position):
    original, table, layout, source_maps, source_nodes, source_leaves = state
    row_layout = row_codec.Layout.bind(original, table)
    tree.require(0 < len(original) <= 8 * 1024 * 1024 and len(original) % tree.PAGE == 0,
                 'Closed source image bound')
    tree.require(0 < len(operands) <= 4096, 'Complete nonempty request batch')
    tree.require(table['count'] == len(table['rows']) == len(table['raw_rows'])
                 == tree.number(original, layout['table_count']), 'Actual complete live counter/rows')
    primary = [index for index in table['indexes'].values() if index['primary']]
    tree.require(len(primary) == 1, 'One actual declared primary')
    selected = primary[0]
    tree.require(selected['unique'] and selected['required'] and not selected['ignored_null']
                 and len(selected['columns']) == 1 and selected['columns'][0][1] == 'true',
                 'Actual required unique ascending scalar primary')
    tree.require(selected['columns'][0][0] == row_layout.fields[primary_position].name,
                 'Actual primary field rather than first Int32')
    tree.require(selected['root'] == layout['root'] and selected['physical'] == 0
                 and tree.number(original, layout['definition'] * tree.PAGE + 51) == 1,
                 'Actual physical primary identity')
    for alias in table['indexes'].values():
        tree.require(alias['physical'] == selected['physical'] and alias['root'] == selected['root']
                     and alias['columns'] == selected['columns'] and alias['unique']
                     and alias['required'] and not alias['ignored_null'], 'True physical logical aliases')
    tree.require(row_layout.fields[primary_position].native == 4, 'Actual primary Int32')
    requests = []
    for values in operands:
        tree.require(values[primary_position][0] == 'int32', 'Explicit primary carrier')
        requests.append((values[primary_position][1], row_codec.encode(row_layout, values)))
    tree.require(len({identity for identity, _ in requests}) == len(requests), 'Request primary uniqueness')
    tree.require(not set(table['rows']) & {identity for identity, _ in requests}, 'Original primary uniqueness')
    owner = layout['definition']
    top, nodes, source_levels = tree.decode_tree(original, layout['root'], owner)
    tree.require(set(nodes) == set(source_nodes) and source_levels[0] == source_leaves,
                 'Complete source index hierarchy')
    source_refs = {}
    for identity, row in table['rows'].items():
        reference = row['page'] << 8 | row['slot']
        tree.require(reference not in source_refs, 'Original ROW uniqueness')
        start, end = tree.WIRE.frame(original, row['page'], row['slot'])
        tree.require(original[start:end].hex() == table['raw_rows'][identity], 'Original raw row identity')
        decoded = record_decode.decode(row_layout, original[start:end])
        record_decode.check_getter(row_layout, decoded, row['values'])
        tree.require(decoded[primary_position] == ('int32', identity), 'Raw source primary value')
        source_refs[reference] = identity
    source_wires = [entry for page in source_levels[0] for entry in nodes[page].entries]
    tree.require(source_wires == sorted(tree.key(identity, ref) for ref, identity in source_refs.items()),
                 'Original actual primary full key/ROW stream')
    image = bytearray(original)
    overlays = {role: tree.MapOverlay(original, layout[role], role)
                for role in ('global', 'owned', 'hint', 'index_owned')}
    tree.require(len({overlay.reference for overlay in overlays.values()}) == 4, 'Distinct canonical map roles')
    for role, overlay in overlays.items():
        tree.require(overlay.before['range'] == source_maps[role]['range']
                     and overlay.members == source_maps[role]['members'], 'Source map identity')
    tree.require(overlays['index_owned'].members == set(nodes), 'Exact original index ownership')
    tree.require(overlays['hint'].members <= overlays['owned'].members, 'Hint subset')
    tree.require(not overlays['global'].members & (overlays['owned'].members | set(nodes)),
                 'Original selected global allocation')
    purposes = {}

    def allocate(purpose):
        page = len(image) // tree.PAGE
        tree.require((page + 1) * tree.PAGE <= tree.MAX_IMAGE, 'Candidate image bound')
        tree.require(page in source_maps['global']['members'], 'Original represented global free cell')
        overlays['global'].set_represented(page, False)
        image.extend(bytes(tree.PAGE))
        purposes[page] = purpose
        return page

    new_refs = []
    for identity, body in requests:
        destination = None
        count, end = 0, tree.PAGE
        if overlays['hint'].members:
            highest = max(overlays['hint'].members)
            tree.require(highest < len(image) // tree.PAGE, 'Hint in current image')
            count, end = tree.geometry(image[highest * tree.PAGE:(highest + 1) * tree.PAGE], owner)
            if count < 255 and len(body) + 2 <= end - 14 - 2 * count:
                destination = highest
            else:
                overlays['hint'].set_represented(highest, False)
        if destination is None:
            destination = allocate('DATA')
            overlays['owned'].set_represented(destination, True)
            overlays['hint'].add_growing(destination, allocate)
            image[destination * tree.PAGE:destination * tree.PAGE + 2] = b'\x01\x01'
            tree.put(image, destination * tree.PAGE + 4, owner)
            count, end = 0, tree.PAGE
        at = destination * tree.PAGE
        start = end - len(body)
        tree.require(count < 255 and start >= 16 + 2 * count, 'Visible new slot capacity')
        image[at + start:at + end] = body
        tree.put(image, at + 14 + count * 2, start, 2)
        tree.put(image, at + 12, count + 1, 2)
        tree.put(image, at + 2, start - 16 - 2 * count, 2)
        reference = destination << 8 | count
        new_refs.append(reference)
        tree.route(top, tree.key(identity, reference)).entries.append(tree.key(identity, reference))
    for node in nodes.values():
        node.entries.sort()
    creation = []
    tree.require(tree.normalize(top, top, creation) == [top], 'Retained root identity')
    for node in creation:
        node.number = allocate('INDEX')
        overlays['index_owned'].add_growing(node.number, allocate)
    levels = tree.levels_of(top)
    for level in levels.values():
        for position, node in enumerate(level):
            previous = level[position - 1].number if position else 0
            following = level[position + 1].number if position + 1 < len(level) else 0
            image[node.number * tree.PAGE:(node.number + 1) * tree.PAGE] = tree.render(
                node, original, owner, previous, following)
    for overlay in overlays.values():
        overlay.write(image)
    history = tree.number(original, layout['history'])
    tree.require(history + len(requests) < 1 << 32, 'Historical u32 counter')
    tree.put(image, layout['history'], history + len(requests))
    tree.put(image, layout['table_count'], table['count'] + len(requests))
    after_top, after_nodes, after_levels = tree.decode_tree(image, layout['root'], owner)
    all_refs = dict(source_refs)
    all_refs.update({ref: identity for (identity, _), ref in zip(requests, new_refs, strict=True)})
    wires = [entry for page in after_levels[0] for entry in after_nodes[page].entries]
    tree.require(wires == sorted(tree.key(identity, ref) for ref, identity in all_refs.items()),
                 'Complete final primary key/ROW stream')
    tree.require(overlays['index_owned'].members == set(after_nodes), 'Exact final index membership')
    for ref in source_refs:
        start, end = tree.WIRE.frame(original, ref >> 8, ref & 255)
        after_start, after_end = tree.WIRE.frame(image, ref >> 8, ref & 255)
        tree.require((start, end) == (after_start, after_end) and image[start:end] == original[start:end],
                     'Original logical ROW/range/payload preservation')
    for values, (_, body), ref in zip(operands, requests, new_refs, strict=True):
        start, end = tree.WIRE.frame(image, ref >> 8, ref & 255)
        tree.require(image[start:end] == body, 'Requested new record and ROW')
        record_decode.same_values(record_decode.decode(row_layout, bytes(image[start:end])), values)
    metadata = {page for page, purpose in purposes.items() if purpose.endswith('Bitmap')}
    tree.require(not metadata & (overlays['owned'].members | overlays['hint'].members
                                 | overlays['index_owned'].members), 'Metadata excluded from memberships')
    tree.require(set(purposes) == set(range(len(original) // tree.PAGE, len(image) // tree.PAGE)),
                 'Complete suffix purpose partition')
    return bytes(image), {
        'source_rows': table['count'], 'final_rows': table['count'] + len(requests),
        'history_before': history, 'history_after': history + len(requests),
        'primary_position': primary_position, 'fixed_end': row_layout.fixed_end,
        'variable_positions': list(row_layout.variables), 'new_ROWS': new_refs,
        'new_record_bytes': [len(body) for _, body in requests],
        'fresh_INDEX_creation_order': [node.number for node in creation],
        'suffix_purposes': {str(page): purpose for page, purpose in sorted(purposes.items())},
        'root_height_before': source_nodes[layout['root']]['height'],
        'root_height_after': after_top.height, 'old_ROWS_preserved': len(source_refs),
        'final_key_ROW_pairs': len(wires), 'bytes_added': len(image) - len(original),
    }
