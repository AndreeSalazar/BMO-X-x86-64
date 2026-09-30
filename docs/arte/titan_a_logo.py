#!/usr/bin/env python3
"""Convierte el logo de TITAN++ en el recurso que F1 (el TALLER) dibuja.

=== Por que paleta + PackBits, y no mascaras como el gato ===

El gato era 97% negro y TRES colores planos: dos mascaras de 1 bit y listo
(`gato_a_mascara.py`). Este logo es otra cosa -- medido, no supuesto:

    casi negro            68 %
    el resto              halos y degradados de azul a violeta

Un bit por pixel lo convierte en un recortable. Hacen falta los degradados, y
un decodificador JPEG en `no_std` serian miles de lineas para dibujar UNA
imagen fija. Asi que el trabajo caro se hace aqui, una vez:

    1. escalar a 512x512 (LANCZOS): llena la presentacion y el fondo
    2. lo casi negro (max < 16) se hace negro EXACTO: el ruido del JPEG en el
       fondo es lo que impedia comprimir las rachas
    3. 128 colores (MEDIANCUT, sin tramado: el tramado rompe las rachas y en
       un halo no se nota la falta)
    4. PackBits sobre los indices: una racha de negro son dos bytes

Sale 106.089 B (medido el 29-09; a 384 eran 63 KB, pero el logo quedaba
chico en una ventana de 760 de alto y las letras se emborronaban) y se
descomprime en quince lineas (`taller/src/art.rs`). El tope del cargador es
1 MiB.

=== El formato (TLG1) ===

    0   "TLG1"
    4   ancho      u16 LE
    6   alto       u16 LE
    8   dibujo     u16 LE   filas que son DIBUJO; debajo van las letras
   10   colores    u16 LE   <= 256
   12   paleta     colores x (R, G, B)
    .   PackBits   byte n:  n < 128  -> n+1 indices literales detras
                            n >= 128 -> el indice de detras, n-126 veces

`dibujo` se MIDE barriendo la imagen (la primera banda vacia despues del
dibujo), no a ojo: el fondo del lienzo usa solo el dibujo -- las letras detras
de los nodos serian ruido -- y la presentacion usa el logo entero.

=== Como se ejecuta ===

    pip install pillow
    python docs/arte/titan_a_logo.py

Escribe `Ultra_userspace/apps/taller/arte/titan.bin`. **La salida esta
commiteada**: el build NO depende de Python. Este script existe para poder
REHACERLA si el logo cambia -- un recurso generado sin su generador es un
recurso que nadie puede volver a hacer (la leccion del gato).
"""

import struct
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    sys.exit("hace falta Pillow:  pip install pillow")

RAIZ = Path(__file__).resolve().parents[2]
FUENTE = Path(__file__).parent / "titan.jpg"
DESTINO = RAIZ / "Ultra_userspace" / "apps" / "taller" / "arte" / "titan.bin"

LADO = 512
COLORES = 128
# Por debajo de esto en los tres canales es fondo: negro exacto.
NEGRO = 16
# Una fila es "vacia" si tiene menos de estos pixeles encendidos (max > 60).
VACIA = 3


def packbits(datos):
    out = bytearray()
    i, n = 0, len(datos)
    while i < n:
        j = i
        while j + 1 < n and datos[j + 1] == datos[i] and j - i < 127:
            j += 1
        racha = j - i + 1
        if racha >= 2:
            out += bytes([126 + racha, datos[i]])
            i = j + 1
            continue
        lit = [datos[i]]
        i += 1
        while i < n and len(lit) < 128 and not (i + 1 < n and datos[i + 1] == datos[i]):
            lit.append(datos[i])
            i += 1
        out += bytes([len(lit) - 1]) + bytes(lit)
    return out


def unpackbits(datos, total):
    """El mismo algoritmo que art.rs, para comprobar la salida aqui mismo."""
    out, i = bytearray(), 0
    while len(out) < total:
        n = datos[i]
        i += 1
        if n < 128:
            out += datos[i:i + n + 1]
            i += n + 1
        else:
            out += bytes([datos[i]]) * (n - 126)
            i += 1
    return bytes(out[:total])


def fila_del_corte(im):
    """La primera fila vacia de la banda que separa el dibujo de las letras:
    se busca desde el 70% del alto hacia abajo."""
    px = im.load()
    w, h = im.size

    def encendidos(y):
        return sum(1 for x in range(w) if max(px[x, y]) > 60)

    for y in range(int(h * 0.7), h):
        if encendidos(y) < VACIA:
            return y
    return h


def main():
    if not FUENTE.exists():
        sys.exit(f"no encuentro {FUENTE}")
    im = Image.open(FUENTE).convert("RGB").resize((LADO, LADO), Image.LANCZOS)
    px = im.load()
    for y in range(LADO):
        for x in range(LADO):
            if max(px[x, y]) < NEGRO:
                px[x, y] = (0, 0, 0)
    dibujo = fila_del_corte(im)
    q = im.quantize(COLORES, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    paleta = q.getpalette()[: COLORES * 3]
    indices = q.tobytes()
    datos = packbits(indices)
    if unpackbits(datos, len(indices)) != indices:
        sys.exit("PackBits no vuelve a lo mismo: no se escribe nada")
    cabecera = b"TLG1" + struct.pack("<HHHH", LADO, LADO, dibujo, COLORES)
    DESTINO.parent.mkdir(parents=True, exist_ok=True)
    DESTINO.write_bytes(cabecera + bytes(paleta) + datos)
    print(f"{DESTINO.relative_to(RAIZ)}: {LADO}x{LADO}, dibujo {dibujo} filas, "
          f"{COLORES} colores, {len(cabecera) + len(paleta) + len(datos)} bytes")


if __name__ == "__main__":
    main()
