"""proton-x -- el guardian de PROTON-X: que lo que se prueba sea lo que se da.

== De donde sale, y con fecha ==

El 27/28-09 PROTON-X crecio de 8 a 18 `.exe` de prueba y de 5 a 17 modulos
en la casa en dos dias. Tres cosas se rompieron sin que ningun guardian lo
viera a tiempo:

  1. dos codigos de error se hicieron `pub(crate)` en la casa y el contrato
     (R23) paro el despliegue del propietario: el numero 2 significaba dos
     cosas;
  2. cada `.exe` nuevo tenia que entrar en TRES sitios a mano -- su huella en
     `HACER.txt`, su copia al volumen en `ejemplos.ps1` y su prueba en el
     banco -- y nada comprobaba que estuviera en los tres;
  3. la tabla de kernel32 se reparte entre diez modulos (`kernel32`, `hilos`,
     `ficheros`...) que se consultan EN ORDEN: un nombre servido por dos se
     queda con el primero y el segundo es codigo muerto que nadie ve.

== Las reglas, y lo que cada una dice que NO ==

  PX1  huellas    un `.exe` de `prueba/` cuya sha256 no esta en HACER.txt
                  (se rehizo y no se apunto, o se apunto otro)
  PX2  volumen    un `.exe` de `prueba/` que `ejemplos.ps1` no copia a
                  `window/` (no llega al Ryzen)
  PX3  banco      un `.exe` de `prueba/` que ninguna prueba del anfitrion
                  corre (`include_bytes!` en `proton-x-casa/tests/`)
  PX4  sin copia  una cabecera de licencia ajena (Wine, DXVK, vkd3d, LGPL,
                  SPDX) en el codigo de PROTON-X: la regla del propietario es
                  que no se copia su codigo
  PX5  un nombre, una puerta   un nombre de Windows servido por DOS modulos
                  de la misma cadena de kernel32 (el segundo no se usa nunca)
  PX6  sin numero publico      un `pub const ERROR_*` en la casa: los codigos
                  de Windows son privados de quien los da (lo que pide R23 del
                  contrato; esto lo dice ANTES y con el sitio)
  PX7  lo que se da, se nombra un `extern "win64" fn` de la casa que ninguna
                  tabla (`dir!`) da ni nadie llama: una funcion de Windows
                  escrita y nunca conectada

== Uso ==

    python proton_x.py --check        las siete reglas sobre el arbol
    python proton_x.py --autoprueba   cada regla, con un caso roto, dice NO

Sale con 0 si todo cuadra; con 1 y la lista de lo que no, si no.
"""

import hashlib
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
PRUEBA = os.path.join(RAIZ, "platform", "shared", "proton-x", "prueba")
CASA = os.path.join(RAIZ, "platform", "shared", "proton-x-casa")
PURO = os.path.join(RAIZ, "platform", "shared", "proton-x")
EJEMPLOS = os.path.join(RAIZ, "Ultra_kernel_x86-64", "build", "ejemplos.ps1")


# -- Las reglas: cada una recibe TEXTO y devuelve lo que rechaza -----------------

def px1_huellas(exes, hacer):
    """exes: {nombre: bytes}. hacer: el texto de HACER.txt."""
    huellas = set(re.findall(r"\b[0-9a-f]{64}\b", hacer))
    return ["PX1 %s: su sha256 (%s...) no esta en HACER.txt" % (n, hashlib.sha256(b).hexdigest()[:12])
            for n, b in sorted(exes.items()) if hashlib.sha256(b).hexdigest() not in huellas]


def px2_volumen(nombres, ejemplos):
    m = re.search(r"foreach \(\$exe in @\(([^)]*)\)\)", ejemplos)
    copiados = set(re.findall(r"'([^']+\.exe)'", m.group(1))) if m else set()
    return ["PX2 %s: ejemplos.ps1 no lo copia a window/" % n for n in sorted(nombres) if n not in copiados]


def px3_banco(nombres, pruebas):
    corridos = set(re.findall(r'include_bytes!\("[^"]*prueba/([^"/]+\.exe)"\)', pruebas))
    return ["PX3 %s: ninguna prueba del anfitrion lo corre" % n for n in sorted(nombres) if n not in corridos]


LICENCIA = re.compile(r"(?i)(copyright[^\n]{0,80}(wine|codeweavers|dxvk|vkd3d|valve|philip rebohle))|\bLGPL\b|SPDX-License-Identifier")


def px4_sin_copia(fuentes):
    """fuentes: {ruta: texto}."""
    malos = []
    for r, t in sorted(fuentes.items()):
        m = LICENCIA.search(t)
        if m:
            linea = t.count("\n", 0, m.start()) + 1
            malos.append("PX4 %s:%d: una cabecera de licencia ajena (%r): aqui no se copia codigo de nadie" % (r, linea, m.group(0)[:40]))
    return malos


def nombres_de_buscar(texto):
    """Los nombres de Windows de las `fn buscar*(...)` de un modulo."""
    nombres = set()
    for cuerpo in re.findall(r"fn buscar\w*\([^)]*\)[^{]*\{(.*?)\n\}", texto, re.S):
        for brazo in re.findall(r'((?:"[^"]+"\s*\|\s*)*"[^"]+")\s*=>', cuerpo):
            nombres.update(re.findall(r'"([^"]+)"', brazo))
    return nombres


def cadena_de_kernel32(lib):
    """Los modulos que `tabla` consulta para kernel32, en su orden."""
    m = re.search(r"kernel32::buscar\(n\)((?:\.or_else\(\|\| \w+::buscar\(n\)\))*)", lib)
    if not m:
        return []
    return ["kernel32"] + re.findall(r"(\w+)::buscar\(n\)", m.group(1))


def px5_un_nombre(modulos, orden):
    """modulos: {modulo: texto}; orden: la cadena de kernel32."""
    visto = {}
    malos = []
    for mod in orden:
        for n in sorted(nombres_de_buscar(modulos.get(mod, ""))):
            if n in visto:
                malos.append("PX5 %s: lo dan %s.rs y %s.rs; la tabla se queda con %s.rs y lo de %s.rs no se usa nunca" % (n, visto[n], mod, visto[n], mod))
            else:
                visto[n] = mod
    return malos


def px6_sin_numero_publico(modulos):
    malos = []
    for mod, t in sorted(modulos.items()):
        for m in re.finditer(r"^\s*pub(?:\([^)]*\))?\s+const\s+(ERROR_\w+)", t, re.M):
            linea = t.count("\n", 0, m.start()) + 1
            malos.append("PX6 %s.rs:%d: %s es publico; los codigos de Windows son privados de quien los da (R23 del contrato)" % (mod, linea, m.group(1)))
    return malos


def px7_lo_que_se_da(modulos):
    todo = "\n".join(modulos.values())
    malos = []
    for mod, t in sorted(modulos.items()):
        for m in re.finditer(r'extern "win64" fn (\w+)', t):
            f = m.group(1)
            # dir!(f), dir!(modulo::f), o f::<...> (una generica en una vtabla).
            dado = re.search(r"dir!\(\s*(?:\w+::)*%s\s*\)|\b%s::<" % (re.escape(f), re.escape(f)), todo)
            llamado = len(re.findall(r"\b%s\s*\(" % re.escape(f), todo)) > 1
            if not dado and not llamado:
                malos.append("PX7 %s.rs: `%s` es una funcion de Windows que ninguna tabla da ni nadie llama" % (mod, f))
    return malos


# -- El arbol ---------------------------------------------------------------------

def leer(r):
    with open(r, encoding="utf-8") as f:
        return f.read()


def check():
    exes = {n: open(os.path.join(PRUEBA, n), "rb").read() for n in os.listdir(PRUEBA) if n.endswith(".exe")}
    hacer = leer(os.path.join(PRUEBA, "HACER.txt"))
    ejemplos = leer(EJEMPLOS)
    tests = os.path.join(CASA, "tests")
    pruebas = "\n".join(leer(os.path.join(tests, n)) for n in os.listdir(tests) if n.endswith(".rs"))
    src = os.path.join(CASA, "src")
    modulos = {n[:-3]: leer(os.path.join(src, n)) for n in os.listdir(src) if n.endswith(".rs")}
    fuentes = {}
    for base in (os.path.join(PURO, "src"), src, PRUEBA):
        for dp, _, fs in os.walk(base):
            for n in fs:
                if n.endswith((".rs", ".c", ".hlsl")):
                    r = os.path.join(dp, n)
                    fuentes[os.path.relpath(r, RAIZ)] = leer(r)
    malos = []
    malos += px1_huellas(exes, hacer)
    malos += px2_volumen(exes.keys(), ejemplos)
    malos += px3_banco(exes.keys(), pruebas)
    malos += px4_sin_copia(fuentes)
    orden = cadena_de_kernel32(modulos.get("lib", ""))
    if not orden:
        malos.append("PX5 lib.rs: no encuentro la cadena de kernel32 en `tabla` (el guardian ya no sabe leerla)")
    malos += px5_un_nombre(modulos, orden)
    malos += px6_sin_numero_publico(modulos)
    malos += px7_lo_que_se_da({k: v for k, v in modulos.items() if k != "lib"})
    if malos:
        for m in malos:
            print("  [X] " + m)
        print("proton-x: %d incumplimiento(s)" % len(malos))
        return 1
    print("clean: PROTON-X cuadra -- %d .exe con su huella, en el volumen y en el banco; %d modulos en la cadena de kernel32 sin nombres repetidos; %d fuentes sin licencias ajenas"
          % (len(exes), len(orden), len(fuentes)))
    return 0


# -- La autoprueba: cada regla, con un caso roto, dice NO -------------------------

def autoprueba():
    casos = [
        ("PX1", px1_huellas({"a.exe": b"x"}, "sha256: " + "0" * 64)),
        ("PX2", px2_volumen(["b.exe"], "foreach ($exe in @('a.exe')) {")),
        ("PX3", px3_banco(["b.exe"], 'include_bytes!("../../proton-x/prueba/a.exe")')),
        ("PX4", px4_sin_copia({"x.rs": "// Copyright 2018 Philip Rebohle (dxvk)\n"})),
        ("PX4", px4_sin_copia({"x.rs": "/* SPDX-License-Identifier: LGPL-2.1 */"})),
        ("PX5", px5_un_nombre({"a": 'pub(crate) fn buscar(n: &str) -> Option<u64> {\n    Some(match n {\n        "Sleep" => dir!(s),\n    })\n}',
                                "b": 'pub(crate) fn buscar(n: &str) -> Option<u64> {\n    Some(match n {\n        "X" | "Sleep" => dir!(t),\n    })\n}'}, ["a", "b"])),
        ("PX6", px6_sin_numero_publico({"f": "pub(crate) const ERROR_FILE_NOT_FOUND: u32 = 2;"})),
        ("PX7", px7_lo_que_se_da({"f": 'extern "win64" fn olvidada(a: u64) -> u64 { a }'})),
    ]
    # Y lo bueno NO se rechaza: una regla que dice que no a todo tampoco protege.
    buenos = [
        ("PX1", px1_huellas({"a.exe": b"x"}, hashlib.sha256(b"x").hexdigest())),
        ("PX4", px4_sin_copia({"x.rs": "// Asi la describen Wine, vkd3d y DXVK (dxbc_checksum)."})),
        ("PX6", px6_sin_numero_publico({"f": "const ERROR_FILE_NOT_FOUND: u32 = 2;"})),
        ("PX7", px7_lo_que_se_da({"f": 'extern "win64" fn dada() {}\n"Dada" => dir!(dada),'})),
        ("PX7", px7_lo_que_se_da({"f": 'extern "win64" fn dada() {}', "g": "(16, dir!(f::dada)),"})),
        ("PX7", px7_lo_que_se_da({"f": 'extern "win64" fn falta<const I: usize>() {}\nfalta::<3> as usize'})),
    ]
    fallos = [r for r, v in casos if not v] + ["%s (rechazo algo bueno)" % r for r, v in buenos if v]
    if fallos:
        print("proton-x: estas reglas no saben decir que NO (o dicen que NO a todo): " + ", ".join(fallos))
        return 1
    print("clean: las 7 reglas de PROTON-X saben decir que NO (%d casos rotos) y que SI (%d buenos)" % (len(casos), len(buenos)))
    return 0


if __name__ == "__main__":
    if sys.argv[1:] == ["--check"]:
        sys.exit(check())
    if sys.argv[1:] == ["--autoprueba"]:
        sys.exit(autoprueba())
    print(__doc__)
    sys.exit(2)
