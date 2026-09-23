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
SPIRV_AS = os.path.join(SDK, "Bin", "spirv-as.exe" if os.name == "nt" else "spirv-as")
DXC = os.path.join(SDK, "Bin", "dxc.exe" if os.name == "nt" else "dxc")


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
    # Y los escritos a mano en SPIR-V de texto: lo que ningun GLSL valido
    # produce (un valor usado donde no domina, para el oraculo).
    for nombre in sorted(os.listdir(PRUEBAS)):
        if not nombre.endswith(".spvasm"):
            continue
        fuente = os.path.join(PRUEBAS, nombre)
        salida = os.path.join(PRUEBAS, nombre[:-7] + ".spv")
        subprocess.run([SPIRV_AS, "--target-env", "spv1.0", "-o", salida, fuente], check=True)
        hechos += 1
    # ** LAS OTRAS DOS PUERTAS (2026-09-23): OpenGL y DirectX llegan al MISMO
    # SPIR-V. El GLSL de computo se fabrica tambien para OpenGL, y los HLSL de
    # `hlsl/` con `dxc -spirv` (el compilador de Microsoft, en el mismo SDK).
    os.makedirs(os.path.join(PRUEBAS, "opengl"), exist_ok=True)
    for nombre in ("suma", "saxpy", "mandelbrot", "trascendentes"):
        fuente = os.path.join(PRUEBAS, nombre + ".comp")
        salida = os.path.join(PRUEBAS, "opengl", nombre + ".spv")
        subprocess.run([GLSLC, "--target-env=opengl", "-O0", "-o", salida, fuente], check=True)
        hechos += 1
    HLSL = os.path.join(PRUEBAS, "hlsl")
    for nombre in sorted(os.listdir(HLSL)):
        if not nombre.endswith(".hlsl"):
            continue
        fuente = os.path.join(HLSL, nombre)
        salida = os.path.join(HLSL, nombre[:-5] + ".spv")
        subprocess.run([DXC, "-spirv", "-T", "cs_6_0", "-E", "main", "-fspv-target-env=vulkan1.0",
                        "-Fo", salida, fuente], check=True)
        hechos += 1
    print("fabricar.py: %d sombreadores" % hechos)


if __name__ == "__main__":
    main()
