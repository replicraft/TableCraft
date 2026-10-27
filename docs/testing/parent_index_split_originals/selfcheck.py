"""Pure in-memory arithmetic/codec controls; no database generation or processes."""
import json
import wire


def entries(count, step):
    return [wire.encoded((position + 1) * step, (0x10000 + position).to_bytes(4, 'big'))
            for position in range(count)]


def main():
    parent = entries(328, 301)
    assert wire.best_size(parent, True) == 3610
    parent.append(wire.encoded(100_000, (0x20000).to_bytes(4, 'big')))
    assert wire.best_size(parent, True) == 3621
    leaf = [wire.encoded(99_001 + position, (0x30000 + position).to_bytes(4, 'big'))
            for position in range(480)]
    assert wire.best_size(leaf, False) <= 3616
    leaf[-1] = wire.encoded(2147483647, leaf[-1][-4:])
    assert wire.best_size(leaf, False) == 3841
    assert wire.best_size(leaf[:240], False) <= 3616
    assert wire.best_size(leaf[240:], False) <= 3616
    assert wire.key(leaf[-1]) == 2147483647
    # NODE capacity excludes child-pointer bytes from shared key/ROW prefix.
    assert wire.best_size([wire.encoded(3, bytes(4))], True) == 13
    assert 329 - 1 + 453 == 781 < 16_384
    print(json.dumps({'pure_assertions': 9, 'native_or_java_calls': 0,
                      'databases_generated': 0, 'actual_builder_geometry_claimed': False}))


if __name__ == '__main__':
    main()
