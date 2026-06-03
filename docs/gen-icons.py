"""
OhMySerial 图标生成器
======================

设计：双向串口通信 — 上下两段弧形箭头（顺时针+逆时针），形成 refresh-like 图案。
配色：青蓝渐变背景 (#06b6d4 → #2563eb) + 白色箭头。

输出到 src-tauri/icons/（覆盖 Tauri 2.x 所需全部尺寸）。
"""
import os
import sys
try:
    sys.stdout.reconfigure(encoding="utf-8")
except Exception:
    pass

import math
from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ICON_DIR = os.path.join(ROOT, "src-tauri", "icons")
os.makedirs(ICON_DIR, exist_ok=True)

# 配色
BG_TOP = (0x06, 0xB6, 0xD4)   # 青 cyan-500
BG_BOT = (0x25, 0x63, 0xEB)   # 蓝 blue-600
WHITE = (0xFF, 0xFF, 0xFF, 255)

CANVAS = 1024
CORNER_R = 200  # 圆角
ARC_R = 320     # 弧的半径
ARC_W = 72      # 弧的线宽
ARROW_LEN = 100 # 箭头三角尺寸


def lerp_rgb(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))


def gradient_rounded_rect(size: int) -> Image.Image:
    """渐变填充 + 圆角方形（一次性画大尺寸再缩放）"""
    big = size * 2
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    for y in range(big):
        c = lerp_rgb(BG_TOP, BG_BOT, y / (big - 1)) + (255,)
        draw.line([(0, y), (big, y)], fill=c)
    # 圆角遮罩
    mask = Image.new("L", (big, big), 0)
    md = ImageDraw.Draw(mask)
    md.rounded_rectangle([(0, 0), (big, big)], radius=CORNER_R * 2, fill=255)
    img.putalpha(mask)
    return img.resize((size, size), Image.LANCZOS)


def draw_arc_arrow(draw: ImageDraw.ImageDraw, cx: float, cy: float,
                   radius: float, start_deg: float, end_deg: float,
                   line_w: int, arrow_size: int):
    """
    画一段圆弧 + 端点三角箭头。
    角度：0°=右, 90°=下, 180°=左, 270°=上 顺时针。
    """
    s = math.radians(start_deg)
    e = math.radians(end_deg)

    # 弧的两端
    x1 = cx + radius * math.cos(s)
    y1 = cy + radius * math.sin(s)
    x2 = cx + radius * math.cos(e)
    y2 = cy + radius * math.sin(e)

    # 弧（PIL.ImageDraw.arc 用 bbox）
    bbox = [cx - radius, cy - radius, cx + radius, cy + radius]
    draw.arc(bbox, start=start_deg, end=end_deg, fill=WHITE, width=line_w)

    # 箭头：在 (x2,y2) 指向 end_deg 切线方向（即 end_deg + 90°）
    tangent = math.radians(end_deg + 90)
    tip_x = x2 + arrow_size * math.cos(tangent)
    tip_y = y2 + arrow_size * math.sin(tangent)
    # 三角基底（垂直于切线两侧）
    perp = math.radians(end_deg)
    base_off = arrow_size * 0.7
    bl_x = x2 + base_off * math.cos(perp + math.radians(135))
    bl_y = y2 + base_off * math.sin(perp + math.radians(135))
    br_x = x2 + base_off * math.cos(perp - math.radians(135))
    br_y = y2 + base_off * math.sin(perp - math.radians(135))
    draw.polygon([(tip_x, tip_y), (bl_x, bl_y), (br_x, br_y)], fill=WHITE)


def draw_icon(size: int) -> Image.Image:
    img = gradient_rounded_rect(size)
    draw = ImageDraw.Draw(img)
    cx = cy = size / 2
    scale = size / CANVAS
    r = ARC_R * scale
    lw = max(2, int(ARC_W * scale))
    al = max(4, int(ARROW_LEN * scale))

    # 上弧：从左上 200° 扫到右上 340°（经过顶部），顺时针，左→右
    draw_arc_arrow(draw, cx, cy, r, 200, 340, lw, al)
    # 下弧：从右下 20° 扫到左下 160°（经过底部），顺时针，右→左
    draw_arc_arrow(draw, cx, cy, r, 20, 160, lw, al)

    return img


SIZES = [
    ("32x32.png", 32),
    ("128x128.png", 128),
    ("128x128@2x.png", 256),
    ("icon.png", 1024),
    ("Square30x30Logo.png", 30),
    ("Square44x44Logo.png", 44),
    ("Square71x71Logo.png", 71),
    ("Square89x89Logo.png", 89),
    ("Square107x107Logo.png", 107),
    ("Square142x142Logo.png", 142),
    ("Square150x150Logo.png", 150),
    ("Square284x284Logo.png", 284),
    ("Square310x310Logo.png", 310),
    ("StoreLogo.png", 50),
]

master = draw_icon(1024)
master.save(os.path.join(ICON_DIR, "icon.png"))
print(f"[OK] icon.png (1024x1024)")

for name, size in SIZES:
    if size == 1024:
        continue
    out = master.resize((size, size), Image.LANCZOS)
    out.save(os.path.join(ICON_DIR, name))
    print(f"[OK] {name} ({size}x{size})")

# ICO：Windows 多尺寸
ico_sizes = [16, 32, 48, 64, 128, 256]
ico_imgs = [master.resize((s, s), Image.LANCZOS) for s in ico_sizes]
ico_imgs[0].save(
    os.path.join(ICON_DIR, "icon.ico"),
    format="ICO",
    sizes=[(s, s) for s in ico_sizes],
    append_images=ico_imgs[1:],
)
print(f"[OK] icon.ico (sizes: {ico_sizes})")
print(f"\nAll icons generated to {ICON_DIR}")
