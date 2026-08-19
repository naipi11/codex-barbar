from __future__ import annotations

import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent
HIGH = (0, 255, 168, 255)
MEDIUM = (255, 170, 0, 255)
LOW = (255, 32, 96, 255)
INK = (16, 19, 26, 255)
SURFACE = (23, 28, 38, 252)
MUTED = (139, 149, 167, 255)
FG = (232, 237, 246, 255)
ACCENT = (48, 209, 88, 255)


def font(size: int, bold: bool = False):
    path = "C:/Windows/Fonts/segoeuib.ttf" if bold else "C:/Windows/Fonts/segoeui.ttf"
    if Path(path).exists():
        return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def wallpaper(size):
    img = Image.new("RGBA", size, INK)
    draw = ImageDraw.Draw(img)
    w, h = size
    draw.ellipse((-int(w * 0.18), -int(h * 0.35), int(w * 0.62), int(h * 0.55)), fill=(48, 209, 88, 34))
    draw.ellipse((int(w * 0.42), int(h * 0.18), int(w * 1.18), int(h * 1.15)), fill=(88, 40, 90, 36))
    return img.filter(ImageFilter.GaussianBlur(42))


def render_mark(size: int, color, glow: float = 0.55):
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    petals = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    cx = cy = size / 2
    ring = size * 0.31
    pw, ph = size * 0.20, size * 0.52
    for i in range(6):
        angle = math.radians(i * 60 - 8)
        dx = math.cos(angle) * ring
        dy = math.sin(angle) * ring
        box = [cx + dx - pw / 2, cy + dy - ph / 2, cx + dx + pw / 2, cy + dy + ph / 2]
        layer = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        ImageDraw.Draw(layer).rounded_rectangle(box, radius=int(pw), fill=color)
        petals = Image.alpha_composite(
            petals, layer.rotate(i * 60, resample=Image.BICUBIC, center=(cx, cy))
        )
    core = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    ImageDraw.Draw(core).ellipse(
        [cx - size * 0.12, cy - size * 0.12, cx + size * 0.12, cy + size * 0.12],
        fill=(8, 10, 14, 255),
    )
    glow_layer = petals.filter(ImageFilter.GaussianBlur(max(2, int(size * 0.09 * (0.25 + glow)))))
    tint = Image.new("RGBA", (size, size), (*color[:3], int(70 + glow * 140)))
    glow_layer = Image.composite(
        tint, Image.new("RGBA", (size, size), (0, 0, 0, 0)), glow_layer.split()[-1]
    )
    img = Image.alpha_composite(img, glow_layer)
    img = Image.alpha_composite(img, petals)
    img = Image.alpha_composite(img, core)
    return img


def tray_panel():
    panel = Image.new("RGBA", (460, 470), (0, 0, 0, 0))
    d = ImageDraw.Draw(panel)
    d.rounded_rectangle((0, 0, 459, 469), 24, fill=SURFACE, outline=(255, 255, 255, 30), width=1)
    mark = render_mark(44, (255, 255, 255, 255), 0.18)
    panel.alpha_composite(mark, (22, 20))
    d.text((78, 18), "naipi122899", font=font(22, True), fill=FG)
    d.text((78, 44), "codex-barbar", font=font(13), fill=MUTED)
    d.text((78, 62), "Signed in", font=font(13, True), fill=ACCENT)
    d.text((352, 24), "v1.0.15", font=font(13), fill=MUTED)
    d.rounded_rectangle((418, 20, 442, 44), 12, fill=(44, 48, 60, 255), outline=(255, 255, 255, 28), width=1)
    d.text((425, 22), "x", font=font(14, True), fill=FG)
    d.rounded_rectangle((18, 96, 442, 198), 18, fill=(20, 24, 34, 235), outline=(255, 255, 255, 22), width=1)
    d.text((34, 110), "Weekly quota", font=font(13), fill=MUTED)
    d.text((318, 108), "72% left", font=font(22, True), fill=FG)
    d.text((34, 136), "Resets Aug 20  ·  6d left", font=font(13), fill=MUTED)
    d.rounded_rectangle((34, 168, 426, 180), 6, fill=(255, 255, 255, 22))
    d.rounded_rectangle((34, 168, 316, 180), 6, fill=ACCENT)
    d.rounded_rectangle((18, 214, 442, 312), 18, fill=(20, 24, 34, 235), outline=(255, 255, 255, 22), width=1)
    d.text((34, 230), "Updated", font=font(14, True), fill=ACCENT)
    d.text((34, 254), "Last refresh: just now", font=font(13), fill=MUTED)
    d.text((34, 276), "Green / gold / red follow remaining quota", font=font(13), fill=MUTED)
    d.rounded_rectangle((18, 332, 104, 372), 18, fill=ACCENT)
    d.text((34, 342), "Refresh", font=font(13, True), fill=INK)
    x = 116
    for label in ("Usage", "Settings", "Close", "Quit"):
        d.rounded_rectangle((x, 332, x + 76, 372), 18, fill=(44, 48, 60, 255), outline=(255, 255, 255, 22), width=1)
        d.text((x + 14, 342), label, font=font(13), fill=FG)
        x += 82
    return panel


def taskbar_capsule():
    cap = Image.new("RGBA", (338, 44), (0, 0, 0, 0))
    d = ImageDraw.Draw(cap)
    d.rounded_rectangle((0, 0, 337, 43), 22, fill=(44, 44, 46, 188), outline=(255, 255, 255, 42), width=1)
    cap.alpha_composite(render_mark(28, (255, 255, 255, 255), 0.12), (8, 8))
    d.text((44, 10), "naipi1", font=font(16, True), fill=FG)
    d.text((116, 10), "Wk 72%", font=font(16, True), fill=ACCENT)
    d.text((198, 10), "8/20", font=font(16, True), fill=MUTED)
    return cap


def compose_hero():
    img = wallpaper((1440, 900))
    panel = tray_panel()
    shadow = Image.new("RGBA", img.size, (0, 0, 0, 0))
    shadow.alpha_composite(Image.new("RGBA", panel.size, (0, 0, 0, 110)), (70, 150))
    img = Image.alpha_composite(img, shadow.filter(ImageFilter.GaussianBlur(22)))
    img.alpha_composite(panel, (56, 128))
    img.alpha_composite(render_mark(210, HIGH, 0.82), (1128, 148))
    img.alpha_composite(Image.new("RGBA", (1440, 64), (28, 30, 36, 220)), (0, 836))
    img.alpha_composite(taskbar_capsule(), (1048, 846))
    d = ImageDraw.Draw(img)
    d.text((56, 36), "codex-barbar", font=font(40, True), fill=FG)
    d.text((360, 52), "Windows tray  ·  taskbar  ·  floating ball", font=font(20), fill=MUTED)
    return img.convert("RGB")


def compose_taskbar():
    img = wallpaper((1440, 240))
    img.alpha_composite(Image.new("RGBA", (1440, 64), (28, 30, 36, 230)), (0, 150))
    img.alpha_composite(taskbar_capsule(), (1040, 160))
    ImageDraw.Draw(img).text((48, 42), "Taskbar status stays resident beside the clock.", font=font(22, True), fill=FG)
    return img.convert("RGB")


def compose_tray():
    img = wallpaper((980, 720))
    img.alpha_composite(tray_panel(), (260, 110))
    return img.convert("RGB")


def compose_ball(color, caption):
    img = Image.new("RGBA", (520, 520), (12, 14, 18, 255))
    img.alpha_composite(render_mark(300, color, 0.8), (110, 70))
    ImageDraw.Draw(img).text((36, 454), caption, font=font(20, True), fill=FG)
    return img.convert("RGB")


def write_gif(path: Path, color, duration: int, frames: int = 30):
    images = []
    for i in range(frames):
        frame = Image.new("RGBA", (360, 360), (12, 14, 18, 255))
        mark = render_mark(240, color, 0.8).rotate(-i * (360 / frames), resample=Image.BICUBIC)
        frame.alpha_composite(mark, (60, 48))
        images.append(frame.convert("P", palette=Image.ADAPTIVE, colors=48))
    images[0].save(path, save_all=True, append_images=images[1:], duration=duration, loop=0, optimize=True, disposal=2)


def main():
    ROOT.mkdir(parents=True, exist_ok=True)
    compose_hero().save(ROOT / "hero.png", optimize=True)
    compose_tray().save(ROOT / "tray-panel.png", optimize=True)
    compose_taskbar().save(ROOT / "taskbar-status.png", optimize=True)
    compose_ball(HIGH, "High quota  ·  idle").save(ROOT / "float-ball-high.png", optimize=True)
    compose_ball(MEDIUM, "Medium quota  ·  thinking x2").save(ROOT / "float-ball-medium.png", optimize=True)
    compose_ball(LOW, "Low quota  ·  fast x3").save(ROOT / "float-ball-low.png", optimize=True)
    write_gif(ROOT / "float-ball-idle.gif", HIGH, 60)
    write_gif(ROOT / "float-ball-thinking.gif", MEDIUM, 30)
    write_gif(ROOT / "float-ball-fast.gif", LOW, 20)
    print("wrote", ROOT)


if __name__ == "__main__":
    main()
