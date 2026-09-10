# -*- coding: utf-8 -*-
"""Fine-grained pixel sampling for sticky window and capture bar."""
import sys, io, os
import cv2
import numpy as np

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")

def hexof(bgr):
    b, g, r = int(bgr[0]), int(bgr[1]), int(bgr[2])
    return "#{:02x}{:02x}{:02x}".format(r, g, b)

def row_profile(path, name, xs, step=8):
    img = cv2.imread(path, cv2.IMREAD_UNCHANGED)
    print("== %s shape=%s (h,w[,a])" % (name, img.shape))
    h = img.shape[0]; w = img.shape[1]
    has_alpha = img.shape[2] == 4 if img.ndim == 3 else False
    print("alpha:", has_alpha)
    if has_alpha:
        a = img[:, :, 3]
        print("alpha corners:", a[0,0], a[0,w-1], a[h-1,0], a[h-1,w-1], "center:", a[h//2, w//2])
        print("alpha opaque rows: first", int(np.argmax(a.max(axis=1) > 200)), "last", int(h - 1 - np.argmax(a[::-1].max(axis=1) > 200)))
    img3 = img[:, :, :3]
    for x in xs:
        col = []
        prev = None
        for y in range(0, h, step):
            c = hexof(img3[y, x])
            if c != prev:
                col.append("y%d=%s" % (y, c))
                prev = c
        print(" x=%d: %s" % (x, " ".join(col[:18])))

d = os.path.join("test", "20260907")
row_profile(os.path.join(d, "0c41f751-6dd9-4897-b4f5-155aa19bcf6c.png"), "sticky", [60, 190, 330], step=10)
print()
row_profile(os.path.join(d, "8f9d8e99-d3be-41e4-a0f8-576c670c7f82.png"), "capture", [60, 280, 500], step=6)
