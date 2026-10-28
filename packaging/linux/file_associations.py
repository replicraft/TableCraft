#!/usr/bin/env python3
"""Explicit per-user desktop registration; never selects a default application."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

DESKTOP_ID = 'org.tablecraft.Inspect.desktop'
MIME_ID = 'application/vnd.ms-access'
RECEIPT = 'tablecraft/file-associations.json'
MIME_XML = b'''<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/vnd.ms-access">
    <comment>Access database</comment>
    <glob pattern="*.mdb"/>
    <glob pattern="*.accdb"/>
  </mime-type>
</mime-info>
'''


def digest(data):
    return hashlib.sha256(data).hexdigest()


def executable_argument(path):
    """Escape Exec quoting first, then the desktop string-value layer."""
    value = str(path)
    if '=' in value or any(ord(character) < 32 for character in value):
        raise ValueError('Executable path contains unsupported desktop characters')
    quoted = ''.join('\\' + character if character in '\\"`$' else character
                     for character in value)
    return '"' + quoted.replace('\\', '\\\\').replace('%', '%%') + '"'


def payloads(executable):
    desktop = (
        '[Desktop Entry]\nType=Application\nName=TableCraft Inspect\n'
        'Comment=Inspect Access databases without executing content\n'
        f'Exec={executable_argument(executable)} open --wait -- %f\n'
        'Terminal=true\nCategories=Development;Database;\n'
        f'MimeType={MIME_ID};application/msaccess;application/vnd.msaccess;application/x-msaccess;\n'
    ).encode('utf-8')
    return {f'applications/{DESKTOP_ID}': desktop,
            'mime/packages/org.tablecraft.Inspect.xml': MIME_XML}


def root_directory():
    value = os.environ.get('XDG_DATA_HOME')
    root = Path(value) if value else Path.home() / '.local/share'
    if not root.is_absolute():
        raise ValueError('XDG_DATA_HOME must be absolute')
    return root


def checked_path(root, relative):
    path = root / relative
    current = path
    while current != root.parent:
        if current.is_symlink():
            raise ValueError(f'Refusing symbolic link: {current}')
        current = current.parent
    return path


def refresh(root):
    subprocess.run(['update-mime-database', str(root / 'mime')], check=True)
    subprocess.run(['update-desktop-database', str(root / 'applications')], check=True)


def apply(action, executable, root):
    """Only owned files are changed; cache-refresh failures remain visible."""
    files = payloads(executable)
    receipt_path = checked_path(root, RECEIPT)
    expected = {'version': 1, 'executable': str(executable),
                'files': {name: digest(data) for name, data in files.items()}}
    receipt = None
    if receipt_path.exists():
        receipt = json.loads(receipt_path.read_text(encoding='utf-8'))
        if receipt != expected:
            raise ValueError('Registration belongs to another installation or was modified')
    for name, data in files.items():
        path = checked_path(root, name)
        if path.exists() and (receipt is None or path.read_bytes() != data):
            raise ValueError(f'Refusing unowned or modified file: {path}')
    if action == 'uninstall' and receipt is None:
        return
    for tool in ('update-mime-database', 'update-desktop-database'):
        if shutil.which(tool) is None:
            raise ValueError(f'Required desktop tool is unavailable: {tool}')
    if action == 'install':
        # Ownership precedes publication so interrupted installs remain recoverable.
        receipt_path.parent.mkdir(parents=True, exist_ok=True)
        if receipt is None:
            with receipt_path.open('x', encoding='utf-8') as stream:
                json.dump(expected, stream, indent=2)
                stream.write('\n')
        for name, data in files.items():
            path = checked_path(root, name)
            path.parent.mkdir(parents=True, exist_ok=True)
            if not path.exists():
                with path.open('xb') as stream:
                    stream.write(data)
    else:
        for name in files:
            checked_path(root, name).unlink(missing_ok=True)
    refresh(root)
    if action == 'uninstall':
        receipt_path.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=('install', 'uninstall'))
    parser.add_argument('--executable', type=Path,
                        default=Path(__file__).resolve().with_name('tablecraft'))
    args = parser.parse_args()
    executable = args.executable.absolute()
    if args.action == 'install' and (not executable.is_file() or not os.access(executable, os.X_OK)):
        parser.error('Executable must be an installed executable file')
    try:
        apply(args.action, executable, root_directory())
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f'File registration failed: {error}\n')
    print('TableCraft file-handler registration updated; defaults were not changed.')


if __name__ == '__main__':
    main()
