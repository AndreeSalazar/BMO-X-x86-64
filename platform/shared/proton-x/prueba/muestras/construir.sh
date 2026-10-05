#!/bin/sh
# construir.sh -- las muestras de Microsoft (DirectX-Graphics-Samples, MIT)
# como `.exe` DE VERDAD para la ESCALERA de PROTON-X (E1.0 de
# docs/plan/PLAN_LA_ESCALERA_PROTON_X.md, 05-10). Su fuente, SIN cambios,
# compilada en Linux con mingw-w64 sobre el UCRT: el mismo CRT que Cyberpunk
# (api-ms-win-crt-*). Reproducible: los mismos commits dan el mismo .exe,
# byte a byte (su sha256 esta en ../HACER.txt).
#
#   sh construir.sh DIRECTORIO_DE_TRABAJO
#
# Pide: git, y los paquetes de Ubuntu g++-mingw-w64-x86-64-win32 (13.2.0) y
# mingw-w64-x86-64-dev (11.0.1). Deja hwindow.exe en el directorio de trabajo.
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
    (
        cd "$1"
        for f in *.cpp; do
            [ "$f" = stdafx.cpp ] && continue
            $G -specs=../ucrt.specs -D__MSVCRT_VERSION__=0xE00 -D_UCRT -std=c++17 -O2 -fpermissive \
                -DUNICODE -D_UNICODE -include include/antes.h -I. -Iinclude \
                -I../dxh/include/directx -I../dxh/include -I../dxm/Inc -c "$f" -o "${f%.cpp}.o" 2>/dev/null
        done
        $G -specs=../ucrt.specs -mwindows -static -static-libgcc -static-libstdc++ -s \
            -Wl,--no-insert-timestamp -o "../$2" *.o -ld3d12 -ldxgi -luser32 -lshell32
    )
    sha256sum "$2"
}
construir HelloWindow hwindow.exe
