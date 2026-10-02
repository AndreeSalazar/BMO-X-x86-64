#!/usr/bin/env python3
# espejo-compara -- pone ESPEJO.TXT (Windows) y DIARIO.TXT (BMO-X) lado a lado
# y dice LA PRIMERA DIFERENCIA, que es la proxima pared de PROTON-X vista antes
# de que el juego se caiga. Ver docs/plan/PLAN_EL_ESPEJO.md.
#
# Los dos ficheros tienen el mismo formato (proton-x-casa/src/diario.rs):
#   # cabecera (dos lineas que empiezan por #)
#   orden hilo dll funcion            una por funcion, la PRIMERA vez
#   vigilada(args) = ...              (paso 2; se comparan por separado)
#
# Una pantalla, no un volcado:
#   - la primera funcion que Windows ve y la casa NO (la pared)
#   - las que la casa llama y Windows no (de mas: raro, pero se dice)
#   - un resumen de cuantas coinciden
#
# `--check` corre sobre los dos ficheros de ejemplo de esta carpeta y comprueba
# que el veredicto es el esperado, para el build.

import sys
import os

AQUI = os.path.dirname(os.path.abspath(__file__))


def funciones(ruta):
    """Lista de (dll, funcion) en orden, saltando cabecera, vigiladas y
    lineas vacias. La clave de comparacion es (dll.lower(), funcion)."""
    fuera = []
    try:
        with open(ruta, "r", encoding="utf-8", errors="replace") as f:
            lineas = f.read().splitlines()
    except OSError as e:
        print("espejo-compara: no se pudo leer %s: %s" % (ruta, e))
        sys.exit(2)
    for ln in lineas:
        s = ln.strip()
        if not s or s.startswith("#"):
            continue
        if "(" in s and ")" in s and "=" in s:
            continue  # una vigilada (paso 2), no una funcion a secas
        partes = s.split()
        if len(partes) < 4:
            continue
        # orden hilo dll funcion
        dll = partes[2]
        fun = partes[3]
        fuera.append((dll.lower(), fun))
    return fuera


def comparar(ruta_espejo, ruta_diario):
    esp = funciones(ruta_espejo)
    dia = funciones(ruta_diario)
    esp_set = set(esp)
    dia_set = set(dia)

    print("== espejo-compara ==")
    print("  Windows (espejo): %d funciones" % len(esp))
    print("  BMO-X   (diario): %d funciones" % len(dia))

    # La pared: la PRIMERA (en el orden de Windows) que la casa no da.
    pared = None
    for d, f in esp:
        if (d, f) not in dia_set:
            pared = (d, f)
            break

    if pared:
        print("")
        print("  LA PROXIMA PARED (Windows la llama, la casa no la da todavia):")
        print("    %s %s" % (pared[0], pared[1]))
    else:
        print("")
        print("  sin pared: la casa da TODAS las que Windows llama")

    # De mas: las que la casa llama y Windows no (raro, se dice contado).
    demas = [(d, f) for (d, f) in dia if (d, f) not in esp_set]
    if demas:
        print("")
        print("  la casa llama %d que Windows no (puede ser de otro camino):" % len(demas))
        for d, f in demas[:8]:
            print("    %s %s" % (d, f))
        if len(demas) > 8:
            print("    ... y %d mas" % (len(demas) - 8))

    comunes = len(esp_set & dia_set)
    print("")
    print("  coinciden %d de %d" % (comunes, len(esp_set)))
    return pared


def autochequeo():
    esp = os.path.join(AQUI, "ejemplo_espejo.txt")
    dia = os.path.join(AQUI, "ejemplo_diario.txt")
    if not (os.path.exists(esp) and os.path.exists(dia)):
        print("espejo-compara --check: faltan los ejemplos")
        return 1
    pared = comparar(esp, dia)
    # El ejemplo esta hecho para que la pared sea QueryPerformanceCounter.
    esperado = ("kernel32.dll", "QueryPerformanceCounter")
    if pared != esperado:
        print("")
        print("clean: FALLO -- la pared esperada era %s, salio %s" % (esperado, pared))
        return 1
    print("")
    print("clean: espejo-compara da la pared esperada (%s)" % (esperado[1],))
    return 0


def main():
    args = sys.argv[1:]
    if "--check" in args:
        sys.exit(autochequeo())
    if len(args) != 2:
        print("uso: espejo.py ESPEJO.TXT DIARIO.TXT   (o --check)")
        sys.exit(2)
    comparar(args[0], args[1])


if __name__ == "__main__":
    main()
