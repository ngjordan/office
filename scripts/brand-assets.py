#!/usr/bin/env python3
"""Regenerate every raster brand asset from the Workspace brand kit.

Usage: scripts/brand-assets.py KIT_DIR   (the extracted Workspace_Brand_Kit)
Needs ImageMagick `convert` (SVG -> PNG) and Pillow (.ico). The .icns uses packaging/macos/make-icns.py.
Outputs: assets/app-icon/*, apps/iridium-office/assets/tools/*.png (white glyphs, tinted at runtime).
"""
import pathlib, re, shutil, subprocess, sys
from PIL import Image

ROOT = pathlib.Path(__file__).resolve().parent.parent
kit = pathlib.Path(sys.argv[1])
icon = ROOT / "assets/app-icon"
APP = "iridium-office"  # file stem; the installed icon id stays ph.office


def render(svg, png, size):
    # Density = 72 * target / native width, so the SVG is rasterised at size (crisp), not upscaled.
    native = int(re.search(r'width="(\d+)"', pathlib.Path(svg).read_text()).group(1))
    subprocess.run(["convert", "-background", "none", "-density", str(max(1, round(72 * size / native))),
                    str(svg), "-resize", f"{size}x{size}", f"PNG32:{png}"], check=True)


# App icon: Rounded is the shipped artwork (png/ico/linux) (Square/Dark kept alongside for stores and dark docks).
svg = kit / "02_App_Icons/SVG"
for v in ("Square", "Rounded", "Dark"):
    shutil.copy(svg / f"Workspace_App_{v}.svg", icon / f"workspace-app-{v.lower()}.svg")
shutil.copy(svg / "Workspace_App_Rounded.svg", icon / f"{APP}.svg")
shutil.copy(svg / "Workspace_App_Rounded.svg", icon / f"{APP}-small.svg")
shutil.copy(svg / "Workspace_App_Rounded.svg", icon / "hicolor/scalable/apps/ph.office.svg")
sizes = (16, 24, 32, 48, 64, 128, 256, 512, 1024)
for s in sizes:
    render(svg / "Workspace_App_Rounded.svg", icon / f"_{s}.png", s)
for s in (128, 256, 512, 1024):
    shutil.copy(icon / f"_{s}.png", icon / f"{APP}-{s}.png")
for s in (128, 256):
    shutil.copy(icon / f"_{s}.png", icon / f"hicolor/{s}x{s}/apps/ph.office.png")
big = Image.open(icon / "_256.png").convert("RGBA")
big.save(icon / f"{APP}.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
# macOS applies its own mask, and the kit says not to pre-round for it: the .icns uses Square.
for s in (16, 32, 64, 128, 256, 512, 1024):
    render(svg / "Workspace_App_Square.svg", icon / f"_sq{s}.png", s)
subprocess.run([sys.executable, str(ROOT / "packaging/macos/make-icns.py"), str(icon / f"{APP}.icns")]
               + [f"{s}={icon / f'_sq{s}.png'}" for s in (16, 32, 64, 128, 256, 512, 1024)], check=True)
for f in icon.glob("_*.png"):
    f.unlink()

# Tool glyphs: the White variant, so the chrome can tint it with the tool's foreground colour.
out = ROOT / "apps/iridium-office/assets/tools"
for f in (kit / "03_Tool_Icons/SVG").glob("*_White.svg"):
    render(f, out / f.name.replace("_White.svg", ".png").lower(), 96)
print("ok")
