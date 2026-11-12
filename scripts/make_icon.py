"""Генерирует assets/icon.ico и assets/icon.png (кнопка питания на синем круге)."""
from PIL import Image, ImageDraw
import math, pathlib

S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.ellipse([16, 16, S - 16, S - 16], fill=(0x3F, 0x5F, 0xA9, 255))
d.ellipse([90, 90, S - 90, S - 90], fill=(0x4A, 0x6C, 0xBA, 255))
c, r, w = S / 2, 250, 78
# дуга
d.arc([c - r, c - r + 40, c + r, c + r + 40], start=-58, end=238, fill="white", width=w)
# палочка
d.rounded_rectangle([c - w / 2, c - 330, c + w / 2, c + 20], radius=w / 2, fill="white")
# правка концов дуги (скругление)
for ang in (-58, 238):
    a = math.radians(ang)
    x, y = c + (r - w / 2) * math.cos(a), c + 40 + (r - w / 2) * math.sin(a)
    d.ellipse([x - w / 2, y - w / 2, x + w / 2, y + w / 2], fill="white")
out = pathlib.Path(__file__).resolve().parent.parent / "assets"
img.resize((256, 256), Image.LANCZOS).save(out / "icon.png")
img.save(out / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
