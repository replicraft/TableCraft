#!/bin/sh
set -e

# Refresh registrations without selecting TableCraft as the default handler.
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications
fi
if command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database /usr/share/mime
fi
