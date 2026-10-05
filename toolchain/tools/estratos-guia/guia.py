#!/usr/bin/env python3
"""estratos-guia -- la GUIA de ESTRATOS sale del CONTRATO, no de la memoria.

Por que existe (2026-10-05)
===========================

El propietario: *"en ESTRATOS sean posible para tener guias simples [...]
plantillas, guias y porque motivos todo que existen cuantos existen y se
pueden usar"*. Ver `docs/plan/PLAN_LAS_RAMAS.md`, R1.

Una guia escrita a mano MIENTE pronto: una puerta nueva que no sale, una que
cambio de sentido, un numero de "cuantas hay" que ya no es verdad. Por eso la
guia no tiene texto propio: cada puerta de ESTRATOS ya esta escrita, con su
porque, en el contrato del ABI:

    platform/abi/bmo-abi/src/syscalls/surface/objetos.rs    ES_NODO_*, ES_HIST_*, ES_TXT_*
    platform/abi/bmo-abi/src/syscalls/surface/disco.rs      ES_GESTO_*

y este obrero las copia, tal cual, a una tabla constante del TALLER:

    Ultra_userspace/apps/taller/src/guia_estratos_gen.rs

    su familia   por su prefijo: el cursor, la historia, los nombres, los gestos
    su nombre    lo que va detras del prefijo, en minusculas
    QUE hace     el PRIMER parrafo de su comentario `///`
    POR QUE      el primer parrafo que la casa marca (`*`, `**`, `[!]`), si hay

[!] Una puerta sin comentario propio es una puerta que la guia no puede
explicar: este guardian dice NO y la nombra. Se arregla en el contrato, que
es donde se lee primero.

Modos
=====

    (nada)     escribe guia_estratos_gen.rs
    --check    sale 1 si guia_estratos_gen.rs no es lo que el contrato dice
"""

import argparse
import os
import re
import sys

RAIZ = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))
SUPERFICIE = os.path.join(RAIZ, "platform", "abi", "bmo-abi", "src", "syscalls", "surface")
FUENTES = [os.path.join(SUPERFICIE, "objetos.rs"), os.path.join(SUPERFICIE, "disco.rs")]
SALIDA = os.path.join(RAIZ, "Ultra_userspace", "apps", "taller", "src", "guia_estratos_gen.rs")

# (prefijo, nombre de la familia, que es, escribe?)
FAMILIAS = [
    ("ES_NODO_", "EL CURSOR", "lee: un cursor que baja y sube por el arbol del volumen", False),
    ("ES_HIST_", "LA HISTORIA", "lee: la cadena de versiones, de ahora hacia atras", False),
    ("ES_TXT_", "LOS NOMBRES", "lee: los textos que el cursor y la historia no caben en un numero", False),
    ("ES_GESTO_", "LOS GESTOS", "escribe: cada gesto publica un estrato nuevo; nada se pisa", True),
]
CONST = re.compile(r"pub const (ES_[A-Z_]+): u64 = (0x[0-9A-Fa-f]+|\d+);")
MARCA = re.compile(r"^(\[!\]|\*+|★+)\s*")


def leer(ruta):
    with open(ruta, encoding="utf-8") as f:
        return f.read()


def limpio(t):
    t = re.sub(r"\[`([^`]+)`\]", r"\1", t)
    t = t.replace("`", "").replace("**", "")
    t = MARCA.sub("", t.strip())
    t = t.replace("*", "")
    return re.sub(r"\s+", " ", t).strip()


def parrafos(doc):
    out, cur = [], []
    for d in doc:
        if not d:
            if cur:
                out.append(cur)
            cur = []
        else:
            cur.append(d)
    if cur:
        out.append(cur)
    return out


def puertas():
    lista, sin = [], []
    for ruta in FUENTES:
        lineas = leer(ruta).split("\n")
        for i, l in enumerate(lineas):
            m = CONST.match(l)
            if not m:
                continue
            nombre, valor = m.group(1), int(m.group(2), 0)
            fam = next((k for k, f in enumerate(FAMILIAS) if nombre.startswith(f[0])), None)
            if fam is None:
                continue
            doc, j = [], i - 1
            while j >= 0 and lineas[j].strip().startswith("///"):
                doc.insert(0, lineas[j].strip()[3:].strip())
                j -= 1
            ps = parrafos(doc)
            if not ps:
                sin.append(nombre)
                continue
            que = limpio(" ".join(ps[0]))
            porque = ""
            for p in ps[1:]:
                if MARCA.match(p[0]):
                    porque = limpio(" ".join(p))
                    break
            corto = nombre[len(FAMILIAS[fam][0]):].lower().replace("_", " ")
            lista.append({"fam": fam, "nombre": corto, "puerta": nombre, "valor": valor, "que": que, "porque": porque})
    return lista, sin


def rust(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def escribir(lista):
    o = []
    o.append("//! GENERADO POR `toolchain/tools/estratos-guia/guia.py` DESDE EL CONTRATO DEL ABI")
    o.append("//! (`platform/abi/bmo-abi/src/syscalls/surface/`) -- NO EDITAR A MANO.")
    o.append("//!")
    o.append("//! La GUIA de ESTRATOS en F1 (`docs/plan/PLAN_LAS_RAMAS.md`, R1): cada puerta")
    o.append("//! que existe, con QUE hace y POR QUE, copiadas del comentario que la define.")
    o.append("//! Lo que se edita es el contrato; esto lo sigue.")
    o.append("")
    o.append("/// Una puerta de ESTRATOS, como la cuenta el contrato.")
    o.append("pub struct Door {")
    o.append("    pub family: usize,")
    o.append("    pub name: &'static str,")
    o.append("    pub door: &'static str,")
    o.append("    pub value: u64,")
    o.append("    pub what: &'static str,")
    o.append("    pub why: &'static str,")
    o.append("}")
    o.append("")
    o.append("/// (nombre, que es, escribe?) de cada familia, por su prefijo.")
    o.append("pub static FAMILIES: [(&str, &str, bool); %d] = [" % len(FAMILIAS))
    for _, n, q, w in FAMILIAS:
        o.append("    (%s, %s, %s)," % (rust(n), rust(q), "true" if w else "false"))
    o.append("];")
    o.append("")
    o.append("/// Cuantas puertas hay: la guia lo dice con este numero, no con uno escrito.")
    o.append("pub const COUNT: usize = %d;" % len(lista))
    o.append("")
    o.append("/// `static`: UNA copia en `.rodata`, que Ring 3 lee por indice.")
    o.append("pub static DOORS: [Door; COUNT] = [")
    for p in lista:
        o.append("    Door {")
        o.append("        family: %d," % p["fam"])
        o.append("        name: %s," % rust(p["nombre"]))
        o.append("        door: %s," % rust(p["puerta"]))
        o.append("        value: %d," % p["valor"])
        o.append("        what: %s," % rust(p["que"]))
        o.append("        why: %s," % rust(p["porque"]))
        o.append("    },")
    o.append("];")
    return "\n".join(o) + "\n"


def main():
    ap = argparse.ArgumentParser(description="la guia de ESTRATOS, desde el contrato")
    ap.add_argument("--check", action="store_true")
    a = ap.parse_args()
    for ruta in FUENTES:
        if not os.path.exists(ruta):
            print("guardian MUERTO: no existe %s" % os.path.relpath(ruta, RAIZ))
            return 1
    lista, sin = puertas()
    if sin:
        print("estratos-guia: %d puerta(s) sin comentario propio en el contrato: %s" % (len(sin), ", ".join(sin)))
        print("  se arregla escribiendo su `///` en platform/abi/bmo-abi/src/syscalls/surface/")
        return 1
    texto = escribir(lista)
    try:
        texto.encode("ascii")
    except UnicodeEncodeError:
        print("estratos-guia: el contrato lleva no-ASCII en una puerta; la tabla va en Ring 3 tal cual")
        return 1
    rel = os.path.relpath(SALIDA, RAIZ)
    cuenta = ", ".join("%s %d" % (f[1].lower(), sum(1 for p in lista if p["fam"] == k)) for k, f in enumerate(FAMILIAS))
    if a.check:
        actual = leer(SALIDA) if os.path.exists(SALIDA) else ""
        if actual != texto:
            print("estratos-guia: %s y el contrato NO dicen lo mismo (una puerta entro, salio o cambio)." % rel)
            print("  se arregla con: python toolchain/tools/estratos-guia/guia.py")
            return 1
        print("clean: la guia de ESTRATOS cuenta %d puertas, las mismas que el contrato (%s)" % (len(lista), cuenta))
        return 0
    with open(SALIDA, "w", encoding="utf-8", newline="\n") as f:
        f.write(texto)
    print("escrito %s: %d puertas (%s)" % (rel, len(lista), cuenta))
    return 0


if __name__ == "__main__":
    sys.exit(main())
