"""LA CABEZA DEL GATO, como la miden las estrellas quietas del escritorio de
mision (docs/arte/sol_quieto.py, docs/arte/nebulosa_quieta.py): el camino de
la cabeza de docs/arte/estrella_gato.js, aplanado para medir su largo
(`sobre`, como getPointAtLength) y preguntar si un punto cae dentro
(`dentro`, como isPointInFill); la semilla de alli, con el redondeo de los
dobles de JavaScript; y como se escriben los numeros en el SVG.
"""
import math

def semilla(s):
    st = [s]
    def r():
        v = float(st[0]) * 1103515245.0 + 12345.0
        st[0] = (int(v) % 2**32) & 0x7fffffff
        return st[0] / 0x7fffffff
    return r

# La cabeza, aplanada para medir su largo (como getPointAtLength).
def cub(a, b, c, d, n=200):
    out = []
    for k in range(1, n + 1):
        t = k / n; u = 1 - t
        out.append((u*u*u*a[0] + 3*u*u*t*b[0] + 3*u*t*t*c[0] + t*t*t*d[0], u*u*u*a[1] + 3*u*u*t*b[1] + 3*u*t*t*c[1] + t*t*t*d[1]))
    return out
def lin(a, b, n=200):
    return [(a[0] + (b[0]-a[0])*k/n, a[1] + (b[1]-a[1])*k/n) for k in range(1, n + 1)]
P = [(-104, 18)]
P += cub((-104, 18), (-104, -42), (-76, -80), (-50, -90))
P += lin((-50, -90), (-78, -158)); P += lin((-78, -158), (-16, -104))
P += cub((-16, -104), (-6, -106), (6, -106), (16, -104))
P += lin((16, -104), (78, -158)); P += lin((78, -158), (50, -90))
P += cub((50, -90), (76, -80), (104, -42), (104, 18))
P += cub((104, 18), (104, 82), (58, 118), (0, 118))
P += cub((0, 118), (-58, 118), (-104, 82), (-104, 18))
L = [0.0]
for i in range(1, len(P)):
    L.append(L[-1] + math.hypot(P[i][0]-P[i-1][0], P[i][1]-P[i-1][1]))
LARGO = L[-1]
def sobre(f):
    s = ((f % 1) + 1) % 1 * LARGO
    lo, hi = 0, len(L) - 1
    while lo < hi:
        m = (lo + hi) // 2
        if L[m] < s: lo = m + 1
        else: hi = m
    i = max(lo, 1); a, b = P[i-1], P[i]
    seg = L[i] - L[i-1] or 1
    t = (s - L[i-1]) / seg
    return (a[0] + (b[0]-a[0])*t, a[1] + (b[1]-a[1])*t)
def dentro(x, y):
    c = False
    for i in range(len(P)):
        a, b = P[i-1], P[i]
        if (a[1] > y) != (b[1] > y) and x < a[0] + (b[0]-a[0]) * (y-a[1]) / (b[1]-a[1]):
            c = not c
    return c
def dist_borde(x, y):
    return min(math.hypot(x-p[0], y-p[1]) for p in P[::4])

CABEZA = 'M -104 18 C -104 -42 -76 -80 -50 -90 L -78 -158 L -16 -104 C -6 -106 6 -106 16 -104 L 78 -158 L 50 -90 C 76 -80 104 -42 104 18 C 104 82 58 118 0 118 C -58 118 -104 82 -104 18 Z'
def f(v):
    t = ('%.1f' % v).rstrip('0').rstrip('.')
    return '0' if t in ('-0', '') else t
o = lambda v: ('%.2f' % v).lstrip('0') if v < 1 else '1'
