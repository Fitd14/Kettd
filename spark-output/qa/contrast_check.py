def hsl(h, s, l):
    h, s, l = h / 360, s / 100, l / 100
    def f(n):
        k = (n + h * 12) % 12
        a = s * min(l, 1 - l)
        return l - a * max(-1, min(k - 3, 9 - k, 1))
    return (round(f(0) * 255), round(f(8) * 255), round(f(4) * 255))

def lum(rgb):
    def ch(c):
        c /= 255
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = map(ch, rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b

def ratio(fg, bg):
    l1, l2 = sorted([lum(fg), lum(bg)], reverse=True)
    return round((l1 + 0.05) / (l2 + 0.05), 2)

papers = {'warm': (44, 40, 96), 'kraft': (34, 45, 78), 'cyan': (190, 45, 88), 'ink': (40, 12, 16)}
strips = {'warm': (44, 32, 92), 'kraft': (33, 38, 72), 'cyan': (190, 32, 82), 'ink': (40, 10, 21)}
inks = {'warm': (30, 10, 15), 'kraft': (28, 30, 18), 'cyan': (200, 25, 18), 'ink': (40, 20, 88)}
mut = (30, 8, 40)
mut_ink = (40, 12, 62)

print('%-7s %8s %9s %10s' % ('paper', 'ink/paper', 'mut/paper', 'mut/strip'))
for k in papers:
    m = mut_ink if k == 'ink' else mut
    a = ratio(hsl(*inks[k]), hsl(*papers[k]))
    b = ratio(hsl(*m), hsl(*papers[k]))
    c = ratio(hsl(*m), hsl(*strips[k]))
    print('%-7s %8.2f %9.2f %10.2f' % (k, a, b, c))

print()
for cand in [(28, 20, 30), (28, 22, 26), (25, 20, 24)]:
    print('kraft muted cand %s: paper %.2f / strip %.2f' % (
        cand, ratio(hsl(*cand), hsl(*papers['kraft'])), ratio(hsl(*cand), hsl(*strips['kraft']))))
