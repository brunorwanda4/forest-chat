from pathlib import Path

from PIL import Image, ImageDraw


SIZE = 1024
SCALE = SIZE / 512
HERE = Path(__file__).parent


def points(values):
    return [(round(x * SCALE), round(y * SCALE)) for x, y in values]


image = Image.new("RGBA", (SIZE, SIZE), "#047857")
draw = ImageDraw.Draw(image)

# Layered greens approximate the SVG gradient while keeping generation portable.
for inset in range(112):
    mix = inset / 111
    color = (
        round(34 + (4 - 34) * mix),
        round(197 + (120 - 197) * mix),
        round(94 + (87 - 94) * mix),
        255,
    )
    box = tuple(round(value * SCALE) for value in (inset, inset, 512 - inset, 512 - inset))
    draw.rounded_rectangle(box, radius=round((112 - inset) * SCALE), fill=color)

draw.rounded_rectangle(
    tuple(round(value * SCALE) for value in (112, 80, 400, 344)),
    radius=round(56 * SCALE),
    fill="#f0fdf4",
)
draw.polygon(points([(135, 300), (127, 402), (239, 304)]), fill="#f0fdf4")
draw.polygon(
    points(
        [
            (256, 119),
            (193, 213),
            (232, 213),
            (176, 304),
            (234, 304),
            (234, 328),
            (278, 328),
            (278, 304),
            (336, 304),
            (280, 213),
            (319, 213),
        ]
    ),
    fill="#15803d",
)

image = image.resize((512, 512), Image.Resampling.LANCZOS)
image.save(HERE / "icon.png")
image.save(HERE / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
