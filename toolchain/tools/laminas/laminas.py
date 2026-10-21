#!/usr/bin/env python3
"""laminas -- toda MAGIA del arbol es una LAMINA del censo o dice por que no.

== De donde sale, y con fecha ==

El 2026-10-14 el propietario: *"RAM = convertir como LAMINA ABSOLUTA, LAMINA
ese mismo para administrar y llamar"*, y *"LAMINA es inteligente por si sola
y el kernel ya sabe que hacer"*. A1 de `docs/plan/EL_FOCO.md`.

Un bloque que un lado presta a otro dice QUE ES en su primera palabra, y el
que lo toma busca esa palabra en `bmo_lamina::censo::CENSO`. Eso solo sirve
si el censo esta COMPLETO: un bloque nuevo con su magia y sin su fila es un
bloque que nadie sabe tomar, y la pregunta *"que se presta en esta maquina"*
vuelve a contestarse de memoria.

== Que comprueba ==

  1. Toda constante de Rust cuyo nombre lleva MAGIA / MAGIC / MAGICO, con un
     valor literal (`0x...` o `u32::from_le_bytes(*b"....")`), es una de dos:
       - una CLASE del censo (`platform/shared/bmo-lamina/src/censo.rs`), o
       - un FORMATO que no se presta (`NO_SON_LAMINAS`, abajo, con motivo).
     Una que no es ninguna: el build para, con su fichero y su linea.
  2. Toda clase del censo existe en el arbol: su magia esta definida en algun
     fichero fuera de `bmo-lamina` (o es la de la antena, que es texto y la
     define `bmo-antena`). Un censo con filas muertas miente al reves.
  3. Ninguna magia esta en las dos listas.

== Antes de juzgar, demuestra que ve ==

Si no encuentra la magia de BVER (`bmo_verrano::lamina::MAGIA`) no ha mirado
nada: MUERTO, no "limpio".

Modos
=====

    --check     lo que corre en el build
    --list      la tabla entera: cada magia, que es y donde
"""

import argparse
import os
import re
import subprocess
import sys

CENSO = "platform/shared/bmo-lamina/src/censo.rs"

# Lo que tiene magia y NO se presta entre dos lados: formatos de fichero, lo
# que viaja dentro de una llamada (se copia y se juzga en la puerta), lo del
# arranque, y lo ajeno (protocolos y firmware de otros). Con su motivo.
NO_SON_LAMINAS = {
    0x464F_5343_424F_4F54: "FOSCBOOT: el traspaso del cargador al kernel, una vez al arrancar",
    0x424D_4F50_524C_4431: "la precarga del arranque, del cargador al kernel",
    0x4341_4944_4142_4D4F: "la caida: RAM del kernel que sobrevive al reinicio, nadie la presta",
    0x5442_4C45_524F_0001: "el tablero de CABINA dentro del kernel",
    0x424D_4F31: "el sello de un trap (BMO1), dentro del kernel",
    0x314F_4D42: "BMO_ABI_MAGIC: la version del ABI, un numero, no un bloque",
    0x3246_4542: "BEF2: el formato del ejecutable, un fichero",
    0x5345_5242: "BRES: los recursos de un paquete, un fichero",
    0x5145_5242: "BREQ: los requisitos de un .bex, una seccion de fichero",
    0x4152_4143: "CARA: el formato de una maqueta, un fichero",
    0x3146_5342: "BSF1: el formato de un sombreador, un fichero",
    0x4150_414D: "MAPA: el mapa de un .bsf, dentro del fichero",
    0x0723_0203: "SPIR-V: formato ajeno (Khronos), un fichero",
    0x20B: "PE32+: formato ajeno (Microsoft), un fichero",
    0x304E_5256: "VRN0: el paquete de una llamada a la 3060; se copia y se juzga en la puerta",
    0x314E_5256: "VRN1: el paquete de una llamada a la 3060; se copia y se juzga en la puerta",
    0x324E_5256: "VRN2: la receta de una llamada a la 3060; se copia y se juzga en la puerta",
    0x10DE: "el id de NVIDIA en el firmware del booter: ajeno",
    0xDC3A_AE21_371A_60B3: "la WPR del GSP: firmware ajeno",
    0x2050_5347: "'GSP ' de los argumentos del GSP-RM 615 (GSP_ARGUMENTS_CACHED): firmware ajeno",
    0x4D43_5450: "MCTP: trama del protocolo externo NVIDIA 615 dentro de la cola DMA, no una clase de lamina BMO",
    0x4341_4231: "CAB1: el formato de un evento de CABINA",
    0x5254_4E41_B4D0_424D: "la lista de huecos del monton, dentro del bloque de UN programa",
    0x6382_5363: "la cookie de DHCP: protocolo ajeno (RFC 2131)",
    0x1993_0520: "la excepcion de C++ de MSVC: ABI ajeno",
    0x1993_0522: "la excepcion de C++ de MSVC: ABI ajeno",
}

# La lamina de la antena es TEXTO: su magia son las cuatro primeras letras de
# su cabecera `LAMINA ...`, y la nombra `bmo-antena` en su lector.
MAGIA_BVER = 0x5245_5642

RE_CONST = re.compile(
    r'^\s*(?:pub(?:\([^)]*\))?\s+)?const\s+([A-Z0-9_]*MAGI[A-Z0-9_]*)\s*:\s*u(?:8|16|32|64)\s*=\s*([^;]+);', re.M)
RE_HEX = re.compile(r'^0x([0-9A-Fa-f_]+)$')
RE_LETRAS = re.compile(r'^u(?:32|64)::from_le_bytes\(\*b"(.{4})"\)$')
RE_CENSO = re.compile(r'magia:\s*0x([0-9A-Fa-f_]+),\s*\n\s*nombre:\s*"([^"]+)"')


def raiz():
    return os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))


def ficheros_rs():
    r = subprocess.run(["git", "ls-files", "-z", "--", "*.rs"], cwd=raiz(), capture_output=True)
    if r.returncode != 0:
        return None
    return [p for p in r.stdout.decode("utf-8", "replace").split("\0") if p]


def valor(expr):
    """El entero de un literal de magia, o None si no es un literal."""
    e = expr.strip()
    m = RE_HEX.match(e)
    if m:
        return int(m.group(1).replace("_", ""), 16)
    m = RE_LETRAS.match(e)
    if m:
        return int.from_bytes(m.group(1).encode("latin-1"), "little")
    return None


def leer_censo(texto):
    return {int(h.replace("_", ""), 16): n for h, n in RE_CENSO.findall(texto)}


def juzgar(censo, magias):
    """`magias`: [(rel, linea, nombre, valor)]. Da (quejas, notas)."""
    quejas = []
    for v in sorted(set(censo) & set(NO_SON_LAMINAS)):
        quejas.append("%#x esta en el censo (%s) Y en NO_SON_LAMINAS: es una cosa o la otra" % (v, censo[v]))
    vistas = set()
    for rel, linea, nombre, v in magias:
        if rel == CENSO:
            continue
        vistas.add(v)
        if v in censo or v in NO_SON_LAMINAS:
            continue
        quejas.append("%s:%d `%s = %#x` no es una lamina del censo ni un formato con motivo" % (rel, linea, nombre, v))
    for v, n in sorted(censo.items()):
        if v not in vistas:
            quejas.append("el censo dice %s (%#x) y ningun fichero lo define: fila muerta" % (n, v))
    return quejas, vistas


def autoprueba():
    censo = {MAGIA_BVER: "BVER"}
    nueva = [("a.rs", 3, "MAGIA", MAGIA_BVER), ("b.rs", 9, "MAGIA", 0x4E55_4556)]
    q, _ = juzgar(censo, nueva)
    assert len(q) == 1 and "b.rs:9" in q[0], q
    q, _ = juzgar(censo, [])
    assert len(q) == 1 and "fila muerta" in q[0], q
    assert valor('u32::from_le_bytes(*b"BVER")') == MAGIA_BVER
    assert valor("0x5245_5642") == MAGIA_BVER
    assert valor("bmo::SUP_MAGIC as u32") is None
    texto = 'Clase {\n        magia: 0x5245_5642,\n        nombre: "BVER",'
    assert leer_censo(texto) == {MAGIA_BVER: "BVER"}


def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--check", action="store_true")
    g.add_argument("--list", action="store_true")
    args = ap.parse_args()
    autoprueba()

    r = raiz()
    rs = ficheros_rs()
    if rs is None:
        print("laminas: MUERTO -- git ls-files no contesto")
        return 2
    try:
        censo = leer_censo(open(os.path.join(r, CENSO), encoding="utf-8").read())
    except OSError:
        print("laminas: MUERTO -- falta %s" % CENSO)
        return 2
    magias = []
    for rel in rs:
        try:
            texto = open(os.path.join(r, rel), encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        for m in RE_CONST.finditer(texto):
            v = valor(m.group(2))
            if v is None:
                continue
            linea = texto.count("\n", 0, m.start()) + 1
            magias.append((rel, linea, m.group(1), v))
    if MAGIA_BVER not in {v for _, _, _, v in magias if _ != CENSO}:
        print("laminas: MUERTO -- no ve la magia de BVER: no ha mirado nada")
        return 2
    if not censo:
        print("laminas: MUERTO -- el censo no tiene ni una clase legible")
        return 2

    quejas, vistas = juzgar(censo, magias)

    if args.list:
        for rel, linea, nombre, v in sorted(magias, key=lambda x: x[3]):
            que = ("LAMINA " + censo[v]) if v in censo else ("no: " + NO_SON_LAMINAS.get(v, "?? SIN DECIR"))
            print("%#020x  %-40s %s:%d" % (v, que[:40], rel, linea))
    if quejas:
        print("laminas: %d queja(s)" % len(quejas))
        for q in quejas:
            print("  " + q)
        print("  una magia nueva: si se PRESTA, su clase en %s; si no, su motivo en NO_SON_LAMINAS" % CENSO)
        return 1
    print("clean: %d laminas en el censo, %d magias del arbol, todas con nombre" % (len(censo), len(magias)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
