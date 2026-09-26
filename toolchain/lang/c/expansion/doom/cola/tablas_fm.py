"""Genera `bmo_opl_tablas.c`: las tablas del sintetizador FM de DOOM en BMO-X.

No hay coma flotante en el sintetizador (ni libm en BMO-X): lo que pide una
raiz, un seno o una potencia se calcula AQUI, una vez, en el anfitrion, y
viaja como numeros. Regenerar:  python tablas_fm.py

Las tablas son funciones matematicas y los datos que el YM3812 publica (la
KSL); no hay codigo de nadie dentro.
"""

import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
DESTINO = os.path.join(os.path.dirname(HERE), "doom", "doomgeneric", "doomgeneric",
                       "bmo_opl_tablas.c")


def fila(nombre, tipo, valores, por_linea=12):
    out = ["static const %s %s[%d] = {" % (tipo, nombre, len(valores))]
    for i in range(0, len(valores), por_linea):
        out.append("    " + ", ".join(str(v) for v in valores[i:i + por_linea]) + ",")
    out.append("};")
    return "\n".join(out)


# El seno de un ciclo en 1024 pasos, a +-4095 (13 bits con signo: la salida
# de un operador del chip).
seno = [int(round(4095 * math.sin(2 * math.pi * i / 1024))) for i in range(1024)]

# La atenuacion en unidades de 0,1875 dB (la del envolvente del chip, 9 bits)
# a ganancia lineal en Q15. 511 unidades = 95,8 dB: ya es cero.
exp = [int(round(32768 * 10 ** (-(u * 0.1875) / 20))) for u in range(512)]

# 2^(k/384) en Q30: la nota en 1/32 de semitono (12 x 32 = 384 por octava).
pot2 = [int(round((1 << 30) * 2 ** (k / 384))) for k in range(384)]

# La KSL del chip: atenuacion de base por los 4 bits altos del F-Number.
kslrom = [0, 32, 40, 45, 48, 51, 53, 55, 56, 58, 59, 60, 61, 62, 63, 64]

# Volumen MIDI (0..127) a pasos de 0,75 dB (los del registro TL), con la
# curva que recomienda General MIDI: 40 log10(v/127). No es la tabla de DMX
# (no se ha copiado de ningun sitio); es una curva con motivo y se dice.
db = []
for v in range(128):
    if v == 0:
        db.append(63)
    else:
        db.append(min(63, int(round(-40 * math.log10(v / 127.0) / 0.75))))

texto = "\n\n".join([
    "/* bmo_opl_tablas.c -- AUTO-GENERADO por doom-port/tablas_fm.py. No se edita\n"
    " * a mano: se regenera. Seno, ganancia, potencias de 2, la KSL del chip y la\n"
    " * curva de volumen, calculados en el anfitrion porque BMO-X no tiene libm. */",
    fila("bmo_opl_seno", "int", seno),
    fila("bmo_opl_exp", "int", exp),
    fila("bmo_opl_pot2", "int", pot2, 8),
    fila("bmo_opl_kslrom", "int", kslrom, 16),
    fila("bmo_mus_db", "int", db, 16),
]) + "\n"

with open(DESTINO, "w", newline="\n") as f:
    f.write(texto)
print("escrito", DESTINO, len(texto), "bytes")
