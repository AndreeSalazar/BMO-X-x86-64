#!/usr/bin/env python3
"""fabricar.py -- regenera los `.spv` de `pruebas/` desde su GLSL, con el SDK del anfitrion.

Los `.spv` van en el repo junto a su `.comp`: el banco NO necesita el SDK para
correr (PLAN_EL_SOMBREADOR, 1c). Esto solo hace falta cuando se cambia un GLSL
o se agrega uno.

    py fabricar.py

Nada del SDK se enlaza: `glslc` es una herramienta del anfitrion, como un
editor de texto. Si falta, se dice y se sale con error.
"""
import os
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
PRUEBAS = os.path.join(AQUI, "..", "pruebas")
SDK = os.environ.get("VULKAN_SDK", r"C:\VulkanSDK\1.4.350.0")
GLSLC = os.path.join(SDK, "Bin", "glslc.exe" if os.name == "nt" else "glslc")


def main():
    if not os.path.exists(GLSLC):
        sys.exit("fabricar.py: no esta glslc (%s). Los .spv del repo siguen valiendo." % GLSLC)
    hechos = 0
    for nombre in sorted(os.listdir(PRUEBAS)):
        if not nombre.endswith(".comp"):
            continue
        fuente = os.path.join(PRUEBAS, nombre)
        salida = os.path.join(PRUEBAS, nombre[:-5] + ".spv")
        # vulkan1.0 = SPIR-V 1.0; -O0 = lo que el GLSL dice, sin que el
        # optimizador de glslc decida por nosotros que instrucciones aparecen.
        subprocess.run([GLSLC, "--target-env=vulkan1.0", "-O0", "-o", salida, fuente], check=True)
        hechos += 1
    print("fabricar.py: %d sombreadores" % hechos)


if __name__ == "__main__":
    main()
