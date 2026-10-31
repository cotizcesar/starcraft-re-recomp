#!/usr/bin/env python3
"""Genera tarjetas de estado PNG para reportar cada paso completado.

Uso:
  python3 tools/status_card.py <salida.png> "<titulo>" "<subtitulo>" "clave=valor" ... [--note texto]
"""
from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

W, H = 1280, 800
BG = (13, 17, 23)
PANEL = (22, 27, 34)
ACCENT = (255, 170, 0)
GREEN = (63, 185, 80)
DIM = (139, 148, 158)
FG = (230, 237, 243)

FONT_BOLD = "/usr/share/fonts/dejavu/DejaVuSans-Bold.ttf"
FONT_REG = "/usr/share/fonts/dejavu/DejaVuSans.ttf"
FONT_MONO = "/usr/share/fonts/dejavu/DejaVuSansMono.ttf"


def font(path: str, size: int) -> ImageFont.FreeTypeFont:
    try:
        return ImageFont.truetype(path, size)
    except OSError:
        return ImageFont.load_default()


def wrap(draw, text, fnt, max_w):
    words, lines, cur = text.split(), [], ""
    for w in words:
        probe = f"{cur} {w}".strip()
        if draw.textlength(probe, font=fnt) <= max_w:
            cur = probe
        else:
            lines.append(cur)
            cur = w
    if cur:
        lines.append(cur)
    return lines


def make(out: Path, title: str, subtitle: str, pairs: list[tuple[str, str]], note: str | None) -> None:
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    f_title = font(FONT_BOLD, 54)
    f_sub = font(FONT_REG, 28)
    f_key = font(FONT_MONO, 24)
    f_val = font(FONT_MONO, 24)
    f_small = font(FONT_REG, 22)

    d.rectangle([0, 0, W, 8], fill=ACCENT)
    d.text((60, 54), "STARCRAFT 1.23.10 · RECONSTRUCCIÓN", font=font(FONT_BOLD, 24), fill=ACCENT)
    d.text((60, 96), title, font=f_title, fill=FG)
    d.text((60, 164), subtitle, font=f_sub, fill=DIM)

    y = 232
    box_h = 44 + 46 * len(pairs)
    d.rounded_rectangle([60, y, W - 60, y + box_h], radius=14, fill=PANEL)
    yy = y + 26
    for k, v in pairs:
        d.text((88, yy), k, font=f_key, fill=DIM)
        d.text((430, yy), str(v), font=f_val, fill=GREEN)
        yy += 46

    yy = y + box_h + 36
    if note:
        for line in wrap(d, note, f_small, W - 140):
            d.text((60, yy), line, font=f_small, fill=DIM)
            yy += 32

    d.line([60, H - 70, W - 60, H - 70], fill=(48, 54, 61), width=2)
    d.text((60, H - 52), "Hermes Agent · REA · Ghidra 12.1.4 · Bazzite Linux", font=font(FONT_REG, 20), fill=DIM)
    img.save(out)
    print(out)


if __name__ == "__main__":
    args = sys.argv[1:]
    note = None
    if "--note" in args:
        i = args.index("--note")
        note = args[i + 1]
        args = args[:i] + args[i + 2 :]
    out = Path(args[0])
    title, subtitle = args[1], args[2]
    pairs = []
    for a in args[3:]:
        k, _, v = a.partition("=")
        pairs.append((k, v))
    make(out, title, subtitle, pairs, note)
