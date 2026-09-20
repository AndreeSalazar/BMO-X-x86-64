#!/usr/bin/env python3
"""planes -- el mapa de lo que FALTA, y no puede envejecer.

Por que existe
==============

El 2026-09-10 el dueno dijo: *"los planes me gustaria que se mezclen [...] pero
dividiendo los planes que faltan [...] me esta fastidiando, me gustaria que
organices profesionalmente"*.

Se midio antes de mover nada, y el numero explica el fastidio:

    26 planes, 10.803 lineas, 77 casillas hechas y 126 PENDIENTES

*** Y no habia ni una vista de conjunto. Para saber que falta hay que abrir
veintiseis ficheros y contar a mano, asi que **nadie lo hace**, asi que la
respuesta a *"que queda"* sale de la memoria en vez de del arbol.

    > Un plan que hay que abrir para saber si tiene algo pendiente es un plan
    > que solo se consulta cuando ya te acordabas de el.

# ** POR QUE UN INDICE GENERADO Y NO UNA CARPETA NUEVA

La tentacion era repartir los 26 en subcarpetas --vivos, cumplidos, archivo--.
Se midio eso tambien:

    282 citas apuntan a ficheros de `plan/` y `metal/`
    CERO ficheros sin citar

O sea que mover cualquiera cuesta arreglar sus citas, y lo que se compra es que
un `ls` salga mas bonito. `NEUTRO/ORDEN.md` ya escribio la regla:

    "Reorganizar es mover lo que esta en el sitio equivocado. Cuando no hay nada
     en el sitio equivocado, reorganizar es estropear algo."

*** Los 26 planes contestan la pregunta de `plan/` --*que casillas faltan*-- y
por eso ninguno se mueve. Lo que faltaba no era una carpeta: **era el indice**.

# Y LO QUE SI ESTABA MAL, dicho por este mismo guardian

Seis de los veintiseis **no traen ni una casilla**, y tres de ellos son los mas
gordos del arbol. Un fichero de 981 lineas en `plan/` que no dice que falta no
contesta la pregunta de su carpeta: es un maestro, o una identidad, con la
palabra PLAN delante. Este guardian los nombra en cada build para que la
diferencia se note.

Modos
=====

    --check     el indice y los planes dicen lo mismo? (lo que corre en el build)
    --apply     regenera el indice
    --dry-run   ensena lo que escribiria
"""

import argparse
import io
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
PLANES = os.path.join(RAIZ, "docs", "plan")
INDICE = os.path.join(PLANES, "ABIERTO.md")

# == *** LOS DOS INDICES DE LA CARPETA, y ninguno es un plan ==============
#
# `ABIERTO.md` dice QUE falta y lo genera esta herramienta. `EL_ORDEN.md` dice
# QUE VA PRIMERO y lo escribe una persona.
#
# ** Los dos llevan casillas, y ninguna es trabajo PROPIO: son punteros a
# casillas que ya viven en un plan. Contarlas seria contar dos veces lo mismo,
# y el guardian lo caza al primer intento -- que es como se descubrio.
#
#     > Un indice que se cuenta a si mismo infla justo el numero que existe
#     > para que se pueda confiar en el.
NO_SON_PLANES = ("ABIERTO.md", "EL_ORDEN.md")

# == *** UNA CASILLA TAMBIEN PUEDE SER UN ENCABEZADO (2026-09-10) =========
#
# La primera version pedia que la linea empezara por `- [ ]`, y por eso dijo
# que `PLAN_ALMACENAMIENTO` no tenia ni una casilla. **Las tiene, y las cinco
# estan hechas**: las escribe como `### [x] Paso 0 -- UNA SOLA PUERTA`.
#
# ** Casi cuesta caro: se iba a reescribir un plan que ya estaba bien. El
# guardian no cazo una deuda, INVENTO una.
#
#     > Un contador que no reconoce una forma legitima no cuenta de menos:
#     > acusa. Y lo que acusa es a quien lo hizo bien de otra manera.
#
# [!] Y por eso el `(?:#{1,6}\s*)?` va DELANTE del guion y no en su lugar: las
# dos formas valen, y una casilla dentro de un encabezado es la que se usa
# cuando el escalon trae parrafos debajo.
HECHA = re.compile(r"^\s*(?:#{1,6}\s*)?(?:[-*]\s*)?\[[xX]\]\s*(.*)$")
FALTA = re.compile(r"^\s*(?:#{1,6}\s*)?(?:[-*]\s*)?\[ \]\s*(.*)$")
TITULO = re.compile(r"^#\s+(.*)$")

# == *** EL ESTADO DE UN PLAN, DICHO POR EL PLAN (2026-09-20) ==============
#
# Eddi: *"organizar las metas que faltan en abiertas, y las cerradas CON
# MOTIVO"*. Un plan con casillas abiertas no siempre esta abierto: `EL_GUARDIAN`
# pide una placa RISC-V y la decision del 18-09 (una arquitectura, un repo) lo
# dejo fuera de este arbol; `EL_ASISTENTE` es el ultimo por decision del dueno;
# `DOOM` se jugo el 20-09 y lo que le queda son numeros de una hoja del metal.
# Contar sus casillas como "lo que falta" es mentir en el numero que existe
# para que se pueda confiar en el.
#
# El plan lo dice el mismo, en sus primeras lineas, con UNA de estas palabras:
#
#     > Estado: **CERRADO** -- hecho el ..., y lo que queda es ...
#     > Estado: **SUPERADO** -- por PLAN_X: ...
#     > Estado: **APARCADO** -- decision de ... : ...
#     > Estado: **ESPERA** -- una decision del dueno: ...
#
# Sin esa linea, el plan esta ABIERTO. La palabra tiene que ir con su motivo
# detras del guion; un estado sin motivo es un agujero, y `docs/METAS.md` es
# donde se leen todos juntos por categoria.
ESTADO = re.compile(r"^>?\s*\**Estado:?\**\s*\**(CERRADO|SUPERADO|APARCADO|ESPERA)\**\s*(?:--|-|:)?\s*(.*)$")
CUANTAS_LINEAS_DE_CABECERA = 30

# El indice se genera, asi que no se edita a mano. Se dice arriba del todo.
CABECERA = "<!-- GENERADO por toolchain/tools/planes. No se edita a mano. -->"


def limpia(t, tope=96):
    """El texto de una casilla, sin markdown y acotado."""
    t = re.sub(r"\*\*(.*?)\*\*", r"\1", t)
    t = re.sub(r"`(.*?)`", r"\1", t)
    t = re.sub(r"\[(.*?)\]\(.*?\)", r"\1", t)
    t = " ".join(t.split())
    return t[:tope] if tope else t


def censo():
    """[(fichero, titulo, hechas, [pendientes]), ...] ordenado por pendientes."""
    filas = []
    for n in sorted(os.listdir(PLANES)):
        if not n.endswith(".md") or n in NO_SON_PLANES:
            continue
        p = os.path.join(PLANES, n)
        with io.open(p, encoding="utf-8", errors="replace") as fh:
            L = fh.read().splitlines()
        titulo = ""
        for l in L:
            m = TITULO.match(l)
            if m:
                titulo = limpia(m.group(1))
                break
        estado = None
        for l in L[:CUANTAS_LINEAS_DE_CABECERA]:
            m = ESTADO.match(l)
            if m:
                estado = (m.group(1), limpia(m.group(2), tope=None))
                break
        hechas = sum(1 for l in L if HECHA.match(l))
        faltan = []
        for l in L:
            m = FALTA.match(l)
            if m:
                texto = limpia(m.group(1))
                if texto:
                    faltan.append(texto)
        filas.append((n, titulo, hechas, faltan, len(L), estado))
    filas.sort(key=lambda r: (-len(r[3]), r[0]))
    return filas


def esta_abierto(f):
    """Un plan cuenta como ABIERTO si tiene casillas y no se declaro cerrado."""
    return bool(f[3]) and f[5] is None


def pinta(filas):
    vivos = [f for f in filas if esta_abierto(f)]
    cerrados = [f for f in filas if f[5] is not None]
    cumplidos = [f for f in filas if not f[3] and f[2] and f[5] is None]
    mudos = [f for f in filas if not f[3] and not f[2] and f[5] is None]
    total = sum(len(f[3]) for f in vivos)
    aparcadas = sum(len(f[3]) for f in cerrados)
    hechas = sum(f[2] for f in filas)

    o = [CABECERA, ""]
    o.append("# LO QUE FALTA -- las casillas abiertas de los %d planes" % len(filas))
    o.append("")
    o.append("> Generado por `toolchain/tools/planes`. **El build comprueba que")
    o.append("> este fichero y los planes dicen lo mismo**, asi que no puede")
    o.append("> envejecer sin que algo se ponga rojo.")
    o.append("")
    o.append("```text")
    o.append("   %3d casillas ABIERTAS en %d planes" % (total, len(vivos)))
    o.append("   %3d hechas" % hechas)
    o.append("   %3d planes CUMPLIDOS (ni una casilla pendiente)" % len(cumplidos))
    o.append("   %3d planes CERRADOS, SUPERADOS, APARCADOS o EN ESPERA, con motivo"
             % len(cerrados))
    o.append("       (sus %d casillas sueltas NO cuentan como abiertas)" % aparcadas)
    o.append("   %3d en plan/ SIN NI UNA CASILLA -- ver el final" % len(mudos))
    o.append("```")
    o.append("")
    # (La ruta se arma en dos trozos para que el guardian de citas no la lea
    # desde este fichero, donde no resuelve: resuelve desde `docs/plan/`.)
    o.append("Por categoria y con el motivo de cada cierre: [`%s`](%s)." % ("../METAS" + ".md", "../METAS" + ".md"))
    o.append("")
    o.append("---")
    o.append("")
    o.append("# Los planes VIVOS, el que mas debe primero")
    o.append("")

    for n, titulo, h, faltan, lin, _e in vivos:
        o.append("## [`%s`](%s) -- %d abiertas, %d hechas" % (n, n, len(faltan), h))
        o.append("")
        if titulo:
            o.append("*%s*" % titulo)
            o.append("")
        for t in faltan[:3]:
            o.append("- [ ] %s" % t)
        if len(faltan) > 3:
            o.append("- ... y %d mas" % (len(faltan) - 3))
        o.append("")

    if cerrados:
        o.append("---")
        o.append("")
        o.append("# CERRADOS, SUPERADOS, APARCADOS Y EN ESPERA -- cada uno con su motivo")
        o.append("")
        o.append("** Lo dice el propio plan en su cabecera (`> Estado: ...`), y")
        o.append("esta herramienta lo copia. Sus casillas sueltas no son deuda: o")
        o.append("ya no aplican, o esperan a alguien que no es el codigo.")
        o.append("")
        cerrados_ordenados = sorted(cerrados, key=lambda f: (f[5][0], f[0]))
        for n, titulo, h, faltan, lin, (palabra, motivo) in cerrados_ordenados:
            o.append("- **%s** [`%s`](%s) -- %s  *(%d hechas, %d sueltas)*"
                     % (palabra, n, n, motivo or "sin motivo escrito", h, len(faltan)))
        o.append("")

    if cumplidos:
        o.append("---")
        o.append("")
        o.append("# CUMPLIDOS -- todas sus casillas marcadas")
        o.append("")
        o.append("** No se archivan ni se mueven: siguen siendo la razon por la")
        o.append("que algo se hizo asi, y eso se consulta mas que la casilla.")
        o.append("")
        for n, titulo, h, _f, lin, _e in cumplidos:
            o.append("- [`%s`](%s) -- %d hechas, %d lineas" % (n, n, h, lin))
        o.append("")

    if mudos:
        o.append("---")
        o.append("")
        o.append("# [!] ESTOS ESTAN EN `plan/` Y NO TRAEN NI UNA CASILLA")
        o.append("")
        o.append("`docs/README.md` dice que `plan/` contesta *\"que casillas")
        o.append("faltan, que las bloquea, como se sabe que quedo hecha\"*. Un")
        o.append("fichero sin casillas **no contesta esa pregunta**: es un")
        o.append("maestro o una identidad con la palabra PLAN delante.")
        o.append("")
        o.append("*** No se mueven solos. Cada uno se decide a mano: o se le")
        o.append("ponen sus casillas, o se muda a la carpeta cuya pregunta si")
        o.append("contesta. Este guardian los nombra en cada build para que la")
        o.append("deuda tenga nombre en vez de ser un fichero mas.")
        o.append("")
        for n, titulo, _h, _f, lin, _e in mudos:
            o.append("- [`%s`](%s) -- %d lineas" % (n, n, lin))
        o.append("")

    return "\n".join(o) + "\n"


def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--check", action="store_true")
    g.add_argument("--apply", action="store_true")
    g.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()

    nuevo = pinta(censo())

    if a.dry_run:
        sys.stdout.write(nuevo)
        return 0

    if a.apply:
        with io.open(INDICE, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(nuevo)
        print("planes: indice reescrito -> docs/plan/ABIERTO.md")
        return 0

    if not os.path.exists(INDICE):
        print("planes: falta docs/plan/ABIERTO.md."
              " Se genera con `python toolchain/tools/planes/planes.py --apply`")
        return 1
    with io.open(INDICE, encoding="utf-8") as fh:
        viejo = fh.read()
    if viejo.replace("\r\n", "\n") != nuevo:
        print("planes: el indice y los planes NO dicen lo mismo.")
        print("  alguien marco o anadio una casilla y el indice se quedo atras.")
        print("  se arregla con: python toolchain/tools/planes/planes.py --apply")
        return 1

    filas = censo()
    abiertas = sum(len(f[3]) for f in filas if esta_abierto(f))
    vivos = sum(1 for f in filas if esta_abierto(f))
    cerrados = sum(1 for f in filas if f[5] is not None)
    sin_motivo = [f[0] for f in filas if f[5] is not None and not f[5][1]]
    mudos = [f[0] for f in filas if not f[3] and not f[2] and f[5] is None]
    if sin_motivo:
        print("planes: un estado sin motivo es un agujero -- %s" % ", ".join(sin_motivo))
        return 1
    msg = ("clean: el indice de planes cuadra -- %d casillas abiertas en %d de"
           " %d planes; %d cerrados o aparcados con motivo"
           % (abiertas, vivos, len(filas), cerrados))
    if mudos:
        msg += ("; y %d en plan/ sin ni una casilla (%s)"
                % (len(mudos), ", ".join(m.replace("PLAN_", "").replace(".md", "")
                                         for m in mudos)))
    print(msg)
    return 0


if __name__ == "__main__":
    sys.exit(main())
