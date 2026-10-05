#!/usr/bin/env python3
"""maestros -- los NODOS MAESTROS de la TAB de F1 salen del BANCO de TITAN++.

Por que existe (2026-10-05)
===========================

El propietario: *"me dejas las TAB elegante, en ellas el estilo de Houdini con
Blender que muestran opciones totales de NODOS maestro ya hechos como ejemplos
y porque, como tutoriales y pruebas"*. Ver `docs/plan/PLAN_TALLER.md` 8.15.

Un catalogo escrito a mano MIENTE pronto: un ejemplo que ya no compila, un
porque que ya no es verdad, un nivel nuevo que no sale. Por eso la TAB no
tiene catalogo propio: cada programa `BIEN` de `toolchain/lang/titan/ejemplos/`
YA es un nodo maestro -- el compilador lo acepta, los dos bancos lo corren y
comparan sus lineas `# sale:` en cada build, y su comentario dice por que
existe. Este obrero los copia, tal cual, a una tabla constante de
`titan-lector`:

    platform/shared/titan-lector/src/maestros_gen.rs

    el nodo        el programa (un fichero, o un paquete de UN solo modulo)
    su familia     su NIVEL, con el nombre de su titulo en GRAMATICA.md
    su porque      el comentario justo antes de `mod main`
    lo que dice    la frase de `mod main "..."`
    lo que trae    las palabras de TITAN++ que usa
    lo que pide    los [permissions] de su Titan.toml (la 3060, la pantalla)
    su prueba      sus lineas `# sale:`

[!] Un paquete de VARIOS modulos (`nivel9/flota`, `nivel10/motor`) no es UN
nodo: no entra. La TAB pone un nodo; un paquete entero es otra cosa.

Y es su propio guardian (`--check`, en el build): la tabla y el banco dicen lo
mismo. Un ejemplo nuevo entra en la TAB solo; uno borrado sale; uno cambiado
cambia. Como `tema_gen.rs`: generado, nunca a mano.

Modos
=====

    (nada)     escribe maestros_gen.rs
    --check    sale 1 si maestros_gen.rs no es lo que el banco dice
"""

import argparse
import os
import re
import sys

RAIZ = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
TITAN = os.path.join(RAIZ, "toolchain", "lang", "titan")
EJEMPLOS = os.path.join(TITAN, "ejemplos")
GRAMATICA = os.path.join(TITAN, "GRAMATICA.md")
SALIDA = os.path.join(RAIZ, "platform", "shared", "titan-lector", "src", "maestros_gen.rs")

# Las palabras de TITAN++ que una ficha muestra, en el orden de la escalera.
PALABRAS = [
    "fn", "let", "mut", "if", "else", "and", "or", "not", "for", "while", "break",
    "continue", "return", "type", "dec", "take", "round", "enum", "match", "mod",
    "use", "pub", "trait", "gpu", "f32",
]

# La clave de `Titan.toml` -> la variante de `bmo_titan_contrato::Permission`.
PERMISOS = {"screen": "Screen", "input": "Input", "sound": "Sound", "gpu": "Gpu", "disk": "Disk", "net": "Net"}


def leer(ruta):
    with open(ruta, encoding="utf-8") as f:
        return f.read()


def familias():
    """{nivel: nombre} de los titulos `## Nivel N -- nombre (` de GRAMATICA.md."""
    out = {}
    for m in re.finditer(r"^## Nivel (\d+) -- ([^(\n]+?)\s*\(", leer(GRAMATICA), re.M):
        out[int(m.group(1))] = m.group(2).strip()
    return out


def permisos(toml):
    """Las claves pedidas (true o una cadena) de [permissions]."""
    out, dentro = [], False
    for linea in toml.splitlines():
        s = linea.strip()
        if s.startswith("["):
            dentro = s == "[permissions]"
            continue
        if dentro and "=" in s and not s.startswith("#"):
            k, v = [x.strip() for x in s.split("=", 1)]
            if v != "false":
                if k not in PERMISOS:
                    raise SystemExit("maestros: permiso desconocido `%s`" % k)
                out.append(k)
    return out


def palabras(fuente):
    """Las palabras de TITAN++ que el cuerpo usa: fuera de comentarios y textos."""
    vistas = set()
    for linea in fuente.splitlines():
        if linea.lstrip().startswith("#") or linea.startswith("mod main "):
            continue  # la cabecera `mod main` la tienen todos: no es lo que trae
        sin_textos = re.sub(r'"(\\.|[^"\\])*"', '""', linea)
        vistas.update(re.findall(r"[a-z_][a-z0-9_]*", sin_textos))
    return [p for p in PALABRAS if p in vistas]


def nodo(nivel, nombre, fuente, toml):
    lineas = fuente.splitlines()
    if not lineas or lineas[0].strip() != "# espera: BIEN":
        return None
    cab = next((i for i, l in enumerate(lineas) if l.startswith("mod main ")), None)
    if cab is None:
        raise SystemExit("maestros: %s/%s es BIEN y no empieza por `mod main`" % (nivel, nombre))
    dice = re.match(r'mod main "([^"]*)"', lineas[cab])
    porque = [l[1:].strip() for l in lineas[:cab] if l.startswith("#") and not re.match(r"# (espera|sale):", l)]
    if not porque or not dice:
        raise SystemExit("maestros: %s/%s no dice su porque (un comentario antes de `mod main`)" % (nivel, nombre))
    return {
        "name": nombre,
        "level": nivel,
        "why": porque[-1],
        "says": dice.group(1),
        "words": palabras(fuente),
        "asks": permisos(toml) if toml else [],
        # sin la linea `# espera`: eso es del banco, no del paquete del propietario
        "source": "\n".join(lineas[1:]) + "\n",
        "out": [l[len("# sale: "):] if l.startswith("# sale: ") else "" for l in lineas[:cab] if l.startswith("# sale:")],
    }


def maestros():
    out, fuera = [], []
    for d in sorted(os.listdir(EJEMPLOS), key=lambda d: int(d[5:]) if re.match(r"nivel\d+$", d) else 99):
        m = re.match(r"nivel(\d+)$", d)
        if not m:
            continue
        nivel = int(m.group(1))
        dnivel = os.path.join(EJEMPLOS, d)
        for nombre in sorted(os.listdir(dnivel)):
            ruta = os.path.join(dnivel, nombre)
            if nombre.endswith(".titan"):
                n = nodo(nivel, nombre[:-6], leer(ruta), None)
            elif os.path.isdir(ruta):
                src = os.path.join(ruta, "src")
                main = os.path.join(src, "main.titan")
                if not os.path.exists(main):
                    continue
                toml = os.path.join(ruta, "Titan.toml")
                n = nodo(nivel, nombre, leer(main), leer(toml) if os.path.exists(toml) else None)
                if n and sum(len(fs) for _, _, fs in os.walk(src)) > 1:
                    fuera.append("%s/%s" % (d, nombre))
                    continue
            else:
                continue
            if n:
                out.append(n)
    return out, fuera


def rust(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


def escribir(lista, fams):
    niveles = max(fams) + 1
    for n in range(niveles):
        if n not in fams:
            raise SystemExit("maestros: GRAMATICA.md no tiene el titulo del nivel %d" % n)
    o = []
    o.append("//! GENERADO POR `toolchain/tools/maestros/maestros.py` DESDE EL BANCO DE TITAN++")
    o.append("//! (`toolchain/lang/titan/ejemplos/`) -- NO EDITAR A MANO.")
    o.append("//!")
    o.append("//! Los NODOS MAESTROS de la TAB de F1 (`docs/plan/PLAN_TALLER.md` 8.15): cada")
    o.append("//! programa `BIEN` del banco, que el compilador acepta y los dos bancos corren")
    o.append("//! en cada build. Lo que se edita es el ejemplo; esto lo sigue.")
    o.append("")
    o.append("use crate::maestros::Master;")
    o.append("use bmo_titan_contrato::{Permission, Permissions};")
    o.append("")
    o.append("/// El nombre de cada familia: el titulo de su nivel en GRAMATICA.md.")
    o.append("pub static FAMILIES: [&str; %d] = [" % niveles)
    for n in range(niveles):
        o.append("    %s," % rust(fams[n]))
    o.append("];")
    o.append("")
    o.append("/// Cuantos hay: la TAB mide sus tablas con esto.")
    o.append("pub const COUNT: usize = %d;" % len(lista))
    o.append("")
    o.append("/// `static`, no `const`: UNA copia en `.rodata`, que Ring 3 lee por indice")
    o.append("/// sin traerla a una pila de 64 KiB.")
    o.append("pub static MASTERS: [Master; COUNT] = [")
    for m in lista:
        asks = "Permissions::NONE" + "".join(".with(Permission::%s)" % PERMISOS[k] for k in m["asks"])
        o.append("    Master {")
        o.append("        name: %s," % rust(m["name"]))
        o.append("        level: %d," % m["level"])
        o.append("        why: %s," % rust(m["why"]))
        o.append("        says: %s," % rust(m["says"]))
        o.append("        words: &[%s]," % ", ".join(rust(w) for w in m["words"]))
        o.append("        asks: %s," % asks)
        o.append("        out: &[%s]," % ", ".join(rust(x) for x in m["out"]))
        o.append("        source: %s," % rust(m["source"]))
        o.append("    },")
    o.append("];")
    return "\n".join(o) + "\n"


def main():
    ap = argparse.ArgumentParser(description="los nodos maestros de la TAB, desde el banco")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    for ruta in [EJEMPLOS, GRAMATICA]:
        if not os.path.exists(ruta):
            print("guardian MUERTO: no existe %s" % os.path.relpath(ruta, RAIZ))
            return 1
    lista, fuera = maestros()
    texto = escribir(lista, familias())
    try:
        texto.encode("ascii")
    except UnicodeEncodeError:
        print("maestros: un ejemplo BIEN lleva no-ASCII; la tabla va en Ring 3 tal cual")
        return 1
    rel = os.path.relpath(SALIDA, RAIZ)
    if a.check:
        actual = leer(SALIDA) if os.path.exists(SALIDA) else ""
        if actual != texto:
            print("maestros: %s y el banco NO dicen lo mismo (un ejemplo entro, salio o cambio)." % rel)
            print("  se arregla con: python toolchain/tools/maestros/maestros.py")
            return 1
        print("clean: %d nodos maestros en la TAB, los mismos que el banco (%d paquetes de varios modulos fuera)" % (len(lista), len(fuera)))
        return 0
    with open(SALIDA, "w", encoding="utf-8", newline="\n") as f:
        f.write(texto)
    print("escrito %s: %d nodos maestros; fuera, por ser de varios modulos: %s" % (rel, len(lista), ", ".join(fuera) or "ninguno"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
