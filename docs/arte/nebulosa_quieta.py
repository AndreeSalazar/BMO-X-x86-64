"""LA NEBULOSA, QUIETA, como SVG (HM3b de docs/plan/PLAN_EL_HUD.md): el primer
fotograma del INICIO en la CPU, antes de que se condense en el SOL.

    python docs/arte/nebulosa_quieta.py > toolchain/tools/maqueta/escritorio/nebulosa.svg

Es UN fotograma (t = 0, ya con forma de gato) del estilo NEBULOSA de
docs/arte/estrella_gato.js: las nubes, los dos pilares de polvo, las 710
particulas de gas (con la misma semilla y en el mismo orden de tiradas) y las
estrellas recien nacidas con sus seis puntas. Lo que alli es un filtro (el
humo, el brillo) aqui es un degradado o un trazo ancho y tenue: el lector de
SVG de MAQUETA 3 no lee filtros. Despues de cambiar esto, se regenera la cara:

    cargo run -p bmo-maqueta -- toolchain/tools/maqueta/escritorio/nebulosa.maqueta \\
        Ultra_userspace/services/director/src/scene/nebulosa_gen.rs
"""
import math
import sys

from cabeza_gato import semilla, sobre, dentro, CABEZA, f, o

r = semilla(42)
out = []
w = out.append

NUBES = [('#FF2E88', -70, -100), ('#8C52FF', 60, -96), ('#3DA5FF', -50, 50), ('#FF2E88', 70, 60),
         ('#5EF2E6', 0, -10), ('#8C52FF', 0, 90), ('#3DA5FF', -80, -20), ('#FF2E88', 85, -10)]
PILARES = ['M -70 120 C -78 70 -50 36 -58 -6 C -60 -26 -40 -32 -36 -10 C -30 30 -42 76 -30 120 Z',
           'M 34 120 C 28 90 52 64 46 34 C 44 20 60 16 64 32 C 70 62 60 96 70 120 Z']

# El gas: el mismo orden de tiradas que `gas()` de la maqueta.
gas = []


def particula(x, y, col, rad, op, orb):
    r()  # el angulo de lejos
    r()  # lo lejos (solo cuenta mientras se junta)
    fase = r() * 6.28
    gas.append((x + math.cos(fase) * orb, y + math.sin(fase * 1.3) * orb, col, rad, op))


while len(gas) < 560:
    x = (r() - .5) * 230
    y = -160 + r() * 280
    if not dentro(x, y):
        continue
    q = math.hypot(x, (y + 10) * .9) / 120
    if q < .35:
        col = '#F2F7F9' if r() < .5 else '#5EF2E6'
    elif q < .7:
        col = '#3DA5FF' if r() < .5 else '#FF2E88'
    else:
        col = '#8C52FF' if r() < .5 else '#FF2E88'
    rad = .7 + r() * 2
    op = .3 + r() * .6
    orb = 2 + r() * 8
    particula(x, y, col, rad, op, orb)
for b in range(150):
    s = sobre(b / 150 + r() * .004)
    j = (r() - .5) * 6
    col = '#FF2E88' if r() < .5 else '#F2F7F9'
    rad = .8 + r() * 1.4
    op = .5 + r() * .5
    particula(s[0] + j, s[1] + j, col, rad, op, 1.5)

w('<?xml version="1.0" encoding="UTF-8"?>')
w('''<!-- LA NEBULOSA, QUIETA: el primer fotograma del INICIO de mision (HM3b de
     docs/plan/PLAN_EL_HUD.md), antes de que el gas se condense en el SOL. Es
     el estilo NEBULOSA de docs/arte/estrella_gato.js a t = 0, con su semilla.
     Lo escribe docs/arte/nebulosa_quieta.py; se edita alli. -->''')
w('<svg xmlns="http://www.w3.org/2000/svg" viewBox="-240 -240 480 480" width="720" height="720">')
w('  <defs>')
w('    <style>')
w('      .pilar { fill: #120A1E; fill-opacity: .78; stroke: #FFD45E; stroke-width: 3; stroke-opacity: .55 }')
w('      .borde { fill: none; stroke: #5EF2E6; stroke-width: 2; stroke-opacity: .25 }')
w('      .borde-luz { fill: none; stroke: #5EF2E6; stroke-width: 9; stroke-opacity: .07 }')
w('    </style>')
w('    <radialGradient id="aura" cx="0" cy="-10" r="170" gradientUnits="userSpaceOnUse">')
w('      <stop offset=".45" stop-color="#8C52FF" stop-opacity=".22"/>')
w('      <stop offset="1" stop-color="#8C52FF" stop-opacity="0"/>')
w('    </radialGradient>')
for i, (c, x, y) in enumerate(NUBES):
    w('    <radialGradient id="nube%d">' % i)
    w('      <stop offset="0" stop-color="%s" stop-opacity=".4"/>' % c)
    w('      <stop offset=".55" stop-color="%s" stop-opacity=".22"/>' % c)
    w('      <stop offset="1" stop-color="%s" stop-opacity="0"/>' % c)
    w('    </radialGradient>')
w('  </defs>')
w('  <circle cx="0" cy="-10" r="170" fill="url(#aura)"/>')
for i, (c, x, y) in enumerate(NUBES):
    # La nube de la maqueta lleva un humo de 16: aqui su borde se apaga en
    # esos 16 de mas, y no se recorta a la cabeza (el degradado ya la funde).
    rad = 46 + (i % 3) * 10 + 16
    w('  <circle cx="%s" cy="%s" r="%s" fill="url(#nube%d)"/>' % (f(x + math.cos(i) * 14), f(y + math.sin(i) * 10), f(rad), i))
for p in PILARES:
    w('  <path class="pilar" d="%s"/>' % p)
for (x, y, col, rad, op) in gas:
    w('  <circle cx="%s" cy="%s" r="%s" fill="%s" fill-opacity="%s"/>' % (f(x), f(y), f(rad), col, o(op)))
w('  <path class="borde-luz" d="%s"/>' % CABEZA)
w('  <path class="borde" d="%s"/>' % CABEZA)
for i, (x, y, largo, col) in enumerate([(-40, 4, 30, '#5EF2E6'), (40, 4, 30, '#5EF2E6'), (-64, -70, 12, '#FFD45E'), (58, 72, 10, '#F2F7F9'),
                                         (-50, -12, 7, '#F2F7F9'), (76, -40, 8, '#FFD45E'), (-20, 84, 6, '#5EF2E6')]):
    luz = .65 + .35 * math.sin(i * 2)
    # El brillo de cada una va en cada trazo: el lector no compone la
    # opacidad de un grupo de varias cosas (pide pintarlo aparte).
    w('  <g transform="translate(%d %d)">' % (x, y))
    d = []
    for a in range(6):
        ang = a * math.pi / 3 + math.pi / 2
        d.append('M0 0 L%s %s' % (f(math.cos(ang) * largo), f(math.sin(ang) * largo)))
    w('    <path d="%s" fill="none" stroke="%s" stroke-width="1.4" stroke-opacity="%s" stroke-linecap="round"/>' % (' '.join(d), col, o(.8 * luz)))
    w('    <path d="M%s 0 H%s" fill="none" stroke="%s" stroke-width="1" stroke-opacity="%s"/>' % (f(-largo * .45), f(largo * .45), col, o(.6 * luz)))
    w('    <circle r="%s" fill="#FFFFFF" fill-opacity="%s"/>' % (f(largo * .14), o(luz)))
    w('    <circle r="%s" fill="%s" fill-opacity="%s"/>' % (f(largo * .32), col, o(.3 * luz)))
    w('  </g>')
w('</svg>')
sys.stdout.write('\n'.join(out) + '\n')
