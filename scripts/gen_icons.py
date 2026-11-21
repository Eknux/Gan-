#!/usr/bin/env python3
"""Gan v2 — kaynak ikondan Tauri ikon setini üretir.

Tauri CLI'ın `tauri icon` komutu tercih edilir (tüm formatları üretir),
ancak burada PIL ile en azından PNG/ICO setini oluşturuyoruz, böylece
`cargo tauri android build` ikon eksikliği yüzünden takılmaz.

Kullanım:  python scripts/gen_icons.py
Çıktı:     src-tauri/icons/{32x32,128x128,128x128@2x,icon}.png + icon.ico
"""
from PIL import Image, ImageDraw, ImageFont
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICONS = os.path.join(ROOT, "src-tauri", "icons")
os.makedirs(ICONS, exist_ok=True)


def make_icon(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # Yuvarlatılmış köşeli arka plan: mor->bordo gradyan taklidi (düz mor).
    radius = size // 5
    d.rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=(103, 80, 164, 255))
    # "G" harfi.
    try:
        font = ImageFont.truetype(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", int(size * 0.62)
        )
    except Exception:
        font = ImageFont.load_default()
    text = "G"
    bbox = d.textbbox((0, 0), text, font=font)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    d.text(((size - tw) / 2 - bbox[0], (size - th) / 2 - bbox[1] - size * 0.02),
           text, font=font, fill=(255, 255, 255, 255))
    return img


def main() -> None:
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
        "Square30Logo.png": 30,
        "Square44Logo.png": 44,
        "Square71Logo.png": 71,
        "Square89Logo.png": 89,
        "Square107Logo.png": 107,
        "Square142Logo.png": 142,
        "Square150Logo.png": 150,
        "Square284Logo.png": 284,
        "Square310Logo.png": 310,
        "StoreLogo.png": 50,
    }
    for name, sz in sizes.items():
        make_icon(sz).save(os.path.join(ICONS, name))
        print(f"  {name} ({sz}x{sz})")
    # ICO (Windows).
    ico = make_icon(256)
    ico.save(os.path.join(ICONS, "icon.ico"), sizes=[(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    print("  icon.ico")
    # Kaynak 1024 (kullanıcı `tauri icon` için).
    make_icon(1024).save(os.path.join(ICONS, "source-1024.png"))
    print("  source-1024.png (kaynak; cargo tauri icon için)")
    print(f"Tamam: {ICONS}")


if __name__ == "__main__":
    main()
