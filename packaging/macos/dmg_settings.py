"""TableCraft's first-party drag-to-Applications layout, written without Finder."""

VOLUME_NAME = "TableCraft"
BACKGROUND_SIZE = (720, 400)
WINDOW_RECT = ((160, 180), BACKGROUND_SIZE)
ICON_LOCATIONS = {"TableCraft.app": (180, 185), "Applications": (540, 185)}
ICON_SIZE = 112
TEXT_SIZE = 14


def settings(app, icon, background):
    return {
        "format": "UDZO",
        "filesystem": "HFS+",
        "files": [str(app)],
        "symlinks": {"Applications": "/Applications"},
        "icon": str(icon),
        "background": str(background),
        "window_rect": WINDOW_RECT,
        "default_view": "icon-view",
        "include_icon_view_settings": True,
        "include_list_view_settings": False,
        "show_status_bar": False,
        "show_tab_view": False,
        "show_toolbar": False,
        "show_pathbar": False,
        "show_sidebar": False,
        "icon_size": ICON_SIZE,
        "text_size": TEXT_SIZE,
        "icon_locations": ICON_LOCATIONS,
        # Hiding an application's extension changes its FinderInfo. Preserve the
        # exact checked application instead of modifying its extended attributes.
        "hide_extensions": [],
    }
