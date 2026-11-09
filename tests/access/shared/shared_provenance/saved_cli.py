"""Shared independent engineering mechanics with explicit feature bindings.

Callers retain their source/codec policy. No production encoder or observed
replacement is used to manufacture an expectation.
"""

def main(
    *,
    GETTERS,
    NATIVE_WORK,
    Path,
    ROOT,
    __doc__,
    argparse,
    json,
    replay,
    subprocess,
):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--evidence-root', type=Path, default=ROOT)
    parser.add_argument('--verify-git', action='store_true')
    parser.add_argument('--producer-report', default=NATIVE_WORK + '/report.json')
    parser.add_argument('--getter-report', default=GETTERS + '/report.json')
    args = parser.parse_args()
    try:
        result = replay(args)
    except FileNotFoundError as error:
        print(json.dumps({'verified': False, 'available': False,
                          'reason': 'Required private evidence is absent', 'missing': str(error.filename)}, indent=2))
        return 2
    except (AssertionError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(json.dumps({'verified': False, 'available': True,
                          'reason': type(error).__name__ + ': ' + str(error)}, indent=2))
        return 1
    print(json.dumps(result, indent=2))
    return 0
