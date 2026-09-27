#!/usr/bin/env python3
"""Regenerates the installer artwork from the app icon.

  windows/installer-logo.bmp    NSIS welcome/finish logo (nsis.sidebarImage)
  windows/installer-header.bmp  NSIS page header, right aligned (nsis.headerImage)
  macos/dmg-background.png      DMG window background (macOS.dmg.background)

Usage: python3 gen-images.py   (run from any directory; needs Pillow)
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
ICON = ROOT / "icons" / "icon.png"

BG_LOWEST = (0x14, 0x18, 0x26)
BG_BASE = (0x1C, 0x20, 0x32)
BORDER = (0x30, 0x36, 0x4C)
MUTED = (0x8D, 0x91, 0xA5)
LABEL_PILL = (0x7A, 0x7F, 0x96)
EMERALD = (0x2B, 0xB8, 0x84)

# Must match bundle.macOS.dmg in tauri.macos.conf.json (Finder centres 128px icons on these points)
DMG_SIZE = (660, 400)
DMG_APP = (180, 170)
DMG_FOLDER = (480, 170)


def glow(size: tuple[int, int], bg: tuple[int, int, int], center: tuple[int, int], radius: int, alpha: int) -> Image.Image:
    img = Image.new("RGB", size, bg)
    layer = Image.new("RGBA", size, (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    cx, cy = center
    d.ellipse((cx - radius, cy - radius, cx + radius, cy + radius), fill=EMERALD + (alpha,))
    layer = layer.filter(ImageFilter.GaussianBlur(radius / 2))
    img.paste(layer, (0, 0), layer)
    return img


def logo(size: int) -> Image.Image:
    return Image.open(ICON).convert("RGBA").resize((size, size), Image.LANCZOS)


def font(size: int) -> ImageFont.ImageFont | ImageFont.FreeTypeFont:
    for name in ("Inter-Medium.ttf", "DejaVuSans.ttf", "LiberationSans-Regular.ttf", "Arial.ttf"):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def save_bmp(img: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    img.convert("RGB").save(path, format="BMP")


def nsis_images() -> None:
    # Full-window logo for welcome/finish pages (rendered stretched into a square control).
    canvas = glow((256, 256), BG_LOWEST, (128, 128), 120, 70)
    mark = logo(192)
    canvas.paste(mark, (32, 32), mark)
    save_bmp(canvas, ROOT / "windows" / "installer-logo.bmp")

    # Header (MUI header image, right aligned): 150x57 at 2x.
    header = glow((300, 114), BG_BASE, (240, 57), 90, 60)
    mark = logo(80)
    header.paste(mark, (300 - 80 - 24, 17), mark)
    save_bmp(header, ROOT / "windows" / "installer-header.bmp")


def dmg_background() -> None:
    w, h = DMG_SIZE
    img = glow(DMG_SIZE, BG_LOWEST, DMG_APP, 150, 45)
    d = ImageDraw.Draw(img)

    # Arrow between the app icon and the Applications folder (icons are 128px, leave a gap)
    y = DMG_APP[1]
    x0, x1 = DMG_APP[0] + 96, DMG_FOLDER[0] - 96
    d.line((x0, y, x1 - 14, y), fill=EMERALD, width=4)
    d.polygon([(x1, y), (x1 - 22, y - 12), (x1 - 22, y + 12)], fill=EMERALD)

    # Finder paints icon labels black in light mode and white in dark mode: give both a mid-tone pill
    for cx, _ in (DMG_APP, DMG_FOLDER):
        d.rounded_rectangle((cx - 66, y + 66, cx + 66, y + 92), radius=13, fill=LABEL_PILL)

    caption = "Drag Termoso to Applications to install"
    f = font(15)
    tw = d.textlength(caption, font=f)
    d.text(((w - tw) / 2, h - 58), caption, font=f, fill=MUTED)
    d.line((0, h - 1, w, h - 1), fill=BORDER, width=1)

    out = ROOT / "macos" / "dmg-background.png"
    out.parent.mkdir(parents=True, exist_ok=True)
    img.save(out, format="PNG", optimize=True)


def main() -> None:
    nsis_images()
    dmg_background()


if __name__ == "__main__":
    main()
