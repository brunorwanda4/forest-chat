"""Builds the app icons and the web logo from assets/logo.png.

Run from anywhere: python src-tauri/icons/generate.py
"""

from pathlib import Path

from PIL import Image


HERE = Path(__file__).parent
ROOT = HERE.parent.parent
SOURCE = ROOT / "assets" / "logo.png"

# Transparent breathing room around the artwork, as a fraction of the side.
MARGIN = 0.04

logo = Image.open(SOURCE).convert("RGBA")

# Trim the transparent border, then centre the artwork on a square canvas so
# the wreath fills the icon instead of floating in empty space.
logo = logo.crop(logo.getchannel("A").getbbox())
side = round(max(logo.size) * (1 + 2 * MARGIN))
square = Image.new("RGBA", (side, side), (0, 0, 0, 0))
square.paste(logo, ((side - logo.width) // 2, (side - logo.height) // 2), logo)

icon = square.resize((512, 512), Image.Resampling.LANCZOS)
icon.save(HERE / "icon.png", optimize=True)
square.save(
    HERE / "icon.ico",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)

# Served by the web UI at /logo.png (login header and favicon).
square.resize((192, 192), Image.Resampling.LANCZOS).save(ROOT / "static" / "logo.png", optimize=True)
