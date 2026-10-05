#!/bin/sh
# construir.sh -- las muestras de Microsoft (DirectX-Graphics-Samples, MIT)
# como `.exe` DE VERDAD para la ESCALERA de PROTON-X (E1.0 de
# docs/plan/PLAN_LA_ESCALERA_PROTON_X.md, 05-10). Su fuente, SIN cambios,
# compilada en Linux con mingw-w64 sobre el UCRT: el mismo CRT que Cyberpunk
# (api-ms-win-crt-*). Reproducible: los mismos commits dan el mismo .exe,
# byte a byte (su sha256 esta en ../HACER.txt).
#
#   DXC=/ruta/a/dxc sh construir.sh DIRECTORIO_DE_TRABAJO
#
# Pide: git, y los paquetes de Ubuntu g++-mingw-w64-x86-64-win32 (13.2.0) y
# mingw-w64-x86-64-dev (11.0.1). Deja los .exe en el directorio de trabajo.
#
# Los sombreadores: las muestras leen shaders_VSMain.cso y shaders_PSMain.cso,
# que su proyecto de Visual Studio compila con DXC al construir
# (`dxc -nologo -Tvs_6_0 -E"VSMain" -Zi -Qembed_debug`). Con DXC=, este script
# los hace igual, en <directorio>/<muestra>/. El DXC de la casa: v1.8.2505
# (9efbb6c3), el de las publicaciones de Microsoft, o compilado de su fuente
# en ese commit (`cmake -G Ninja -C cmake/caches/PredefinedParams.cmake
# -DHLSL_INCLUDE_TESTS=OFF -DSPIRV_BUILD_TESTS=OFF`, y `ninja dxc`).
set -eu

DGS=e5975f9b0744fc5096dd593ed72bf8f5c004164f      # microsoft/DirectX-Graphics-Samples, 2026-09-23
DXH=adbd6f3ba40795c46a8d0f33af00bcb57ff0f0a4      # microsoft/DirectX-Headers, 2026-09-23
DXM=e2f2b9bddbbc4fd0f6f63586f86b2279213b991d      # microsoft/DirectXMath, 2026-10-02

AQUI=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$1"
cd "$1"

traer() { # traer REPO COMMIT CARPETA
    if [ ! -d "$3" ]; then
        git init -q "$3"
        git -C "$3" fetch -q --depth 1 "https://github.com/microsoft/$1" "$2"
        git -C "$3" checkout -q FETCH_HEAD
    fi
}
traer DirectX-Graphics-Samples "$DGS" dgs
traer DirectX-Headers "$DXH" dxh
traer DirectXMath "$DXM" dxm

G=x86_64-w64-mingw32-g++-win32
# El UCRT en vez de msvcrt.dll: los specs de este gcc, con -lucrt.
$G -dumpspecs | sed 's/-lmsvcrt/-lucrt/g' > ucrt.specs

# construir MUESTRA EXE: la carpeta de D3D12HelloWorld/src y el .exe.
construir() {
    rm -rf "$1" && mkdir -p "$1/include/d3dx12"
    cp dgs/Samples/Desktop/D3D12HelloWorld/src/"$1"/*.cpp dgs/Samples/Desktop/D3D12HelloWorld/src/"$1"/*.h "$1"/
    # Los nombres que la muestra pide como en Windows (sin mayusculas en Linux).
    printf '#include <d3dx12.h>\n' > "$1/include/d3dx12/d3dx12.h"
    printf '#include <d3dcompiler.h>\n' > "$1/include/D3Dcompiler.h"
    cp "$AQUI/antes.h" "$1/include/antes.h"
    cp "$AQUI/guids.cpp" "$1/zz_guids.cpp"
    (
        cd "$1"
        for f in *.cpp; do
            [ "$f" = stdafx.cpp ] && continue
            # Los IID (guids.cpp) sin el puente: solo las cabeceras.
            puente="-include include/antes.h"
            [ "$f" = zz_guids.cpp ] && puente=""
            $G -specs=../ucrt.specs -D__MSVCRT_VERSION__=0xE00 -D_UCRT -std=c++17 -O2 -fpermissive \
                -DUNICODE -D_UNICODE $puente -I. -Iinclude \
                -I../dxh/include/directx -I../dxh/include -I../dxm/Inc -c "$f" -o "${f%.cpp}.o" 2>/dev/null
        done
        # Los IID en una biblioteca: el enlazador solo la usa si algo los
        # pide, y un .exe que no los pide sale igual que sin ella.
        x86_64-w64-mingw32-ar rcD libguids.a zz_guids.o
        $G -specs=../ucrt.specs -mwindows -static -static-libgcc -static-libstdc++ -s \
            -Wl,--no-insert-timestamp -o "../$2" $(ls *.o | grep -v zz_guids) -L. -lguids -ld3d12 -ldxgi -luser32 -lshell32
    )
    sha256sum "$2"
    # Sus .cso, si hay DXC y la muestra los lee.
    if [ -n "${DXC:-}" ] && [ -f "dgs/Samples/Desktop/D3D12HelloWorld/src/$1/shaders.hlsl" ]; then
        m=${2%.exe}
        rm -rf "$m" && mkdir -p "$m"
        cp "dgs/Samples/Desktop/D3D12HelloWorld/src/$1/shaders.hlsl" "$m/"
        (
            cd "$m"
            "$DXC" -nologo -Tvs_6_0 -E"VSMain" -Zi -Qembed_debug -Fo shaders_VSMain.cso shaders.hlsl
            "$DXC" -nologo -Tps_6_0 -E"PSMain" -Zi -Qembed_debug -Fo shaders_PSMain.cso shaders.hlsl
            rm shaders.hlsl
        )
        sha256sum "$m"/*.cso
    fi
}
# Los Hello de D3D12HelloWorld (E1 de la escalera), con nombres de 8.3: el
# FAT32 de BMO-X busca asi.
construir HelloWindow hwindow.exe
construir HelloTriangle htriang.exe
construir HelloTexture htexture.exe
construir HelloConstBuffers hcbuffer.exe
construir HelloFrameBuffering hframes.exe
construir HelloBundles hbundles.exe
