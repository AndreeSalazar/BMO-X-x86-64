"""atributos.py -- saca de cuda.h la tabla {numero, nombre} de CU_DEVICE_ATTRIBUTE_*.

Uso:  python atributos.py <ruta a cuda.h> > atributos.h

Los nombres son los de NVIDIA (su cabecera); aqui solo se copian el numero y el
nombre, sin el prefijo, para que sonda3060.cu pregunte por TODOS y diga cual es
cual. Si dos nombres comparten numero (alias), vale el primero.
"""

import re
import sys


def main():
    with open(sys.argv[1], encoding="latin-1") as fh:
        texto = fh.read()
    vistos = {}
    for nombre, valor in re.findall(r"^\s+CU_DEVICE_ATTRIBUTE_(\w+)\s*=\s*(\d+)", texto, re.M):
        valor = int(valor)
        if nombre == "MAX" or valor in vistos:
            continue
        vistos[valor] = nombre
    print("static const struct { int id; const char *nombre; } ATRIBUTOS[] = {")
    for valor in sorted(vistos):
        print('    {%d, "%s"},' % (valor, vistos[valor]))
    print("};")


if __name__ == "__main__":
    main()
