from pathlib import Path
import hashlib
import json
import math

root = Path(__file__).resolve().parent
root.mkdir(parents=True, exist_ok=True)

paper = '<path d="M7 3h12l6 6v20H7z" fill="{paper}"/><path d="M19 3v7h6" fill="none"/>'
grid = '<rect x="3" y="5" width="26" height="22" rx="2" fill="{paper}"/><path d="M3 11h26" stroke="{blue}" stroke-width="6"/><path d="M3 17h26M3 23h26M11 11v16M21 11v16" fill="none"/>'
funnel = '<path d="M3 5h26L19 17v9l-6 3V17z" fill="{pale}"/>'
check = '<path d="m16 23 4 4 9-11" fill="none" stroke="{green}" stroke-width="3"/>'
plus = '<path d="M24 17v12M18 23h12" stroke="{green}" stroke-width="3"/>'
lightning = '<path d="m19 2-10 16h7l-3 12 12-18h-8z" fill="{gold}"/>'

# Rounded, evenly spaced teeth and a clear central opening stay recognizable
# at the small Backstage rail size. This geometry is authored here, not imported.
gear_points = []
for tooth in range(8):
    for offset, radius in [(-0.45, 10), (-0.22, 10), (-0.22, 13.5),
                           (0.22, 13.5), (0.22, 10), (0.45, 10)]:
        angle = (tooth + offset) * math.tau / 8
        gear_points.append((16 + math.cos(angle) * radius,
                            16 + math.sin(angle) * radius))

gear_commands = []
for index, point in enumerate(gear_points):
    previous = gear_points[index - 1]
    following = gear_points[(index + 1) % len(gear_points)]
    before_length = math.dist(point, previous)
    after_length = math.dist(point, following)
    rounding = min(0.7, before_length * 0.35, after_length * 0.35)
    before = tuple(point[axis] + (previous[axis] - point[axis])
                   * rounding / before_length for axis in range(2))
    after = tuple(point[axis] + (following[axis] - point[axis])
                  * rounding / after_length for axis in range(2))
    prefix = 'M' if index == 0 else 'L'
    gear_commands.append(f'{prefix}{before[0]:.3f},{before[1]:.3f}')
    gear_commands.append(f'Q{point[0]:.3f},{point[1]:.3f} {after[0]:.3f},{after[1]:.3f}')
gear_commands.append('Z')
gear = '<path d="' + ' '.join(gear_commands) + '" fill="{ink}" stroke="none"/>'
gear += '<circle cx="16" cy="16" r="4.7" fill="{paper}" stroke="none"/>'

icons = {
    'run': '<path d="M12 3h8l-2 18h-4z" fill="{red}" stroke="none"/><circle cx="16" cy="27" r="3" fill="{red}" stroke="none"/>',
    'add_tables': grid + '<circle cx="24" cy="24" r="7" fill="{paper}" stroke="none"/>' + plus,
    'application': '<path d="M5 8v16c0 3 5 5 11 5s11-2 11-5V8" fill="{app_red}" stroke="{app_red}"/><ellipse cx="16" cy="8" rx="11" ry="5" fill="{app_red}" stroke="{paper}" stroke-width="1.5"/><path d="M5 16c0 3 5 5 11 5s11-2 11-5M5 24c0 3 5 5 11 5s11-2 11-5" fill="none" stroke="{paper}" stroke-width="1.5"/>',
    'folder_open': '<path d="M3 9V6h10l3 3h12v17H3z" fill="{gold}"/><path d="m3 15 5-4h23l-5 16H3z" fill="{palegold}"/>',
    'settings': gear,
    'info': '<circle cx="16" cy="16" r="13" fill="{blue}" stroke="none"/><circle cx="16" cy="9" r="1.5" fill="{paper}" stroke="none"/><path d="M13 14h3v10h4" fill="none" stroke="{paper}"/>',
    'arrow_left': '<path d="m14 5-11 11 11 11v-7h15v-8H14z" fill="{blue}"/>',
    'undo': '<path d="M13 4 3 12l10 7v-6h5c7 0 10 6 7 13 8-7 4-19-7-19h-5z" fill="{blue}"/>',
    'redo': '<path d="m19 4 10 8-10 7v-6h-5c-7 0-10 6-7 13C-1 19 3 7 14 7h5z" fill="{green}"/>',
    'attachment': '<path d="M12 23V10a6 6 0 0 1 12 0v13a9 9 0 0 1-18 0V11" fill="none" stroke="{blue}"/><path d="M12 10v13a3 3 0 0 0 6 0V10" fill="none"/>',
    'table': grid,
    'table_edit': grid + '<path d="m15 25 2-6L26 8l5 4-10 12z" fill="{gold}"/><path d="m17 19 4 5" fill="none"/>',
    'paste': '<rect x="6" y="5" width="21" height="24" rx="2" fill="{gold}"/><rect x="11" y="3" width="11" height="5" rx="2" fill="{paper}"/><path d="M11 13h11M11 18h11M11 23h7" fill="none"/>',
    'cut': '<circle cx="8" cy="25" r="5" fill="{pale}"/><circle cx="24" cy="25" r="5" fill="{pale}"/><path d="m11 21 13-18M21 21 8 3" fill="none" stroke="{blue}"/><circle cx="16" cy="15" r="2" fill="{gold}"/>',
    'copy': '<path d="M4 3h17v22H4z" fill="{pale}"/><path d="M11 9h17v21H11z" fill="{paper}"/><path d="M15 15h9M15 20h9M15 25h6" fill="none" stroke="{blue}"/>',
    'paint_brush': '<path d="m7 21 7 7c-7 4-11 1-11-3z" fill="{blue}"/><path d="m7 21 12-14 6 6-11 15z" fill="{gold}"/><path d="m19 7 4-4 6 6-4 4" fill="{paper}"/><path d="m8 21 6 6"/>',
    'ascending': '<path d="M5 5v22m-3-4 3 4 3-4" fill="none" stroke="{blue}"/><path d="m13 12 3-8 3 8m-5-3h4M13 20h8l-8 8h8" fill="none"/><path d="M25 5h4M25 10h4M25 19h4M25 24h4" stroke="{green}"/>',
    'descending': '<path d="M5 5v22m-3-4 3 4 3-4" fill="none" stroke="{blue}"/><path d="M13 4h8l-8 8h8m-8 16 3-8 3 8m-5-3h4" fill="none"/><path d="M25 5h4M25 10h4M25 19h4M25 24h4" stroke="{green}"/>',
    'filter': funnel,
    'selection': grid + '<rect x="12" y="12" width="8" height="10" fill="{palegold}" stroke="{gold}"/><path d="m23 24 3 3 5-7" fill="none" stroke="{green}"/>',
    'advanced_filter': funnel + '<circle cx="24" cy="23" r="7" fill="{blue}"/><path d="M20 21h8M20 25h8M23 18v6M26 24v4" stroke="{paper}"/>',
    'toggle_filter': funnel + '<circle cx="25" cy="24" r="6" fill="{green}"/><path d="m21 24 3 3 5-6" fill="none" stroke="{paper}"/>',
    'remove_sort': '<path d="M4 5h19M4 11h14M4 17h9" stroke="{blue}"/><path d="m17 19 12 10m0-10L17 29" stroke="{red}" stroke-width="3"/>',
    'refresh': paper + '<path d="M22 16a8 8 0 1 0 1 10" fill="none" stroke="{green}" stroke-width="3"/><path d="m17 16 7-1v7" fill="{green}" stroke="{green}"/>',
    'add': paper + '<path d="m25 14 2 5 5 1-4 4 1 5-4-3-5 3 1-5-4-4 6-1z" fill="{gold}"/>',
    'save': '<path d="M4 3h21l4 5v21H4z" fill="{blue}"/><path d="M9 3h13v9H9zM9 19h15v10H9z" fill="{paper}"/><path d="M18 4v6M13 23h7" stroke="{blue}"/>',
    'delete': grid + '<path d="m17 17 12 12m0-12L17 29" stroke="{red}" stroke-width="4"/>',
    'sum': '<path d="M25 5H8l10 11L8 27h17v-4H14l7-7-7-7h11z" fill="{blue}"/>',
    'spelling': '<path d="m3 17 4-12 4 12m-7-4h6M15 5v12m0-6h4a3 3 0 0 1 0 6h-4" fill="none"/>' + check,
    'more_records': grid + '<path d="M17 18h14v12H17z" fill="{palegold}"/><path d="M20 24h1m3 0h1m3 0h1" stroke="{ink}" stroke-width="2.5"/>',
    'search': '<circle cx="13" cy="13" r="9" fill="{pale}"/><path d="m20 20 9 9" stroke="{gold}" stroke-width="5"/><path d="m7 13 6-6" stroke="{paper}"/>',
    'replace': '<path d="M5 9h21m-4-4 5 4-5 4M27 23H6m4-4-5 4 5 4" fill="none" stroke="{blue}"/><rect x="3" y="3" width="8" height="9" fill="{palegold}"/><rect x="21" y="21" width="8" height="9" fill="{pale}"/>',
    'go_to': '<path d="M3 10h15V4l12 12-12 12v-6H3z" fill="{blue}"/><path d="M6 14h8M6 18h8" stroke="{paper}"/>',
    'cursor': '<path d="M7 3v24l6-7 6 10 5-3-6-10h10z" fill="{pale}"/>',
    'bold': '<path d="M9 4h9c11 0 11 12 4 12 10 0 9 12-2 12H9zm6 5v5h4c4 0 4-5 0-5zm0 10v5h5c4 0 4-5 0-5z" fill="{ink}" stroke="none"/>',
    'italic': '<path d="M15 4h12v4h-4l-8 16h4v4H7v-4h4l8-16h-4z" fill="{ink}" stroke="none"/>',
    'underline': '<path d="M8 4v13a8 8 0 0 0 16 0V4" fill="none" stroke-width="3"/><path d="M5 29h22" stroke="{blue}" stroke-width="3"/>',
    'text_color': '<path d="m7 22 9-18 9 18m-14-7h10" fill="none" stroke-width="3"/><path d="M4 28h24" stroke="{red}" stroke-width="5"/>',
    'fill': '<path d="m5 14 11-10 12 12-11 10z" fill="{palegold}"/><path d="m5 14 12 12 11-10" fill="{gold}"/><path d="m28 22-3 5a3 3 0 0 0 6 0z" fill="{blue}"/>',
    'zoom': '<circle cx="13" cy="13" r="9" fill="{pale}"/><path d="M8 13h10M13 8v10" stroke="{blue}"/><path d="m20 20 9 9" stroke="{gold}" stroke-width="5"/>',
    'key': '<circle cx="10" cy="11" r="8" fill="{gold}"/><circle cx="10" cy="10" r="2" fill="{paper}"/><path d="m16 17 12 12m-7-8 4-4m-1 7 4-4" stroke="{gold}" stroke-width="5"/>',
    'validation': '<path d="M3 5h21v17H3z" fill="{paper}"/><path d="M7 10h11M7 15h7" stroke="{blue}"/>' + check,
    'lookups': grid + '<circle cx="22" cy="21" r="7" fill="{palegold}"/><path d="m27 26 4 4" stroke="{gold}" stroke-width="4"/>',
    'macro_create': paper + lightning + '<path d="M26 21v10M21 26h10" stroke="{green}" stroke-width="3"/>',
    'macro_rename': paper + '<path d="M10 11h9M10 16h7" stroke="{blue}"/><path d="m15 25 2-5 10-10 4 4-10 10z" fill="{gold}"/>',
    'relationships': '<path d="M10 10h12v12H10" fill="none" stroke="{blue}"/><rect x="2" y="3" width="11" height="10" rx="1" fill="{paper}"/><rect x="19" y="18" width="11" height="11" rx="1" fill="{paper}"/><path d="M2 7h11M19 22h11" stroke="{blue}"/>',
    'properties': '<rect x="4" y="3" width="24" height="26" rx="2" fill="{paper}"/><path d="M4 10h24M13 10v19M7 15h3M7 20h3M7 25h3M17 15h7M17 20h7M17 25h7" stroke="{blue}"/>',
    'indexes': '<path d="M5 4h23v24H5z" fill="{paper}"/><path d="M11 8h12M11 14h12M11 20h12M11 25h12" stroke="{blue}"/><path d="M3 7h4M3 13h4M3 19h4M3 25h4" stroke="{gold}" stroke-width="3"/>',
    'dependencies': '<path d="M16 8v9M7 17h18M7 17v7M25 17v7" fill="none" stroke="{blue}"/><rect x="10" y="2" width="12" height="8" rx="1" fill="{palegold}"/><rect x="2" y="23" width="10" height="7" rx="1" fill="{paper}"/><rect x="20" y="23" width="10" height="7" rx="1" fill="{paper}"/>',
    'builder': '<path d="m6 6 19 19m-5-3 4-2 6 6-4 4z" fill="{pale}"/><path d="m18 4 1-3 2 3 4 1-4 2-2 4-1-4-4-2zM5 18l1-3 2 3 3 1-3 2-2 3-1-3-3-2z" fill="{gold}" stroke="none"/>',
    'insert_rows': grid + plus,
    'delete_rows': grid + '<path d="M16 23h15" stroke="{red}" stroke-width="4"/>',
}


# Native record navigation uses simple solid shapes that survive 12–16pt sizes.
icons.update({
    'first_record': '<path d="M7 5v22M26 5 11 16l15 11z" fill="{ink}" stroke="{ink}"/>',
    'previous_record': '<path d="M24 5 8 16l16 11z" fill="{ink}" stroke="none"/>',
    'next_record': '<path d="m8 5 16 11-16 11z" fill="{ink}" stroke="none"/>',
    'last_record': '<path d="M25 5v22M6 5l15 11L6 27z" fill="{ink}" stroke="{ink}"/>',
    'new_record': '<path d="m3 7 12 9-12 9z" fill="{ink}" stroke="none"/><path d="m24 4 2 7 7 2-6 4 1 7-6-4-6 4 2-7-5-4 7-2z" fill="{gold}" stroke="none"/>',
    'no_filter': funnel + '<path d="m6 27 21-22" stroke="{red}" stroke-width="3"/>',
    'view_datasheet': grid,
    'view_design': '<path d="M4 3h8v26H4zM17 29V4l12 25z" fill="{paper}"/><path d="M4 9h5M4 15h5M4 21h5M21 21h4" stroke="{blue}"/>',
    'quick_access': '<path d="M5 7h22M5 13h22M5 19h22" stroke="{ink}"/><path d="m11 25 5 5 5-5z" fill="{ink}" stroke="none"/>',
})

form = '<rect x="3" y="4" width="26" height="24" rx="2" fill="{paper}"/><path d="M3 10h26" stroke="{blue}" stroke-width="5"/><path d="M7 15h5M7 22h5M16 15h9M16 22h9" stroke="{ink}"/>'
report = paper + '<path d="M11 12h10M11 17h10M11 22h6" stroke="{blue}"/>'
pencil = '<path d="m18 27 1-6L27 9l4 4-9 12z" fill="{gold}" stroke="{ink}"/>'
wand = '<path d="m17 29 11-14M23 9v-6M20 6h6" stroke="{gold}" stroke-width="3"/>'
arrow_in = '<path d="M1 19h13m-4-4 4 4-4 4" stroke="{green}" stroke-width="3" fill="none"/>'
arrow_out = '<path d="M18 22h13m-4-4 4 4-4 4" stroke="{blue}" stroke-width="3" fill="none"/>'

icons.update({
    'query_design': grid + '<path d="m17 27 1-5 9-12 4 4-10 12z" fill="{gold}"/>',
    'query_wizard': grid + wand,
    'form': form,
    'form_design': form + pencil,
    'blank_form': '<rect x="3" y="4" width="26" height="24" rx="2" fill="{paper}"/><path d="M3 10h26" stroke="{blue}" stroke-width="5"/>',
    'form_wizard': form + wand,
    'report': report,
    'report_design': report + pencil,
    'blank_report': paper,
    'report_wizard': report + wand,
    'labels': '<rect x="3" y="4" width="26" height="24" rx="2" fill="{paper}"/><path d="M16 4v24M3 16h26" stroke="{blue}"/><path d="M7 10h5M20 10h5M7 22h5M20 22h5" stroke="{ink}"/>',
    'code': paper + '<path d="m14 13-4 5 4 5m6-10 4 5-4 5" fill="none" stroke="{blue}"/>',
    'class_module': paper + '<rect x="10" y="12" width="12" height="12" rx="2" fill="{pale}"/><path d="M10 17h12M15 17v7" stroke="{blue}"/>',
    'import': paper + arrow_in,
    'export': paper + arrow_out,
    'saved_imports': icons['save'] + arrow_in,
    'saved_exports': icons['save'] + arrow_out,
    'linked_tables': grid + '<path d="m13 20 4-4a5 5 0 0 1 7 7l-4 4m-2-6-4 4a5 5 0 0 1-7-7l4-4" fill="none" stroke="{green}" stroke-width="3"/>',
    'spreadsheet': grid + '<path d="m12 13 9 12m0-12-9 12" stroke="{green}" stroke-width="3"/>',
    'text_file': paper + '<path d="M11 13h11M11 18h11M11 23h11" stroke="{ink}"/>',
    'xml': paper + '<path d="m13 13-4 5 4 5m8-10 4 5-4 5m-6 0 3-10" fill="none" stroke="{green}"/>',
    'database_file': icons['application'] + arrow_out,
    'compact_repair': icons['application'] + '<path d="M25 3v10m-4-5 4 5 4-5" stroke="{gold}" stroke-width="3"/>',
    'documenter': report + '<circle cx="23" cy="23" r="7" fill="{palegold}"/><path d="M20 20h6M20 24h6" stroke="{ink}"/>',
    'performance': '<path d="M3 27V5M3 27h26" fill="none"/><path d="m6 22 6-8 6 3L27 5" fill="none" stroke="{green}" stroke-width="3"/>',
    'analyze_table': grid + '<circle cx="23" cy="22" r="7" fill="{palegold}"/><path d="m18 22 3 3 7-8" fill="none" stroke="{green}"/>',
    'help': '<circle cx="16" cy="16" r="13" fill="{blue}" stroke="none"/><path d="M11 11c0-7 13-7 11 0-1 4-6 3-6 8" fill="none" stroke="{paper}" stroke-width="3"/><circle cx="16" cy="25" r="1.6" fill="{paper}" stroke="none"/>',
    'feedback': '<path d="M3 4h26v19H14l-8 6v-6H3z" fill="{pale}"/><path d="M8 10h16M8 16h11" stroke="{blue}"/>',
    'before_change': icons['macro_create'] + '<path d="m9 6-6 5 6 5" fill="none" stroke="{blue}" stroke-width="3"/>',
    'before_delete': icons['macro_rename'] + '<path d="m9 6-6 5 6 5" fill="none" stroke="{red}" stroke-width="3"/>',
    'after_insert': icons['macro_create'] + '<path d="m23 3 6 5-6 5" fill="none" stroke="{green}" stroke-width="3"/>',
    'after_update': icons['macro_rename'] + '<path d="m23 3 6 5-6 5" fill="none" stroke="{blue}" stroke-width="3"/>',
    'after_delete': paper + '<path d="m12 14 10 10m0-10L12 24m11-21 6 5-6 5" fill="none" stroke="{red}" stroke-width="3"/>',
})

icons.update({
    'align_left': '<path d="M4 6h24M4 13h15M4 20h24M4 27h15" stroke="{ink}"/>',
    'align_center': '<path d="M4 6h24M9 13h14M4 20h24M9 27h14" stroke="{ink}"/>',
    'align_right': '<path d="M4 6h24M13 13h15M4 20h24M13 27h15" stroke="{ink}"/>',
    'borders': '<rect x="4" y="4" width="24" height="24" fill="{paper}"/><path d="M4 16h24M16 4v24" stroke="{blue}"/>',
    'alternate_rows': grid + '<path d="M4 17h24M4 24h24" stroke="{palegold}" stroke-width="5"/>',
    'decrease_indent': '<path d="M15 5h14M15 12h10M15 20h14M15 27h10m-15-16-7 5 7 5" fill="none" stroke="{ink}"/>',
    'increase_indent': '<path d="M15 5h14M15 12h10M15 20h14M15 27h10M3 11l7 5-7 5" fill="none" stroke="{ink}"/>',
    'bullets': '<path d="M11 6h18M11 16h18M11 26h18" stroke="{ink}"/><circle cx="4" cy="6" r="2" fill="{blue}"/><circle cx="4" cy="16" r="2" fill="{blue}"/><circle cx="4" cy="26" r="2" fill="{blue}"/>',
    'numbering': '<path d="M12 6h17M12 16h17M12 26h17M3 3h2v6M2 13h5l-5 6h5M2 23h5l-3 3h3v3H2" fill="none" stroke="{ink}"/>',
})

# A query-shaped panel with three independently positioned parameter controls.
# The geometry is authored here and deliberately distinct from the settings gear.
icons['parameters'] = (
    '<rect x="3" y="3" width="26" height="26" rx="2" fill="{paper}"/>'
    '<path d="M3 9h26" stroke="{blue}" stroke-width="5"/>'
    '<path d="M7 14h4M7 20h4M7 26h4" stroke="{ink}"/>'
    '<path d="M15 14h10M15 20h10M15 26h10" stroke="{blue}"/>'
    '<circle cx="18" cy="14" r="2.3" fill="{palegold}" stroke="{gold}"/>'
    '<circle cx="24" cy="20" r="2.3" fill="{palegold}" stroke="{gold}"/>'
    '<circle cx="20" cy="26" r="2.3" fill="{palegold}" stroke="{gold}"/>'
)

palettes = {
    'light': dict(ink='#445367', paper='#fbfdff', pale='#d8e9f8', blue='#4788bd', green='#3e9365', gold='#d8a039', palegold='#f5dfa0', red='#bf5962', app_red='#b12b3d'),
    'dark': dict(ink='#e6edf5', paper='#34485b', pale='#4f7799', blue='#88c9ff', green='#81e3a7', gold='#ffd16e', palegold='#92723a', red='#ff9caa', app_red='#dc6572'),
    'disabled-light': dict.fromkeys(['ink', 'blue', 'green', 'gold', 'red', 'app_red'], '#777f89') | dict(paper='#e7eaee', pale='#d5d9df', palegold='#d5d9df'),
    'disabled-dark': dict.fromkeys(['ink', 'blue', 'green', 'gold', 'red', 'app_red'], '#c2ccd7') | dict(paper='#303841', pale='#46505d', palegold='#46505d'),
}

records = []
for name, fragment in icons.items():
    variants = {}
    for variant, palette in palettes.items():
        filename = f'{name}-{variant}.svg'
        source = '\n'.join([
            '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">',
            f'<g stroke="{palette["ink"]}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">',
            fragment.format(**palette),
            '</g>',
            '</svg>',
            '',
        ])
        (root / filename).write_text(source)
        variants[variant] = dict(file=filename, sha256=hashlib.sha256(source.encode()).hexdigest())
    records.append(dict(name=name, variants=variants))

(root / 'PROVENANCE.json').write_text(json.dumps(dict(
    origin='Original TableCraft vector artwork authored for this repository; no imported glyph paths, fonts, logos or raster assets.',
    canvas=[32, 32],
    variants=list(palettes),
    icons=records,
), indent=2) + '\n')
(root / 'README.md').write_text('''# TableCraft command icons

Original colored vector illustrations authored for TableCraft. The command
metaphors and compact density are informed by the retained Access Home/Design
references; the geometry is original and does not copy Microsoft glyph paths,
product logos, or PaintCraft artwork. PaintCraft was consulted only for its
embedded theme-variant SVG rendering and display-density approach.

Each icon has a 32-unit canvas, a light/dark color variant, and corresponding
neutral disabled variants. Enabled rendering preserves the source colors;
disabled commands use neutral artwork and muted text. The Ribbon preserves full
artwork opacity for legibility; other controls retain their host disabled styling. Dark variants use brighter outlines and saturated semantic accents so
small command glyphs remain distinct from the dark Ribbon background. Record
navigation has dedicated solid silhouettes; Datasheet and Design view glyphs
have separate geometry from table editing. Files are self-contained vector shapes with no text/font dependency,
external references, filters, scripts, or linked images. Commands sharing a
concept may share geometric building blocks; distinct command meanings have
separate compositions.

Run `python3 assets/ui/icons/tablecraft/generate.py` from any working directory
to reproduce the variants and hash manifest. The geometry and semantic palette
are defined together in that owning generator.

These original repository assets use the repository license. The complete local
file/hash inventory is in PROVENANCE.json. The older Fluent subset and its MIT
notice remain retained source material but are not embedded by the active icon
module.
''')
print(f'Generated {len(records)} original icon concepts, {len(records) * len(palettes)} SVGs')
