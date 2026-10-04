#!/usr/bin/env python3
"""titan-leyes -- LAS LEYES DE TITAN++: que no se alteren, y por que.

Por que existe (2026-10-04)
===========================

El propietario, con los niveles 0 a 10 hechos: *"tener guardian estricto el
TITAN++ que no se altere, con reglas y porque"*.

Las reglas de TITAN++ ya las hacian cumplir tests y programas del banco, pero
REPARTIDAS: un test en `words.rs`, otro en `message.rs`, un ejemplo por cada
NO. Nadie veia la lista entera, y una regla podia irse de paso -- se borra el
ultimo ejemplo de un codigo, se marca un test `#[ignore]` porque estorba, se
cambia una fila de la gramatica -- sin que nada avisara. Un lenguaje que
cambia de paso es exactamente lo que TITAN++ prometio no ser (C++).

Por eso `LEYES.txt`: cada ley en una linea, su PORQUE, y QUIEN la hace cumplir
hoy. Y este guardian, que en cada build comprueba:

    1. CADA LEY SIGUE CUMPLIDA. Su `cumple` existe: el test esta en su fichero
       y no esta ignorado, o hay un programa del banco que provoca su NO.
    2. NINGUNA LEY CAMBIA A ESCONDIDAS. El sello es la huella de todas; si el
       texto cambia, el build para, y moverlo es `--sellar "el motivo"`, que
       deja la fecha y el motivo en la HISTORIA del fichero.
    3. EL CODIGO, LA GRAMATICA Y EL BANCO DICEN LO MISMO. Cada codigo de
       `message.rs` tiene su fila en GRAMATICA.md y un programa que lo
       provoca, y GRAMATICA no nombra codigos que no existen. Cada carpeta
       `ejemplos/nivelN` esta en los DOS bancos: una que no estuviera seria
       un nivel que nadie corre.

[!] Lo que NO hace: correr los tests. Eso es `cargo test`, y el build lo hace;
esto mira que las piezas que lo protegen sigan ahi, en milisegundos y sin
compilar nada.

Modos
=====

    (nada)              comprueba; sale 1 si algo falla
    --check             lo mismo (lo que corre el build)
    --sellar "motivo"   las leyes cambiaron por decision del propietario:
                        mueve el sello y apunta el motivo en la HISTORIA
"""

import argparse
import datetime
import hashlib
import os
import re
import sys

RAIZ = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
AQUI = os.path.dirname(os.path.abspath(__file__))
LEYES = os.path.join(AQUI, "LEYES.txt")
TITAN = os.path.join(RAIZ, "toolchain", "lang", "titan")
MENSAJES = os.path.join(TITAN, "src", "message.rs")
GRAMATICA = os.path.join(TITAN, "GRAMATICA.md")
EJEMPLOS = os.path.join(TITAN, "ejemplos")
BANCOS = [os.path.join(TITAN, "tests", "banco.rs"), os.path.join(TITAN, "emisor-x86_64", "tests", "banco.rs")]

LEY = re.compile(r"^(L\d\d)\s+(.+)$")


def leer(ruta):
    with open(ruta, encoding="utf-8") as f:
        return f.read()


def secciones(texto):
    """Las tres secciones del fichero: sus lineas, sin comentarios."""
    out, actual = {}, None
    for linea in texto.splitlines():
        if linea.startswith("#"):
            continue
        m = re.match(r"^\[(\w+)\]\s*$", linea)
        if m:
            actual = m.group(1)
            out[actual] = []
            continue
        if actual:
            out[actual].append(linea)
    return out


def leyes(lineas):
    """[(id, ley, porque, cumple)] en orden."""
    out = []
    for linea in lineas:
        m = LEY.match(linea)
        if m:
            out.append([m.group(1), m.group(2).strip(), "", ""])
            continue
        s = linea.strip()
        if out and s.startswith("porque "):
            out[-1][2] = s[len("porque "):].strip()
        elif out and s.startswith("cumple "):
            out[-1][3] = s[len("cumple "):].strip()
    return [tuple(x) for x in out]


def huella(lista):
    texto = "\n".join("|".join(l) for l in lista)
    return hashlib.sha256(texto.encode("utf-8")).hexdigest()


def esperas():
    """{codigo: [ejemplo, ...]}: lo que dice la primera linea de cada programa
    del banco (un fichero, o el src/main.titan de un paquete)."""
    out = {}
    for nivel in sorted(os.listdir(EJEMPLOS)):
        dnivel = os.path.join(EJEMPLOS, nivel)
        if not os.path.isdir(dnivel):
            continue
        for nombre in sorted(os.listdir(dnivel)):
            ruta = os.path.join(dnivel, nombre)
            if os.path.isdir(ruta):
                ruta = os.path.join(ruta, "src", "main.titan")
            elif not nombre.endswith(".titan"):
                continue
            if not os.path.exists(ruta):
                continue
            primera = leer(ruta).splitlines()[:1]
            if primera and primera[0].startswith("# espera: "):
                out.setdefault(primera[0][len("# espera: "):].strip(), []).append(nivel + "/" + nombre)
    return out


def codigos():
    """Los codigos que el compilador conoce: `Code::X => NN` de message.rs."""
    return sorted({"T%04d" % int(n) for n in re.findall(r"Code::\w+\s*=>\s*(\d+),", leer(MENSAJES))})


def test_vivo(ruta, funcion):
    """None si el test existe y no esta ignorado; si no, por que."""
    completa = os.path.join(RAIZ, ruta)
    if not os.path.exists(completa):
        return "no existe el fichero %s" % ruta
    lineas = leer(completa).splitlines()
    for i, l in enumerate(lineas):
        if re.search(r"\bfn %s\s*\(" % re.escape(funcion), l):
            antes = " ".join(lineas[max(0, i - 3):i])
            if "#[ignore" in antes:
                return "el test `%s` esta marcado #[ignore]: una ley que no se prueba no se cumple" % funcion
            return None
    return "no hay `fn %s` en %s" % (funcion, ruta)


def comprobar(lista, sello, historia_vacia):
    fallos = []
    vistos = set()
    ejemplos = esperas()
    conocidos = codigos()
    # 1. cada ley, entera y cumplida
    for (lid, ley, porque, cumple) in lista:
        if lid in vistos:
            fallos.append("%s esta dos veces" % lid)
        vistos.add(lid)
        if not porque:
            fallos.append("%s no dice su PORQUE: una regla sin motivo es una costumbre" % lid)
        partes = cumple.split()
        if not partes:
            fallos.append("%s no dice quien la hace cumplir" % lid)
        elif partes[0] == "test" and len(partes) == 3:
            por = test_vivo(partes[1], partes[2])
            if por:
                fallos.append("%s (%s): %s" % (lid, ley, por))
        elif partes[0] == "ejemplo" and len(partes) == 2:
            cod = partes[1]
            if cod not in conocidos:
                fallos.append("%s: el codigo %s no existe en message.rs" % (lid, cod))
            elif cod not in ejemplos:
                fallos.append("%s (%s): ya no hay ningun programa del banco que provoque %s" % (lid, ley, cod))
        else:
            fallos.append("%s: `cumple %s` no se entiende (test RUTA FUNCION, o ejemplo T00NN)" % (lid, cumple))
    # 2. el sello
    actual = huella(lista)
    if sello != actual:
        if sello == "pendiente":
            fallos.append("las leyes no tienen sello todavia: --sellar \"el motivo\"")
        else:
            fallos.append("UNA LEY CAMBIO y el sello no: el texto de las leyes lo decide el propietario. Si fue el, `titan_leyes.py --sellar \"el motivo\"` (deja el motivo en la HISTORIA); si no, deshaz el cambio")
    # 3. el codigo, la gramatica y el banco
    gram = leer(GRAMATICA)
    filas = set(re.findall(r"^\| (T\d{4}) \|", gram, re.M))
    for c in conocidos:
        if c not in filas:
            fallos.append("%s existe en message.rs y GRAMATICA.md no tiene su fila" % c)
        if c not in ejemplos:
            fallos.append("%s existe y ningun programa del banco lo provoca: un NO que nadie ha visto" % c)
    for c in sorted(filas - set(conocidos)):
        fallos.append("GRAMATICA.md tiene una fila de %s, y el compilador no conoce ese codigo" % c)
    niveles = sorted(d for d in os.listdir(EJEMPLOS) if os.path.isdir(os.path.join(EJEMPLOS, d)) and re.match(r"nivel\d+$", d))
    for banco in BANCOS:
        texto = leer(banco)
        for n in niveles:
            if '"%s"' % n not in texto:
                fallos.append("ejemplos/%s no esta en %s: un nivel que nadie corre" % (n, os.path.relpath(banco, RAIZ)))
    return fallos, actual, len(conocidos), len(niveles)


def main():
    ap = argparse.ArgumentParser(description="las leyes de TITAN++")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--sellar", metavar="MOTIVO")
    a = ap.parse_args()
    for ruta in [LEYES, MENSAJES, GRAMATICA, EJEMPLOS] + BANCOS:
        if not os.path.exists(ruta):
            print("guardian MUERTO: no existe %s" % os.path.relpath(ruta, RAIZ))
            return 1
    texto = leer(LEYES)
    sec = secciones(texto)
    lista = leyes(sec.get("LEYES", []))
    if not lista:
        print("guardian MUERTO: LEYES.txt no tiene ninguna ley")
        return 1
    sello = next((l.strip() for l in sec.get("SELLO", []) if l.strip()), "pendiente")
    if a.sellar is not None:
        motivo = a.sellar.strip()
        if not motivo:
            print("--sellar pide el MOTIVO: una ley se cambia a la vista")
            return 2
        nuevo = huella(lista)
        hoy = datetime.date.today().isoformat()
        antes, _, resto = texto.partition("[SELLO]\n")
        _, _, historia = resto.partition("[HISTORIA]\n")
        historia = historia.rstrip("\n")
        linea = "%s  %s  %d leyes  %s" % (hoy, nuevo[:12], len(lista), motivo)
        texto = antes + "[SELLO]\n" + nuevo + "\n\n[HISTORIA]\n" + (historia + "\n" if historia else "") + linea + "\n"
        with open(LEYES, "w", encoding="utf-8") as f:
            f.write(texto)
        print("leyes selladas: %d, huella %s" % (len(lista), nuevo[:12]))
        return 0
    fallos, _, ncod, nniv = comprobar(lista, sello, not sec.get("HISTORIA"))
    if fallos:
        print("las leyes de TITAN++ NO se cumplen:")
        for f in fallos:
            print("  [x] " + f)
        return 1
    print("clean: las %d leyes de TITAN++ siguen cumplidas y selladas; %d codigos con su fila y su programa; %d niveles en los dos bancos" % (len(lista), ncod, nniv))
    return 0


if __name__ == "__main__":
    sys.exit(main())
