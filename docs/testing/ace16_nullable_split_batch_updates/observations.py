"""Post-proof physical differences; these facts never supply constructor bytes."""


def compare(case, original_table, public, expected, facts, reader):
    owners = []
    for name in ('LeftOrder', 'PairOrder'):
        index = original_table['indexes'][name]
        if name == 'LeftOrder' or case['recipe'] == 'Full':
            count = 500
        else:
            count = 300 if case['recipe'] == 'Clear' else 202
        series = []
        for raw in (public, expected):
            nodes, _ = reader.tree(raw, index['root'], original_table['definition'],
                                   index['columns'], count)
            leaves = []
            for number, item in nodes.items():
                if item['height'] == 0:
                    page = raw[number * 4096:(number + 1) * 4096]
                    entries = sum(bool(page[27 + bit // 8] & (1 << (bit % 8)))
                                  for bit in range(1, 3617))
                    leaves.append(dict(page=number, entries=entries,
                                       prefix=item['prefix'], active=item['active']))
            series.append(leaves)
        owners.append(dict(physical=index['physical'], public_leaves=series[0],
                           canonical_leaves=series[1]))
    differences = {}
    for offset, (left, right) in enumerate(zip(public, expected)):
        if left != right:
            page = str(offset // 4096)
            differences[page] = differences.get(page, 0) + 1
    inactive = sum(left != right for cell in facts['inactive_null_payloads']
                   for left, right in zip(bytes.fromhex(cell['before']), bytes.fromhex(cell['after'])))
    return dict(id=case['id'], recipe=case['recipe'], public_bytes=len(public),
                canonical_bytes=len(expected), common_prefix_differing_bytes=sum(differences.values()),
                public_extra_bytes=len(public) - len(expected), byte_exact=public == expected,
                common_prefix_by_page=differences, physical=owners,
                new_public_index=facts['new_index_pages'], new_public_free=facts['new_free_pages'],
                inactive_requested_null_cells=len(facts['inactive_null_payloads']),
                inactive_null_payload_differing_bytes=inactive,
                histories=facts['histories'], warnings=facts['warnings'])
