"""EL SOL DE PLASMA, QUIETO, como SVG (HM3 de docs/plan/PLAN_EL_HUD.md).

    python docs/arte/sol_quieto.py > toolchain/tools/maqueta/escritorio/sol.svg

Es UN fotograma (t = 0) del estilo SOL DE PLASMA de docs/arte/estrella_gato.js,
con su misma semilla (la cuenta de `semilla`, con el redondeo de los dobles de
JavaScript) y su misma cabeza. Lo que alli es un filtro aqui es un degradado o
un trazo ancho y tenue: el lector de SVG de MAQUETA 3 no lee filtros. Despues
de cambiar esto, se regenera la cara:

    cargo run -p bmo-maqueta -- toolchain/tools/maqueta/escritorio/sol.maqueta \
        Ultra_userspace/services/director/src/scene/sol_gen.rs
"""
import math, sys
T = 0.0

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
r = semilla(7)
out = []
w = out.append
rayos = []
for i in range(22):
    a = i / 22 * math.pi * 2 - T / 400 * math.pi / 180; l = 160 + r() * 80
    op = .08 + r() * .1
    rayos.append('<path d="M %s %s L %s %s L %s %s Z" fill-opacity="%s"/>' % (
        f(math.cos(a-.05)*100), f(math.sin(a-.05)*100), f(math.cos(a)*l), f(math.sin(a)*l), f(math.cos(a+.05)*100), f(math.sin(a+.05)*100), o(op)))
esp = []
for k in range(110):
    s0 = sobre(k / 110)
    if s0[1] < -92: continue
    s1 = sobre(k / 110 + .002)
    nx, ny = s1[1]-s0[1], s0[0]-s1[0]; nl = math.hypot(nx, ny) or 1
    fase = r() * 6.28
    ln = 7 + 12 * abs(math.sin(T / 500 + fase))
    # Hacia FUERA (en estrella_gato.js salen hacia dentro y la cabeza las tapa).
    x2, y2 = s0[0] + nx/nl*ln, s0[1] + ny/nl*ln
    esp.append((k % 4 != 0, '<line x1="%s" y1="%s" x2="%s" y2="%s"/>' % (f(s0[0]), f(s0[1]), f(x2), f(y2))))
granos = []
for j in range(130):
    x = (r() - .5) * 230; y = -150 + r() * 270; rr = 4 + r() * 9; fase = r() * 6.28
    cx = ((x + T / 90 + 115) % 230 + 230) % 230 - 115 + math.cos(T / 1400 + fase) * 4
    op = .06 + .14 * abs(math.sin(T / 800 + fase))
    # Sin clip (el de la cabeza no es convexo): solo los granulos que caben enteros.
    if dentro(cx, y) and dist_borde(cx, y) >= rr:
        granos.append('<circle cx="%s" cy="%s" r="%s" fill-opacity="%s"/>' % (f(cx), f(y), f(rr), o(op)))
bucles = []
for i, b in enumerate([[-78, -158, -100, -50], [78, -158, 100, -50], [-62, 98, 10, 118]]):
    h = 70 + 30 * math.sin(T / 1100 + i * 2)
    x1, y1, x2, y2 = b
    mx, my = (x1+x2)/2, (y1+y2)/2; nl = math.hypot(mx, my + 20) or 1
    cx, cy = mx + mx/nl*h, my + (my+20)/nl*h
    d = 'M %s %s Q %s %s %s %s' % (f(x1), f(y1), f(cx), f(cy), f(x2), f(y2))
    fr = ((T / 1800) + i * .3) % 1; u = 1 - fr
    gx, gy = u*u*x1 + 2*u*fr*cx + fr*fr*x2, u*u*y1 + 2*u*fr*cy + fr*fr*y2
    bucles.append((d, gx, gy))

w('<?xml version="1.0" encoding="UTF-8"?>')
w('''<!-- EL SOL DE PLASMA, QUIETO: la estrella del escritorio de mision (HM3 de
     docs/plan/PLAN_EL_HUD.md). Es UN fotograma (t = 0) del estilo SOL DE PLASMA
     de docs/arte/estrella_gato.js: los mismos rayos, espiculas, granulos,
     bucles y manchas, con la misma semilla. Lo que alli es un filtro (el
     brillo, la niebla) aqui es un degradado o un trazo ancho y tenue: el
     lector de SVG de MAQUETA 3 no lee filtros (seccion 2e de
     docs/componente/LA_MAQUETA_EXIGE.md). Viva, en la 3060, es HM3c.
     Lo escribe docs/arte/sol_quieto.py; se edita alli. -->''')
w('<svg xmlns="http://www.w3.org/2000/svg" viewBox="-240 -240 480 480" width="720" height="720">')
w('  <defs>')
w('    <style>')
w('      .rayos { fill: #FF2E88 }')
w('      .espiculas { stroke: #FFB36B; stroke-width: 1.5; stroke-opacity: .85; stroke-linecap: round }')
w('      .clara { stroke: #FFF3D6 }')
w('      .granos { fill: #FFF3D6 }')
w('      .bucle { fill: none; stroke: #FFD45E; stroke-width: 3.5; stroke-opacity: .85; stroke-linecap: round }')
w('      .bucle-luz { fill: none; stroke: #FFD45E; stroke-width: 10; stroke-opacity: .16; stroke-linecap: round }')
w('      .gota { fill: #FFF3D6 }')
w('      .estria { stroke: #2A0814; stroke-opacity: .5 }')
w('    </style>')
w('    <radialGradient id="niebla" cx="0" cy="-12" r="150" gradientUnits="userSpaceOnUse">')
w('      <stop offset=".45" stop-color="#FF2E88" stop-opacity=".45"/>')
w('      <stop offset=".72" stop-color="#FF2E88" stop-opacity=".16"/>')
w('      <stop offset="1" stop-color="#FF2E88" stop-opacity="0"/>')
w('    </radialGradient>')
w('    <radialGradient id="sol" cx="45%" cy="40%" r="65%">')
w('      <stop offset="0" stop-color="#FFF3D6"/>')
w('      <stop offset=".35" stop-color="#FFD45E"/>')
w('      <stop offset=".7" stop-color="#FF2E88"/>')
w('      <stop offset="1" stop-color="#5A1030"/>')
w('    </radialGradient>')
w('    <radialGradient id="limbo">')
w('      <stop offset=".6" stop-color="#5A1030" stop-opacity="0"/>')
w('      <stop offset="1" stop-color="#2A0814" stop-opacity=".75"/>')
w('    </radialGradient>')
w('    <path id="cabeza" d="%s"/>' % CABEZA)
w('  </defs>')
w('  <g class="rayos">')
for x in rayos: w('    ' + x)
w('  </g>')
w('  <circle cx="0" cy="-12" r="150" fill="url(#niebla)"/>')
w('  <g class="espiculas">')
for clara, x in esp: w('    ' + (x if clara else x.replace('<line ', '<line class="clara" ')))
w('  </g>')
w('  <use href="#cabeza" fill="url(#sol)"/>')
w('  <g class="granos">')
for x in granos: w('    ' + x)
w('  </g>')
w('  <path d="M -74 62 q 16 -12 30 4 t 28 -2 M 34 84 q 14 8 26 -4" fill="none" stroke="#5A1030" stroke-width="3" stroke-opacity=".5" stroke-linecap="round"/>')
w('  <use href="#cabeza" fill="url(#limbo)"/>')
for d, gx, gy in bucles:
    w('  <path class="bucle-luz" d="%s"/>' % d)
    w('  <path class="bucle" d="%s"/>' % d)
    w('  <circle class="gota" cx="%s" cy="%s" r="2.6"/>' % (f(gx), f(gy)))
for ox, oy in [(-40, 4), (40, 4)]:
    w('  <g transform="translate(%d %d)">' % (ox, oy))
    w('    <ellipse rx="28" ry="13" fill="#7A1838" fill-opacity=".7"/>')
    for a in range(24):
        c, s = math.cos(a / 24 * math.pi * 2), math.sin(a / 24 * math.pi * 2)
        w('    <line class="estria" x1="%s" y1="%s" x2="%s" y2="%s"/>' % (f(c*19), f(s*6), f(c*28), f(s*13)))
    w('    <rect x="-18" y="-4" width="36" height="8" rx="3" fill="#2A0814"/>')
    w('  </g>')
w('  <polygon points="-6,30 6,30 0,37" fill="#5A1030" fill-opacity=".8"/>')
w('</svg>')
sys.stdout.write('\n'.join(out) + '\n')
