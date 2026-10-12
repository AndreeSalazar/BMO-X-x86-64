"""Las dimensiones: los hipercubos de 0D a 10D, y las paralelas de BMO-X.

Uso: python3 docs/arte/dimensiones.py docs/arte   (pide matplotlib y numpy)
Los datos de BMO-X son los del banco de `platform/shared/verrano-ayni`.
"""
import itertools
import math
import sys

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.collections import LineCollection
from matplotlib.patches import Circle, Rectangle, Polygon

FONDO = "#fcfcfb"
TINTA = "#0b0b0b"
TINTA2 = "#52514e"
AZUL = "#2a78d6"
NARANJA = "#eb6834"
REJILLA = "#e4e3df"
salida = sys.argv[1]

plt.rcParams.update({"font.family": "DejaVu Sans", "text.color": TINTA,
                     "axes.edgecolor": REJILLA, "axes.labelcolor": TINTA2,
                     "xtick.color": TINTA2, "ytick.color": TINTA2})


def hipercubo(n):
    """Vertices (+-1)^n proyectados al plano de Petrie y sus aristas."""
    if n == 0:
        return np.zeros((1, 2)), []
    ang = np.array([k * math.pi / n for k in range(n)])
    base = np.stack([np.cos(ang), np.sin(ang)], axis=1)  # n x 2
    vs = np.array(list(itertools.product([-1, 1], repeat=n)), dtype=float)
    p = vs @ base
    aristas = []
    for i in range(len(vs)):
        for k in range(n):
            j = i ^ (1 << (n - 1 - k))
            if i < j:
                aristas.append((p[i], p[j]))
    return p, aristas


# ---------------------------------------------------------------- figura 1
fig = plt.figure(figsize=(16, 13), facecolor=FONDO)
fig.suptitle("Del punto al hipercubo de 10D: cuanto mas dimensiones, mas se parece a un circulo",
             fontsize=19, fontweight="bold", y=0.985)
fig.text(0.5, 0.952,
         "Cada cubo de n dimensiones, visto de frente por su eje mas simetrico (la proyeccion de Petrie). "
         "La linea discontinua es el circulo que lo envuelve.",
         ha="center", fontsize=12, color=TINTA2)

for n in range(11):
    ax = fig.add_subplot(3, 4, n + 1)
    ax.set_facecolor(FONDO)
    ax.set_aspect("equal")
    ax.axis("off")
    p, aristas = hipercubo(n)
    r = max(1e-9, np.max(np.linalg.norm(p, axis=1)))
    if n >= 3:
        ax.add_patch(Circle((0, 0), r, fill=False, ls=(0, (4, 4)), lw=1, color=TINTA2, alpha=0.6))
    if aristas:
        alfa = 0.9 if n <= 4 else max(0.12, 0.9 / (1.6 ** (n - 4)))
        ancho = 2 if n <= 4 else max(0.35, 2 / (1.35 ** (n - 4)))
        ax.add_collection(LineCollection(aristas, colors=AZUL, linewidths=ancho, alpha=alfa))
    tam = 60 if n <= 3 else max(2, 60 / (2 ** (n - 3)))
    ax.scatter(p[:, 0], p[:, 1], s=tam, color=TINTA, zorder=3, linewidths=0)
    lim = 1.25 * max(r, 1)
    ax.set_xlim(-lim, lim)
    ax.set_ylim(-lim, lim)
    nombre = {0: "un punto", 1: "una linea", 2: "un cuadrado", 3: "el cubo (el de la CPU)",
              4: "el teseracto"}.get(n, "")
    ax.set_title(f"{n}D  {nombre}".strip(), fontsize=14, fontweight="bold", loc="left")
    ax.text(0, -lim * 0.98, f"{2**n:,} vertices  ·  {n * 2**(n-1) if n else 0:,} aristas".replace(",", "."),
            ha="center", va="bottom", fontsize=10.5, color=TINTA2)

# El panel 12: que tan redondo es el contorno.
ax = fig.add_subplot(3, 4, 12)
ax.set_facecolor(FONDO)
ns = np.arange(2, 41)
red = 100 * np.cos(math.pi / (2 * ns))
ax.plot(ns, red, color=AZUL, lw=2)
marcas = np.arange(2, 11)
ax.scatter(marcas, 100 * np.cos(math.pi / (2 * marcas)), s=36, color=AZUL, zorder=3,
           edgecolors=FONDO, linewidths=2)
ax.axhline(100, color=TINTA2, lw=1, ls=(0, (4, 4)))
ax.text(40, 100.3, "circulo perfecto (100 %)", ha="right", va="bottom", fontsize=9.5, color=TINTA2)
for n_, dy in ((3, -3.2), (10, -3.2)):
    v = 100 * math.cos(math.pi / (2 * n_))
    ax.annotate(f"{n_}D: {v:.1f} %", (n_, v), xytext=(n_ + 2.5, v + dy), fontsize=10, color=TINTA,
                arrowprops=dict(arrowstyle="-", color=TINTA2, lw=0.8))
ax.set_ylim(68, 103)
ax.set_xlim(1, 41)
ax.set_xlabel("dimensiones")
ax.set_title("Que tan redondo es el contorno", fontsize=14, fontweight="bold", loc="left")
ax.set_ylabel("radio de dentro / de fuera (%)")
ax.grid(axis="y", color=REJILLA, lw=0.8)
for s in ("top", "right"):
    ax.spines[s].set_visible(False)

fig.text(0.5, 0.012,
         "El contorno de nD es un poligono de 2n lados: nunca llega a ser un circulo, pero se le acerca sin fin "
         "(10D: 98,8 %; 40D: 99,9 %).",
         ha="center", fontsize=12, color=TINTA)
fig.tight_layout(rect=(0, 0.03, 1, 0.94))
fig.savefig(f"{salida}/dimensiones-hipercubos.png", dpi=110, facecolor=FONDO)

# ---------------------------------------------------------------- figura 2
fig = plt.figure(figsize=(16, 9.6), facecolor=FONDO)
fig.suptitle("Las dimensiones PARALELAS de BMO-X: las que si se miden",
             fontsize=19, fontweight="bold", y=0.985)
fig.text(0.5, 0.94, "La GPU dibuja el corte (3D). La CPU ve la serie de cortes (4D) y la mano que elige el siguiente (5D).",
         ha="center", fontsize=12, color=TINTA2)

# 1D: un pixel a lo largo del tiempo.
ax = fig.add_subplot(2, 3, 1)
ax.set_facecolor(FONDO)
t = np.arange(60)
rng = np.random.default_rng(3)
v = np.where(t < 32, 0.42, 0.42 + 0.3 * (1 - np.exp(-(t - 32) / 4))) + rng.normal(0, 0.004, 60)
ax.plot(t, v, color=AZUL, lw=2)
ax.axvspan(0, 31, color=REJILLA, alpha=0.6, lw=0)
ax.text(15.5, 0.8, "igual 32 fotogramas:\nno se recalcula (A2)", ha="center", va="top", fontsize=10.5, color=TINTA)
ax.text(50, 0.56, "cambia:\nse vuelve a sombrear", ha="center", va="top", fontsize=10.5, color=TINTA)
ax.set_ylim(0.3, 0.85)
ax.set_xlabel("fotograma")
ax.set_ylabel("luz del pixel")
ax.set_title("1D  un pixel en el tiempo", fontsize=14, fontweight="bold", loc="left")
for s in ("top", "right"):
    ax.spines[s].set_visible(False)

# 2D: el mapa de UMA (el del banco real, `suave`).
ax = fig.add_subplot(2, 3, 2)
ax.set_facecolor(FONDO)
ax.set_aspect("equal")
ax.axis("off")
mapa = ["X1", "X2x2", "X2x2", "X2x2", "X1", "X1", "X1", "X1",
        "X2x2", "X2x2", "X2x2", "X2x2", "X1", "X1", "X1", "X1",
        "X2x2", "X2x2", "X2x2", "X2x2", "X1", "X1", "X1", "X1",
        "X4x4", "X4x4", "X4x4", "X4x4", "X1", "X1", "X1", "X1"]
tono = {"X1": "#1c5cab", "X2x2": "#6da7ec", "X4x4": "#c7dcf6"}
for i, m in enumerate(mapa):
    cx, cy = i % 8, 3 - i // 8
    ax.add_patch(Rectangle((cx, cy), 1, 1, facecolor=tono[m], edgecolor=FONDO, lw=2))
    ax.text(cx + 0.5, cy + 0.5, m[1:], ha="center", va="center", fontsize=9,
            color="white" if m == "X1" else TINTA)
ax.text(0.5, 3.5 + 0.62, "HUD", ha="center", va="bottom", fontsize=9, color=TINTA2)
ax.text(2, -0.25, "cielo: grueso", ha="center", va="top", fontsize=10.5, color=TINTA)
ax.text(6, -0.25, "textura: entera", ha="center", va="top", fontsize=10.5, color=TINTA)
ax.set_xlim(-0.2, 8.2)
ax.set_ylim(-1.0, 4.7)
ax.set_title("2D  el mapa de UMA (banco: 62 % del trabajo)", fontsize=14, fontweight="bold", loc="left")

# 3D: el cubo, en perspectiva, como lo dibuja la CPU.
ax = fig.add_subplot(2, 3, 3)
ax.set_facecolor(FONDO)
ax.set_aspect("equal")
ax.axis("off")
vs = np.array(list(itertools.product([-1, 1], repeat=3)), dtype=float)
a, b = 0.62, -0.5
R = np.array([[math.cos(a), 0, math.sin(a)], [0, 1, 0], [-math.sin(a), 0, math.cos(a)]]) @ \
    np.array([[1, 0, 0], [0, math.cos(b), -math.sin(b)], [0, math.sin(b), math.cos(b)]])
w = vs @ R.T
z = w[:, 2] + 5
p = w[:, :2] * (3.2 / z)[:, None]
caras = [[0, 1, 3, 2], [4, 5, 7, 6], [0, 1, 5, 4], [2, 3, 7, 6], [0, 2, 6, 4], [1, 3, 7, 5]]
luz = np.array([-0.4, 0.8, -0.45])
luz /= np.linalg.norm(luz)
cerca = []
for c in sorted(caras, key=lambda c: -np.mean(z[c])):
    centro = w[c].mean(axis=0)
    normal = centro / np.linalg.norm(centro)
    camara = np.array([0.0, 0.0, -5.0])
    if normal @ (camara - centro) <= 0:  # la cara mira hacia atras: la CPU no la pinta
        continue
    cerca.append((float(normal @ luz), c))
# La luz decide el tono: la cara mas iluminada, la mas clara.
for (_, c), tono in zip(sorted(cerca, reverse=True), ("#a9c9f2", "#4a8ad8", "#1c5cab")):
    ax.add_patch(Polygon(p[c], closed=True, facecolor=tono, edgecolor=FONDO, lw=2.5))
ax.text(0, -1.25, "solo las caras que miran a la camara: las de atras no se pintan", ha="center", fontsize=10, color=TINTA2)
ax.set_xlim(-1.3, 1.3)
ax.set_ylim(-1.3, 1.3)
ax.set_title("3D  el corte que dibuja la GPU", fontsize=14, fontweight="bold", loc="left")

# 4D: la serie de cortes (PACHA).
ax = fig.add_subplot(2, 3, 4)
ax.set_facecolor(FONDO)
ax.set_aspect("equal")
ax.axis("off")
for k in range(6):
    dx, dy = k * 0.55, k * 0.32
    al = 0.25 + 0.15 * k
    ax.add_patch(Rectangle((dx, dy), 2.2, 1.5, facecolor=FONDO, edgecolor=AZUL, lw=1.6, alpha=al))
    ax.add_patch(Circle((dx + 0.6 + 0.18 * k, dy + 0.75), 0.25, color=AZUL, alpha=al))
ax.annotate("", xy=(5.3, 2.1), xytext=(0.3, -0.25),
            arrowprops=dict(arrowstyle="->", color=TINTA2, lw=1.2))
ax.text(3.6, 0.15, "el tiempo: cada fotograma es un corte;\nla CPU ve la serie y sabe que se mueve",
        fontsize=10.5, color=TINTA)
ax.set_xlim(-0.4, 6.4)
ax.set_ylim(-0.6, 3.3)
ax.set_title("4D  PACHA, la serie de cortes", fontsize=14, fontweight="bold", loc="left")

# 5D: la mano (la frenada real del banco: de 100 px a parar en 350 px).
ax = fig.add_subplot(2, 3, (5, 6))
ax.set_facecolor(FONDO)
ts = np.arange(6) * 0.008
xs = 100 + 2000 * ts - 0.5 * 8000 * ts ** 2
ax.scatter(xs, np.full_like(xs, 300), s=50, color=AZUL, zorder=3, label="lo que el raton ya hizo (6 muestras, cada 8 ms)")
for ang, al in ((-20, 0.25), (-10, 0.35), (10, 0.35), (20, 0.25)):
    r = 250
    ax.plot([xs[-1], xs[-1] + r * math.cos(math.radians(ang))], [300, 300 + r * math.sin(math.radians(ang))],
            color=TINTA2, lw=1.2, ls=(0, (3, 3)), alpha=al + 0.3)
ax.plot([xs[-1], 350], [300, 300], color=NARANJA, lw=2.5)
ax.scatter([350], [300], s=140, color=NARANJA, zorder=4, edgecolors=FONDO, linewidths=2,
           label="donde va a parar: la prediccion (confianza 0,95)")
ax.scatter([350], [300], s=5200, facecolors="none", edgecolors=NARANJA, linewidths=1.4, linestyle=(0, (4, 3)), zorder=2)
ax.text(350, 392, "el foco: aqui todo el detalle", ha="center", va="bottom", fontsize=10.5, color=TINTA)
ax.text(560, 300, "las otras ramas posibles:\nsin confianza, no se usan", va="center", fontsize=10.5, color=TINTA2)
ax.text(100, 225, "Predecir para AHORRAR o PREPARAR, nunca para ACTUAR: no mueve, no apunta, no hace clic.",
        fontsize=11, color=TINTA, fontweight="bold")
ax.set_xlim(60, 760)
ax.set_ylim(205, 445)
ax.set_xlabel("x en pantalla (px)")
ax.set_yticks([])
ax.legend(loc="upper left", frameon=False, fontsize=10.5)
ax.set_title("5D  la mano: donde para el puntero (la frenada real del banco)", fontsize=14,
             fontweight="bold", loc="left")
for s in ("top", "right", "left"):
    ax.spines[s].set_visible(False)

fig.tight_layout(rect=(0, 0, 1, 0.93))
fig.savefig(f"{salida}/dimensiones-bmo-x.png", dpi=110, facecolor=FONDO)
print("ok")
