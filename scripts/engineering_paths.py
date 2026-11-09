"""Current-layout paths for explicitly invoked compatibility engineering tools."""
from pathlib import Path
import sys


def repository_root(source=__file__):
    """Find this checkout without relying on the tool's directory depth."""
    for parent in Path(source).resolve().parents:
        if (parent / 'Cargo.toml').is_file() and (parent / 'crates').is_dir():
            return parent
    raise ValueError('engineering tool is outside the TableCraft checkout')


def configure_imports(source=__file__):
    """Expose current domain packages and their shared tool modules, in order."""
    root = repository_root(source)
    access = root / 'tests/access'
    domains = sorted(
        path for path in access.iterdir()
        if path.is_dir() and path.name != '__pycache__'
    )
    features = sorted(
        path for domain in domains for path in domain.iterdir()
        if path.is_dir() and path.name != '__pycache__'
    )
    sys.path[:0] = [
        str(Path(source).resolve().parent),
        str(root / 'scripts'),
        *map(str, domains),
        *map(str, features),
    ]
    return root


def reference_file(value):
    """Resolve an explicit current recipe path, or one unambiguous recipe name.

    Historical paths are deliberately unsupported. A bare name must identify
    exactly one maintained engineering file; ambiguity is an error.
    """
    root = repository_root()
    supplied = Path(value)
    if supplied.is_absolute() or '..' in supplied.parts:
        raise ValueError('engineering recipe must use a current repository path')
    if len(supplied.parts) > 1:
        result = (root / supplied).resolve()
        result.relative_to(root / 'tests/access')
        if not result.is_file():
            raise FileNotFoundError(result)
        return result
    matches = [
        path for path in (root / 'tests/access').glob('*/*/' + supplied.name)
        if path.is_file()
    ]
    if len(matches) != 1:
        raise ValueError(f'engineering recipe name has {len(matches)} matches: {supplied.name}')
    result = matches[0].resolve()
    result.relative_to(root / 'tests/access')
    return result
