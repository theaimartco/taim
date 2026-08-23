"""Geometry for Aura's icon set, on a 24x24 grid in SVG coordinates (y down).

Each icon is a list of contours; each contour is a list of segments:
    ("L", x, y)                -- line to
    ("Q", cx, cy, x, y)        -- quadratic curve to
The first point of a contour is its last segment's endpoint, closed implicitly.

Outer contours are wound clockwise and holes counter-clockwise. TrueType fills
by non-zero winding, so only the *relative* direction matters -- which survives
the y-flip into font space.

Stroke-like forms are authored as filled outlines: a font glyph has no stroke.
"""

W = 2.0  # nominal stroke weight, in grid units


def rect(x, y, w, h):
    """A clockwise rectangle."""
    return [("L", x + w, y), ("L", x + w, y + h), ("L", x, y + h), ("L", x, y)]


def circle(cx, cy, r, ccw=False):
    """A circle as four quadratic arcs. `ccw` reverses it, making it a hole."""
    k = r * 0.5523 * 1.16  # quadratic control offset for a 4-segment circle
    pts = [
        (cx, cy - r), (cx + k, cy - r), (cx + r, cy - k), (cx + r, cy),
        (cx + r, cy + k), (cx + k, cy + r), (cx, cy + r),
        (cx - k, cy + r), (cx - r, cy + k), (cx - r, cy),
        (cx - r, cy - k), (cx - k, cy - r),
    ]
    if ccw:
        pts = [pts[0]] + pts[1:][::-1]
    out = []
    for i in range(4):
        c1 = pts[1 + i * 3]
        end = pts[(3 + i * 3) % 12]
        out.append(("Q", c1[0], c1[1], end[0], end[1]))
    return out


def ring(cx, cy, r_outer, r_inner):
    return [circle(cx, cy, r_outer), circle(cx, cy, r_inner, ccw=True)]


def poly(*points):
    return [("L", x, y) for x, y in points]


def bar(x1, y1, x2, y2, w=W):
    """A rectangle of width `w` from (x1,y1) to (x2,y2)."""
    import math
    dx, dy = x2 - x1, y2 - y1
    length = math.hypot(dx, dy) or 1.0
    nx, ny = -dy / length * w / 2, dx / length * w / 2
    return poly((x1 + nx, y1 + ny), (x2 + nx, y2 + ny),
                (x2 - nx, y2 - ny), (x1 - nx, y1 - ny))


def rotate(contours, deg, cx=12.0, cy=12.0):
    import math
    a = math.radians(deg)
    ca, sa = math.cos(a), math.sin(a)

    def rp(x, y):
        x, y = x - cx, y - cy
        return (x * ca - y * sa + cx, x * sa + y * ca + cy)

    out = []
    for contour in contours:
        new = []
        for seg in contour:
            if seg[0] == "L":
                new.append(("L", *rp(seg[1], seg[2])))
            else:
                new.append(("Q", *rp(seg[1], seg[2]), *rp(seg[3], seg[4])))
        out.append(new)
    return out


# ── The set ───────────────────────────────────────────────────────────

_PLUS = [poly((11, 4), (13, 4), (13, 11), (20, 11), (20, 13), (13, 13),
              (13, 20), (11, 20), (11, 13), (4, 13), (4, 11), (11, 11))]
_CHEVRON = [poly((12, 15.4), (19.4, 8.0), (18.0, 6.6), (12, 12.6),
                 (6.0, 6.6), (4.6, 8.0))]
_ARROW = [poly((11, 3), (13, 3), (13, 15), (17.5, 15), (12, 21), (6.5, 15),
               (11, 15))]

ICONS = {
    "add":            _PLUS,
    "remove":         [rect(4, 11, 16, 2)],
    "close":          rotate(_PLUS, 45),
    "check":          [poly((9.4, 18.6), (21.4, 6.6), (20.0, 5.2), (9.4, 15.8),
                            (4.0, 10.4), (2.6, 11.8))],
    "chevron_down":   _CHEVRON,
    "chevron_up":     rotate(_CHEVRON, 180),
    "chevron_left":   rotate(_CHEVRON, 90),
    "chevron_right":  rotate(_CHEVRON, -90),
    "arrow_down":     _ARROW,
    "arrow_up":       rotate(_ARROW, 180),
    "arrow_left":     rotate(_ARROW, 90),
    "arrow_right":    rotate(_ARROW, -90),
    "menu":           [rect(3, 5, 18, 2), rect(3, 11, 18, 2), rect(3, 17, 18, 2)],
    "more_vert":      [circle(12, 5, 2), circle(12, 12, 2), circle(12, 19, 2)],
    "more_horiz":     [circle(5, 12, 2), circle(12, 12, 2), circle(19, 12, 2)],
    "home":           [poly((12, 2.6), (22, 11.6), (19, 11.6), (19, 21),
                            (14, 21), (14, 15), (10, 15), (10, 21), (5, 21),
                            (5, 11.6), (2, 11.6))],
    "search":         ring(10, 10, 7.5, 5.5) + [bar(14.8, 14.8, 21, 21, 2.6)],
    "settings":       [rect(3, 5, 18, 2), rect(3, 11, 18, 2), rect(3, 17, 18, 2),
                       circle(8, 6, 3, ccw=True), circle(16, 12, 3, ccw=True),
                       circle(10, 18, 3, ccw=True)],
    "filter":         [poly((2.5, 4), (21.5, 4), (14, 12.5), (14, 20.5),
                            (10, 18.5), (10, 12.5))],
    "star":           [poly((12, 2), (14.9, 8.9), (22, 9.5), (16.6, 14.3),
                            (18.2, 21.4), (12, 17.6), (5.8, 21.4),
                            (7.4, 14.3), (2, 9.5), (9.1, 8.9))],
    "heart":          [[("Q", 12, 4.5, 14.2, 4.5), ("Q", 20, 4.5, 20, 10.4),
                        ("Q", 20, 14.6, 12, 21), ("Q", 4, 14.6, 4, 10.4),
                        ("Q", 4, 4.5, 9.8, 4.5)]],
    "person":         [circle(12, 7.5, 4.2),
                       poly((4.5, 21), (5.6, 16.4), (18.4, 16.4), (19.5, 21))],
    "calendar":       [rect(3, 5, 18, 16), rect(5, 9, 14, 10),
                       rect(7, 2.5, 2, 4), rect(15, 2.5, 2, 4)],
    "clock":          ring(12, 12, 9.5, 7.5) + [bar(12, 12, 12, 7),
                                                bar(12, 12, 16, 12)],
    "info":           ring(12, 12, 9.5, 7.5) + [rect(11, 10.5, 2, 7),
                                                circle(12, 7.5, 1.3)],
    "warning":        [poly((12, 2.5), (22.5, 21), (1.5, 21)),
                       rect(11, 9, 2, 6.5), circle(12, 18, 1.3)],
    "delete":         [rect(3.5, 5, 17, 2), rect(9, 2.5, 6, 2),
                       poly((5.5, 8), (18.5, 8), (17.2, 21.5), (6.8, 21.5))],
    "edit":           [poly((3, 17.5), (16.2, 4.3), (19.7, 7.8), (6.5, 21)),
                       poly((2.5, 21.5), (2.5, 18.2), (5.8, 21.5))],
    "download":       [poly((11, 3), (13, 3), (13, 12), (16.5, 12), (12, 17.5),
                            (7.5, 12), (11, 12)), rect(4, 19, 16, 2)],
    "upload":         [poly((11, 17.5), (13, 17.5), (13, 8.5), (16.5, 8.5),
                            (12, 3), (7.5, 8.5), (11, 8.5)), rect(4, 19, 16, 2)],
    "play":           [poly((7, 4), (20, 12), (7, 20))],
    "pause":          [rect(6.5, 4, 3.8, 16), rect(13.7, 4, 3.8, 16)],
    "mail":           [rect(2, 5, 20, 14), rect(4, 8.4, 16, 8.6),
                       poly((3.4, 5.6), (20.6, 5.6), (12, 13.2))],
    "folder":         [poly((2, 5), (9.5, 5), (11.5, 7.6), (22, 7.6), (22, 20),
                            (2, 20))],
    "visibility":     [[("Q", 12, 5, 12, 5), ("Q", 19, 5, 22.5, 12),
                        ("Q", 19, 19, 12, 19), ("Q", 5, 19, 1.5, 12),
                        ("Q", 5, 5, 12, 5)], circle(12, 12, 3.4, ccw=True)],
    "lock":           [rect(4, 10.5, 16, 11),
                       circle(12, 8, 4.8), circle(12, 8, 2.8, ccw=True),
                       rect(7.2, 8, 9.6, 3.5)],
}
