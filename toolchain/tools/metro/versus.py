#!/usr/bin/env python3
"""EL VERSUS: TITAN++ de BMO-X contra el C, el C++ y el Rust de siempre.

El propietario (16-10): *"me gustaria saber que pongas en titan++ en
velocidad que traiga en mi bmo-X vs C y Cpp y rust el estandar [...] para que
tu pruebes luego y eso poner como versus"*.

El mismo programa (`duelo/`: primos, fib, bytes, ordenar) en:

    BMO-X    TITAN++ y el C de la casa, contados por el METRO (el emulador de
             `bmo-lower`: `metro --duelo`)
    estandar GCC y Clang a -O2 (C y C++) y rustc -O (`duelo/estandar/`), y
             GCC y Clang A TOPE: -O3 -march=znver3 (la CPU de BMO-X),
             corridos en el anfitrion y contados por valgrind (cachegrind,
             "I refs")

La vara es la MISMA en los dos lados: INSTRUCCIONES de x86-64 ejecutadas, no
ciclos ni segundos. Al estandar se le resta lo que gasta su programa VACIO
(arrancar glibc, iostream o la std de Rust, leer el numero e imprimir), para
contar solo el trabajo; los programas leen el numero de la consola para que
el compilador no pueda resolverlos al compilar.

[!] Lo que esto NO dice: los ciclos del Ryzen (una instruccion de SIMD hace
el trabajo de varias; eso lo mide el metal). Y que el estandar no comprueba
lo que TITAN++ comprueba: C y C++ no miran desbordes ni indices; Rust mira
los indices y no los desbordes (en -O).

Uso:  python toolchain/tools/metro/versus.py      (pide gcc, clang, g++,
      clang++, rustc y valgrind; si falta uno, su columna dice por que)
      python toolchain/tools/metro/versus.py --escribe
                                    y ademas reescribe EL MARCADOR
                                    (`toolchain/tools/metro/MARCADOR.md`):
                                    cada duelo contra cada uno, GANA /
                                    EMPATA / pierde, con su numero

El propietario, 16-10: *"poner eso en datos y documentos que decir que
faltan"*. El marcador no se escribe a mano: lo escribe esta herramienta con
lo que mide, y el diff de cada commit lo enseña.
"""

import os
import re
import shutil
import subprocess
import sys
import tempfile
import datetime

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
ESTANDAR = os.path.join(RAIZ, "toolchain", "tools", "metro", "duelo", "estandar")
DUELOS = [("primos", "20000"), ("fib", "22"), ("bytes", "200000"), ("ordenar", "600")]
# Lo que imprimen los de TITAN++ en el metro (y lo que tiene que imprimir el resto).
ESPERA = {"primos": "2262", "fib": "17711", "bytes": "25499040", "ordenar": "0 999"}

# (columna, herramienta, extension, orden de compilar)
COMPILADORES = [
    ("gcc C", "gcc", "c", ["gcc", "-O2", "-o"]),
    ("clang C", "clang", "c", ["clang", "-O2", "-o"]),
    ("g++ C++", "g++", "cpp", ["g++", "-O2", "-o"]),
    ("clang++ C++", "clang++", "cpp", ["clang++", "-O2", "-o"]),
    ("rustc Rust", "rustc", "rs", ["rustc", "-O", "-o"]),
    # Los veteranos A TOPE: -O3 y la CPU de BMO-X (Zen 3: AVX2, BMI2, FMA).
    ("gcc Zen3", "gcc", "c", ["gcc", "-O3", "-march=znver3", "-o"]),
    ("clang Zen3", "clang", "c", ["clang", "-O3", "-march=znver3", "-o"]),
]


def instrucciones(exe, entra):
    """Las instrucciones que ejecuta `exe` con `entra` en la consola, y lo
    que imprime."""
    r = subprocess.run(
        ["valgrind", "--tool=cachegrind", "--cache-sim=no", "--cachegrind-out-file=/dev/null", exe],
        input=entra + "\n", capture_output=True, text=True, check=True,
    )
    m = re.search(r"I\s+refs:\s+([\d,]+)", r.stderr)
    if not m:
        raise RuntimeError("valgrind no dijo cuantas instrucciones")
    return int(m.group(1).replace(",", "")), r.stdout.strip()


def bmo():
    """Lo que dice el metro de BMO-X: {duelo: (pasos C de la casa, pasos TITAN++)}."""
    r = subprocess.run(["cargo", "run", "-q", "--release", "-p", "bmo-metro", "--", "--duelo"],
                       cwd=RAIZ, capture_output=True, text=True, check=True)
    v = {}
    for linea in r.stdout.splitlines():
        p = linea.split()
        if len(p) == 4 and p[0] in dict(DUELOS):
            v[p[0]] = (int(p[2]), int(p[1]))
    return v


def main():
    casa = bmo()
    estandar = {}
    with tempfile.TemporaryDirectory() as tmp:
        for col, herramienta, ext, orden in COMPILADORES:
            if not shutil.which(herramienta) or not shutil.which("valgrind"):
                estandar[col] = "falta " + (herramienta if not shutil.which(herramienta) else "valgrind")
                continue
            vacio = os.path.join(tmp, f"vacio_{ext}_{herramienta}")
            subprocess.run(orden + [vacio, os.path.join(ESTANDAR, f"vacio.{ext}")], check=True)
            base, _ = instrucciones(vacio, "1")
            fila = {}
            for d, entra in DUELOS:
                exe = os.path.join(tmp, f"{d}_{ext}_{herramienta}")
                subprocess.run(orden + [exe, os.path.join(ESTANDAR, f"{d}.{ext}")], check=True)
                n, salida = instrucciones(exe, entra)
                fila[d] = (max(n - base, 1), salida)
            estandar[col] = fila

    # Lo que imprime cada uno tiene que ser lo de TITAN++: si no, no es el
    # mismo programa y la cuenta no vale.
    malos = []
    for col, *_ in COMPILADORES:
        f = estandar[col]
        if not isinstance(f, str):
            for d, _ in DUELOS:
                if f[d][1] != ESPERA[d]:
                    malos.append(f"{col} {d}: imprime {f[d][1]!r}, TITAN++ {ESPERA[d]!r}")
    if malos:
        print("[!] no es el mismo programa:\n  " + "\n  ".join(malos))
        return 1

    cols = ["TITAN++", "C casa"] + [c[0] for c in COMPILADORES]
    print("EL VERSUS -- instrucciones de x86-64 del TRABAJO (sin el arranque)")
    print()
    print(f"{'duelo':<9}" + "".join(f"{c:>13}" for c in cols))
    for d, _ in DUELOS:
        t, c = casa[d]
        celdas = [f"{t:>13,}", f"{c:>13,}"]
        for col, *_ in COMPILADORES:
            f = estandar[col]
            celdas.append(f"{'-':>13}" if isinstance(f, str) else f"{f[d][0]:>13,}")
        print(f"{d:<9}" + "".join(celdas))
    print()
    print("cuantas veces mas instrucciones hace TITAN++ (1,0 = empate)")
    print()
    print(f"{'duelo':<9}" + "".join(f"{c:>13}" for c in cols[1:]))
    for d, _ in DUELOS:
        t, c = casa[d]
        celdas = [f"{t / c:>12.1f}x"]
        for col, *_ in COMPILADORES:
            f = estandar[col]
            celdas.append(f"{'-':>13}" if isinstance(f, str) else f"{t / f[d][0]:>12.1f}x")
        print(f"{d:<9}" + "".join(celdas))
    for col, *_ in COMPILADORES:
        if isinstance(estandar[col], str):
            print(f"\n[!] {col}: {estandar[col]}")
    if "--escribe" in sys.argv:
        escribe_marcador(casa, estandar)
    return 0


def veredicto(r):
    """Lo que dice un cociente (instrucciones de TITAN++ / las del otro)."""
    if r < 0.95:
        return f"GANA {r:.2f}"
    if r <= 1.05:
        return f"EMPATA {r:.2f}"
    return f"pierde {r:.2f}"


def escribe_marcador(casa, estandar):
    """EL MARCADOR: cada duelo contra cada uno, y la cuenta."""
    rivales = ["C casa"] + [c[0] for c in COMPILADORES]
    filas = []
    cuenta = {"GANA": 0, "EMPATA": 0, "pierde": 0}
    for d, _ in DUELOS:
        t, c = casa[d]
        celdas = [veredicto(t / c)]
        for col, *_ in COMPILADORES:
            f = estandar[col]
            celdas.append("-" if isinstance(f, str) else veredicto(t / f[d][0]))
        for x in celdas:
            for k in cuenta:
                if x.startswith(k):
                    cuenta[k] += 1
        filas.append((d, t, celdas))
    ancho = max(len(x) for _, _, cs in filas for x in cs + rivales) + 2
    out = []
    out.append("# EL MARCADOR -- TITAN++ (BMO-X) contra C, C++ y Rust")
    out.append("")
    out.append("Lo escribe `python toolchain/tools/metro/versus.py --escribe`; no se toca")
    out.append("a mano. La vara: INSTRUCCIONES de x86-64 del trabajo (el metro de BMO-X")
    out.append("para TITAN++ y el C de la casa; valgrind para el resto, sin su arranque).")
    out.append("El numero es TITAN++ / el otro: por debajo de 1, TITAN++ hace MENOS.")
    out.append("GANA por debajo de 0,95; EMPATA hasta 1,05; pierde por encima.")
    out.append("")
    out.append(f"Medido: {datetime.date.today().isoformat()}.")
    out.append("")
    out.append("```text")
    out.append(f"{'duelo':<9}{'TITAN++':>12}  " + "".join(f"{r:<{ancho}}" for r in rivales))
    for d, t, celdas in filas:
        out.append(f"{d:<9}{t:>12,}  " + "".join(f"{x:<{ancho}}" for x in celdas))
    out.append("```")
    out.append("")
    total = sum(cuenta.values())
    out.append(f"**La cuenta: {cuenta['GANA']} gana, {cuenta['EMPATA']} empata y {cuenta['pierde']} pierde, de {total}.**")
    out.append("")
    out.append("[!] Lo que esto NO dice: los ciclos del Ryzen (una instruccion de SIMD")
    out.append("hace el trabajo de varias, y una division cuesta unas 40 veces una suma).")
    out.append("Eso lo mide el metal (AF10). Y TITAN++ mira desbordes e indices; C y C++")
    out.append("no miran nada, Rust mira los indices.")
    out.append("")
    out.append("Lo que FALTA para ganar donde pierde esta en")
    out.append("`docs/plan/PLAN_TITAN_AFINADO.md`, seccion 2d (EL MARCADOR).")
    ruta = os.path.join(RAIZ, "toolchain", "tools", "metro", "MARCADOR.md")
    with open(ruta, "w", encoding="ascii") as f:
        f.write("\n".join(out) + "\n")
    print(f"\nmarcador escrito: {os.path.relpath(ruta, RAIZ)} -- {cuenta['GANA']} gana, {cuenta['EMPATA']} empata, {cuenta['pierde']} pierde")


if __name__ == "__main__":
    sys.exit(main())
