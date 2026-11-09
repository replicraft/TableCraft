"""Run one current compatibility tool with its feature-local Python imports."""
import argparse
from pathlib import Path
import runpy
import sys

REPOSITORY = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPOSITORY / 'scripts'))
from engineering_paths import configure_imports


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('tool', type=Path, help='current path under tests/access')
    parser.add_argument('arguments', nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    target = (REPOSITORY / arguments.tool).resolve()
    target.relative_to(REPOSITORY / 'tests/access')
    if target.suffix != '.py' or not target.is_file():
        parser.error('tool must be an existing Python file under tests/access')
    configure_imports(target)
    sys.argv = [str(target), *arguments.arguments]
    runpy.run_path(str(target), run_name='__main__')


if __name__ == '__main__':
    main()
