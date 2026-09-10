# -*- coding: utf-8 -*-
"""QA screenshot analysis: OCR text + background color sampling."""
import sys, io, json, os
import cv2
import numpy as np

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")

from rapidocr_onnxruntime import RapidOCR

ocr = RapidOCR()

TEST_DIR = os.path.join("test", "20260907")
files = sorted(f for f in os.listdir(TEST_DIR) if f.endswith(".png"))

def hexof(bgr):
    b, g, r = int(bgr[0]), int(bgr[1]), int(bgr[2])
    return "#{:02x}{:02x}{:02x}".format(r, g, b)

for fn in files:
    path = os.path.join(TEST_DIR, fn)
    img = cv2.imread(path)
    if img is None:
        print("== %s : READ FAIL" % fn)
        continue
    h, w = img.shape[:2]
    print("=" * 70)
    print("== %s  %dx%d" % (fn, w, h))

    # --- dominant colors (quantized) ---
    small = cv2.resize(img, (min(w, 400), min(h, 400)), interpolation=cv2.INTER_AREA)
    q = (small // 8 * 8).reshape(-1, 3)
    colors, counts = np.unique(q, axis=0, return_counts=True)
    order = np.argsort(-counts)[:8]
    total = counts.sum()
    dom = ["%s %.0f%%" % (hexof(colors[i]), 100.0 * counts[i] / total) for i in order]
    print("dominant: " + " | ".join(dom))
    # corner + center samples
    pts = {"TL": (4, 4), "TR": (w - 5, 4), "BL": (4, h - 5), "BR": (w - 5, h - 5), "C": (w // 2, h // 2)}
    print("samples: " + " ".join("%s=%s" % (k, hexof(img[y, x])) for k, (x, y) in pts.items()))

    # --- OCR ---
    result, _ = ocr(img)
    if not result:
        print("(no text detected)")
        continue
    lines = []
    for box, text, score in result:
        xs = [p[0] for p in box]; ys = [p[1] for p in box]
        lines.append((min(ys), min(xs), text, float(score)))
    lines.sort(key=lambda t: (round(t[0] / 14), t[1]))
    for y, x, text, score in lines:
        print("  y=%4d x=%4d [%0.2f] %s" % (y, x, score, text))
