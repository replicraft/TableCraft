"""Render original TableCraft installation artwork using the bundled Selawik faces."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SIZE = (720, 400)


def render(destination):
    try:
        from PIL import Image, ImageDraw, ImageFont
    except ImportError as error:
        raise ValueError("DMG artwork requires packaging/macos/requirements.txt") from error

    regular = ROOT / "assets/fonts/selawk.ttf"
    bold = ROOT / "assets/fonts/selawkb.ttf"
    canvas = Image.new("RGB", SIZE, "#f7f8fa")
    draw = ImageDraw.Draw(canvas)
    title = ImageFont.truetype(str(bold), 38)
    subtitle = ImageFont.truetype(str(regular), 17)
    instruction = ImageFont.truetype(str(bold), 24)
    detail = ImageFont.truetype(str(regular), 16)
    red = "#bd394d"

    # The cylinder is original TableCraft artwork, not another project's logo.
    draw.rectangle((44, 44, 75, 70), fill=red)
    draw.ellipse((44, 63, 75, 77), fill=red)
    draw.ellipse((44, 37, 75, 51), fill="#db6172")
    draw.arc((44, 50, 75, 64), 0, 180, fill="#f8d9df", width=2)
    draw.text((94, 27), "TableCraft", font=title, fill="#282c35")
    draw.text((96, 77), "Install for macOS", font=subtitle, fill="#646c7b")
    draw.line((40, 116, 680, 116), fill="#dde1e8", width=1)

    for box in ((94, 134, 266, 279), (454, 134, 626, 279)):
        draw.rounded_rectangle(box, radius=18, fill="#ffffff", outline="#e4e7ed", width=1)
    draw.line((302, 193, 417, 193), fill=red, width=4)
    draw.line((399, 175, 419, 193, 399, 211), fill=red, width=4, joint="curve")
    draw.text((360, 308), "Drag TableCraft to Applications", anchor="mt", font=instruction, fill="#303541")
    draw.text((360, 351), "Then open TableCraft from Applications.", anchor="mt", font=detail, fill="#646c7b")
    draw.rectangle((0, 395, 720, 400), fill=red)

    # PNG pixels contain rendered text, not embedded or modified font programs.
    with Path(destination).open("xb") as stream:
        canvas.save(stream, format="PNG", optimize=True)
