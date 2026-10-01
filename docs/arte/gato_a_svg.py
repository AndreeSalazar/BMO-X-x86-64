#!/usr/bin/env python3
"""El gato de BMO-X en SVG HD y ANIMADO, calcado de `bmo-x-gato.jpg` (01-10).

=== Por que calcado y no dibujado ===

El propietario: *"el original de referencia y las animaciones"*. Un gato
dibujado a mano se parece; uno calcado ES el del logo. Se separa el trazo
blanco y los ojos cian de la imagen, se calcan sus bordes a 3x (lados de pixel
enlazados en lazos, Douglas-Peucker y Catmull-Rom a Bezier) y se rellenan con
`evenodd`: el mismo dibujo, nitido a cualquier medida.

Encima, las animaciones: los ojos laten, parpadean y dan glitch en rosa y
azul, dos franjas del gato saltan, la cola se mece, una linea escanea, el
triangulo late y el kanji tira. Con `prefers-reduced-motion` se queda quieto.

=== Como se ejecuta ===

    pip install pillow
    python docs/arte/gato_a_svg.py

Escribe `docs/arte/bmo-x-gato-hd.svg`. La salida esta commiteada: el build no
depende de esto, existe para poder REHACERLO si el logo cambia.
"""

import sys
from pathlib import Path

try:
    from PIL import Image, ImageFilter
except ImportError:
    sys.exit("hace falta Pillow:  pip install pillow")

AQUI = Path(__file__).resolve().parent
SRC = AQUI / 'bmo-x-gato.jpg'
X0, Y0, X1, Y1 = 315, 255, 725, 735   # la caja del gato en la imagen
K = 3                                  # se calca a 3x para que la curva sea fina

im = Image.open(SRC).convert('RGB').crop((X0, Y0, X1, Y1))
im = im.resize(((X1 - X0) * K, (Y1 - Y0) * K), Image.BICUBIC).filter(ImageFilter.GaussianBlur(1.2))
W, H = im.size
px = im.load()

def mascara(prueba):
    m = bytearray(W * H)
    for y in range(H):
        for x in range(W):
            if prueba(*px[x, y]):
                m[y * W + x] = 1
    return m

blanco = mascara(lambda r, g, b: r > 120 and g > 120 and b > 120 and abs(r - b) < 70 and r > g - 40)
cian = mascara(lambda r, g, b: g > 110 and b > 110 and r < g - 50)

def bordes(m):
    """Los lados de pixel entre dentro y fuera, en sentido horario, enlazados en lazos."""
    sig = {}
    def dentro(x, y):
        return 0 <= x < W and 0 <= y < H and m[y * W + x]
    for y in range(H):
        fila = y * W
        for x in range(W):
            if not m[fila + x]:
                continue
            if not dentro(x, y - 1): sig.setdefault((x, y), []).append((x + 1, y))
            if not dentro(x + 1, y): sig.setdefault((x + 1, y), []).append((x + 1, y + 1))
            if not dentro(x, y + 1): sig.setdefault((x + 1, y + 1), []).append((x, y + 1))
            if not dentro(x - 1, y): sig.setdefault((x, y + 1), []).append((x, y))
    lazos = []
    while sig:
        ini = next(iter(sig))
        lazo = [ini]
        p = ini
        while True:
            l = sig.get(p)
            if not l:
                break
            q = l.pop()
            if not l:
                del sig[p]
            if q == ini:
                break
            lazo.append(q)
            p = q
        if len(lazo) >= 8:
            lazos.append(lazo)
    return lazos

def dp(pts, eps):
    """Douglas-Peucker sobre un lazo cerrado."""
    if len(pts) < 4:
        return pts
    def rec(a, b, out):
        (x1, y1), (x2, y2) = pts[a], pts[b]
        dx, dy = x2 - x1, y2 - y1
        n = (dx * dx + dy * dy) ** 0.5 or 1e-9
        mx, mi = 0, -1
        for i in range(a + 1, b):
            x, y = pts[i]
            d = abs(dy * x - dx * y + x2 * y1 - y2 * x1) / n
            if d > mx:
                mx, mi = d, i
        if mx > eps and mi > 0:
            rec(a, mi, out); rec(mi, b, out)
        else:
            out.append(pts[b])
    mid = len(pts) // 2
    out = [pts[0]]
    rec(0, mid, out)
    pts2 = pts + [pts[0]]
    out2 = []
    def rec2(a, b):
        (x1, y1), (x2, y2) = pts2[a], pts2[b]
        dx, dy = x2 - x1, y2 - y1
        n = (dx * dx + dy * dy) ** 0.5 or 1e-9
        mx, mi = 0, -1
        for i in range(a + 1, b):
            x, y = pts2[i]
            d = abs(dy * x - dx * y + x2 * y1 - y2 * x1) / n
            if d > mx:
                mx, mi = d, i
        if mx > eps and mi > 0:
            rec2(a, mi); rec2(mi, b)
        else:
            out2.append(pts2[b])
    rec2(mid, len(pts))
    return out + out2[:-1]

def curva(pts):
    """Catmull-Rom cerrado a Bezier cubicas, en coordenadas de la imagen."""
    n = len(pts)
    f = lambda p: ((p[0] / K) + X0, (p[1] / K) + Y0)
    P = [f(p) for p in pts]
    d = 'M%.1f %.1f' % P[0]
    for i in range(n):
        p0, p1, p2, p3 = P[i - 1], P[i], P[(i + 1) % n], P[(i + 2) % n]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        d += 'C%.1f %.1f %.1f %.1f %.1f %.1f' % (c1[0], c1[1], c2[0], c2[1], p2[0], p2[1])
    return d + 'Z'

def calcar(m, minimo):
    out = []
    for lazo in bordes(m):
        if len(lazo) < minimo:
            continue
        s = dp(lazo, 1.9)
        if len(s) >= 3:
            xs = [p[0] / K + X0 for p in lazo]; ys = [p[1] / K + Y0 for p in lazo]
            out.append({'d': curva(s), 'caja': [min(xs), min(ys), max(xs), max(ys)]})
    return out

CALCO = {'trazo': calcar(blanco, 30), 'ojos': calcar(cian, 30)}

CSS = """
.bx-trazo{stroke:#F2F7F9;stroke-width:4.5;stroke-linecap:round;stroke-linejoin:round}
.bx-fino{stroke:#F2F7F9;stroke-width:2.2;stroke-linecap:round;fill:none}
.bx-ojos{animation:bx-brillo 2.6s ease-in-out infinite}
.bx-parpado{transform-box:fill-box;transform-origin:center;animation:bx-parpadeo 6s infinite}
.bx-g1{animation:bx-glitch1 3.7s steps(1) infinite}
.bx-g2{animation:bx-glitch2 3.7s steps(1) infinite}
.bx-corte{animation:bx-corte 5.3s steps(1) infinite}
.bx-cola{transform-box:fill-box;transform-origin:100% 20%;animation:bx-cola 4s ease-in-out infinite}
.bx-escaneo{animation:bx-escaneo 3.2s linear infinite}
.bx-aviso{animation:bx-aviso 1.6s ease-in-out infinite}
.bx-kanji{animation:bx-kanji 7s steps(1) infinite}
@keyframes bx-brillo{0%,100%{opacity:1}50%{opacity:.78}}
@keyframes bx-parpadeo{0%,44%,48%,100%{transform:scaleY(1)}46%{transform:scaleY(.12)}}
@keyframes bx-glitch1{0%,82%,100%{transform:translate(0,0);opacity:0}84%{transform:translate(-9px,1px);opacity:.95}87%{transform:translate(6px,-2px);opacity:.95}90%{transform:translate(-3px,0);opacity:.7}93%{transform:translate(12px,0);opacity:.5}}
@keyframes bx-glitch2{0%,82%,100%{transform:translate(0,0);opacity:0}85%{transform:translate(9px,-1px);opacity:.95}88%{transform:translate(-6px,2px);opacity:.95}91%{transform:translate(4px,0);opacity:.7}94%{transform:translate(-12px,0);opacity:.5}}
@keyframes bx-corte{0%,88%,100%{transform:translateX(0);opacity:0}89%{transform:translateX(-14px);opacity:1}91%{transform:translateX(10px);opacity:1}93%{transform:translateX(-5px);opacity:1}95%{opacity:0}}
@keyframes bx-cola{0%,100%{transform:rotate(0deg)}50%{transform:rotate(-6deg)}}
@keyframes bx-escaneo{0%{transform:translateY(0);opacity:0}8%{opacity:.55}92%{opacity:.55}100%{transform:translateY(470px);opacity:0}}
@keyframes bx-aviso{0%,100%{opacity:1}50%{opacity:.35}}
@keyframes bx-kanji{0%,90%,100%{transform:translate(0,0)}92%{transform:translate(4px,0)}94%{transform:translate(-3px,0)}}
@media (prefers-reduced-motion: reduce){.bx-ojos,.bx-parpado,.bx-g1,.bx-g2,.bx-corte,.bx-cola,.bx-escaneo,.bx-aviso,.bx-kanji{animation:none}}
"""

# La silueta: cabeza y cuerpo en un solo trazo, oreja izquierda en punta y la
# derecha doblada en la punta, como el logo original.
def _es_cola(c):
    x0, y0, x1, y1 = c['caja']
    return y0 >= 605 and x1 <= 545 and x0 < 530
TRAZO = ''.join(f'<path d="{c["d"]}"/>' for c in CALCO['trazo'] if not _es_cola(c))
COLA_T = ''.join(f'<path d="{c["d"]}"/>' for c in CALCO['trazo'] if _es_cola(c))
OJOS_T = ''.join(f'<path d="{c["d"]}"/>' for c in CALCO['ojos'])

def ojos(color, clase):
    return f'<g class="{clase}"><g class="bx-parpado" fill="{color}">{OJOS_T}</g></g>'

def gato(id_):
    # En las coordenadas de bmo-x-gato.jpg: el gato ocupa (327,273)-(710,729).
    return f"""<defs>
<filter id="{id_}-neon" x="-50%" y="-200%" width="200%" height="500%">
<feGaussianBlur stdDeviation="5" result="b"/>
<feMerge><feMergeNode in="b"/><feMergeNode in="b"/><feMergeNode in="SourceGraphic"/></feMerge>
</filter>
<filter id="{id_}-halo" x="-20%" y="-20%" width="140%" height="140%">
<feGaussianBlur stdDeviation="4"/>
</filter>
<clipPath id="{id_}-franja"><rect x="300" y="470" width="440" height="24"/><rect x="300" y="640" width="440" height="14"/></clipPath>
<clipPath id="{id_}-caja"><rect x="310" y="262" width="420" height="474"/></clipPath>
<linearGradient id="{id_}-luz" x1="0" y1="0" x2="0" y2="1">
<stop offset="0" stop-color="#5EF2E6" stop-opacity="0"/>
<stop offset=".5" stop-color="#5EF2E6" stop-opacity=".9"/>
<stop offset="1" stop-color="#5EF2E6" stop-opacity="0"/>
</linearGradient>
</defs>
<ellipse cx="520" cy="728" rx="215" ry="12" fill="#5EF2E6" opacity=".10"/>
<g opacity=".45" filter="url(#{id_}-halo)" fill="#5EF2E6" fill-rule="evenodd">{TRAZO}{COLA_T}</g>
<g class="bx-cola" fill="#F2F7F9" fill-rule="evenodd">{COLA_T}</g>
<g fill="#F2F7F9" fill-rule="evenodd">{TRAZO}</g>
<g filter="url(#{id_}-neon)" class="bx-ojos">{ojos('#5EF2E6','')}</g>
<g style="mix-blend-mode:screen">{ojos('#FF2E88','bx-g1')}{ojos('#3DA5FF','bx-g2')}</g>
<g class="bx-corte" clip-path="url(#{id_}-franja)" fill="#FF2E88" fill-rule="evenodd">{TRAZO}{COLA_T}</g>
<g clip-path="url(#{id_}-caja)"><rect class="bx-escaneo" x="310" y="252" width="420" height="10" fill="url(#{id_}-luz)"/></g>"""

def logo(id_):
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 900" width="1000" height="900" role="img" aria-label="BMO-X: el gato con los ojos de neon">
<g transform="translate(-200 -200) scale(1.07)">{gato(id_)}</g>
<g class="bx-kanji" font-family="'Noto Sans JP', 'Yu Gothic', sans-serif" font-size="170" fill="#5EF2E6">
<text x="610" y="575" filter="url(#{id_}-neon)" opacity=".55">\u732b</text>
<text x="610" y="575">\u732b</text>
</g>
<g font-family="Michroma, Orbitron, 'Segoe UI', sans-serif">
<text x="480" y="745" text-anchor="middle" font-size="104" fill="#F2F7F9" letter-spacing="10">BMO-X</text>
<g transform="translate(788 690)" class="bx-aviso">
<path d="M0 -26 L26 18 L-26 18 Z" fill="none" stroke="#5EF2E6" stroke-width="4" stroke-linejoin="round"/>
<path d="M0 -10 V5 M0 11 V11.5" stroke="#5EF2E6" stroke-width="4" stroke-linecap="round"/>
</g>
<line x1="200" y1="812" x2="300" y2="812" stroke="#5EF2E6" stroke-width="2" opacity=".6"/>
<line x1="700" y1="812" x2="800" y2="812" stroke="#5EF2E6" stroke-width="2" opacity=".6"/>
<text x="500" y="820" text-anchor="middle" font-size="24" fill="#5EF2E6" letter-spacing="9">BMO METAKERNEL</text>
</g>
</svg>"""


svg = logo('bx').replace('viewBox="0 0 1000 900" width="1000" height="900"',
                         'viewBox="0 0 1000 900" width="1000" height="900" style="background:#000"', 1)
svg = svg.replace('role="img" aria-label="BMO-X: el gato con los ojos de neon">',
                  'role="img" aria-label="BMO-X: el gato con los ojos de neon">\n<style>' + CSS +
                  '</style>\n<rect x="0" y="0" width="1000" height="900" fill="#000"/>', 1)
# Solo ASCII en el repo: el kanji va como entidad.
svg = svg.replace('\u732b', '&#x732B;')
(AQUI / 'bmo-x-gato-hd.svg').write_text('<?xml version="1.0" encoding="UTF-8"?>\n' + svg + '\n')
print('escrito', AQUI / 'bmo-x-gato-hd.svg')
