from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
FONT_ZH = str(ROOT / "src/assets/font/NaikaiFont-Regular.ttf")
FONT_MC = str(ROOT / "src/assets/font/Minecraft.otf")
ICON = Image.open(ROOT / "src/assets/icon.png").convert("RGBA")
GOLD = (246, 224, 164)


def backdrop(w, h, glow):
    im = Image.new("RGB", (w, h))
    px = im.load()
    for y in range(h):
        for x in range(w):
            t = x / w * 0.35 + y / h * 0.65
            px[x, y] = (int(32 - 21 * t), int(36 - 23 * t), int(43 - 27 * t))
    layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    gx, gy, gr = glow
    ImageDraw.Draw(layer).ellipse((gx - gr, gy - gr, gx + gr, gy + gr), fill=(*GOLD, 70))
    return Image.alpha_composite(im.convert("RGBA"), layer.filter(ImageFilter.GaussianBlur(gr * 0.55)))


def icon(size):
    return ICON.resize((size, size), Image.NEAREST if ICON.width <= size else Image.LANCZOS)


def sidebar():
    w, h = 328, 628
    im = backdrop(w, h, (w * 0.35, h * 0.22, 230))
    d = ImageDraw.Draw(im)
    im.alpha_composite(icon(120), ((w - 120) // 2, 120))

    def center(text, font, y, fill):
        x = (w - d.textlength(text, font=font)) / 2
        d.text((x + 2, y + 3), text, font=font, fill=(0, 0, 0, 160))
        d.text((x, y), text, font=font, fill=fill)

    center("輝煌", ImageFont.truetype(FONT_ZH, 76), 268, (255, 255, 255))
    center(" ".join("Brilliant"), ImageFont.truetype(FONT_MC, 26), 360, GOLD)
    d.line((w * 0.28, 412, w * 0.72, 412), fill=(*GOLD, 120), width=2)
    center("Minecraft 啟動器", ImageFont.truetype(FONT_ZH, 22), 430, (220, 222, 226))
    center("brilliantw.net", ImageFont.truetype(FONT_MC, 18), h - 60, (150, 154, 160))
    im.convert("RGB").save(HERE / "sidebar.bmp")


def header():
    w, h = 300, 114
    im = Image.new("RGBA", (w, h), (21, 24, 28, 255))
    layer = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    ImageDraw.Draw(layer).ellipse((150, -60, 360, 150), fill=(*GOLD, 55))
    im = Image.alpha_composite(im, layer.filter(ImageFilter.GaussianBlur(40)))
    d = ImageDraw.Draw(im)
    im.alpha_composite(icon(72), (w - 72 - 22, (h - 72) // 2))
    zh = ImageFont.truetype(FONT_ZH, 40)
    mc = ImageFont.truetype(FONT_MC, 16)
    right = w - 72 - 34
    d.text((right - d.textlength("輝煌", font=zh), 22), "輝煌", font=zh, fill=(255, 255, 255))
    d.text((right - d.textlength("Brilliant", font=mc), 72), "Brilliant", font=mc, fill=GOLD)
    im.convert("RGB").save(HERE / "header.bmp")


if __name__ == "__main__":
    sidebar()
    header()
    print("sidebar.bmp / header.bmp 已更新")
