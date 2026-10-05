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

# construir CARPETA EXE: la carpeta de la muestra (bajo Samples/Desktop) y el .exe.
construir() {
    src="dgs/Samples/Desktop/$1"
    b="obra_${2%.exe}"
    rm -rf "$b" && mkdir -p "$b/include/d3dx12"
    cp "$src"/*.cpp "$src"/*.h "$b"/
    # Los nombres que la muestra pide como en Windows (sin mayusculas en Linux).
    printf '#include <d3dx12.h>\n' > "$b/include/d3dx12/d3dx12.h"
    printf '#include <d3dcompiler.h>\n' > "$b/include/D3Dcompiler.h"
    cp "$AQUI/antes.h" "$AQUI/pix3.h" "$b/include/"
    cp "$AQUI/guids.cpp" "$b/zz_guids.cpp"
    (
        cd "$b"
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
    # Los Hello: sus dos .cso, de shaders.hlsl, si hay DXC.
    if [ -n "${DXC:-}" ] && [ -f "$src/shaders.hlsl" ]; then
        sombreador "$1" "${2%.exe}" shaders vs_6_0 VSMain shaders_VSMain
        sombreador "$1" "${2%.exe}" shaders ps_6_0 PSMain shaders_PSMain
    fi
}

# sombreador CARPETA MUESTRA HLSL PERFIL ENTRADA CSO: un .cso como lo hace el
# proyecto de Visual Studio de la muestra (`dxc -nologo -T<perfil> -E<entrada>
# -Zi -Qembed_debug`), en <directorio>/<muestra>/. Sin DXC, nada.
sombreador() {
    [ -n "${DXC:-}" ] || return 0
    mkdir -p "$2"
    cp "dgs/Samples/Desktop/$1/$3.hlsl" "$2/"
    (
        cd "$2"
        "$DXC" -nologo -T"$4" -E"$5" -Zi -Qembed_debug -Fo "$6.cso" "$3.hlsl"
        rm "$3.hlsl"
    )
    sha256sum "$2/$6.cso"
}

# datos CARPETA MUESTRA FICHERO: lo que la muestra lee de su carpeta (su malla).
datos() {
    mkdir -p "$2"
    cp "dgs/Samples/Desktop/$1/$3" "$2/"
    sha256sum "$2/$3"
}

# Los Hello de D3D12HelloWorld (E1 de la escalera), con nombres de 8.3: el
# FAT32 de BMO-X busca asi.
H=D3D12HelloWorld/src
for m in htriang htexture hcbuffer hframes hbundles; do rm -rf "$m"; done
construir $H/HelloWindow hwindow.exe
construir $H/HelloTriangle htriang.exe
construir $H/HelloTexture htexture.exe
construir $H/HelloConstBuffers hcbuffer.exe
construir $H/HelloFrameBuffering hframes.exe
construir $H/HelloBundles hbundles.exe

# E2 de la escalera: una muestra por casilla abierta.
D=D3D12DynamicIndexing/src
rm -rf dynindex
construir $D dynindex.exe
sombreador $D dynindex shader_mesh_simple_vert vs_6_0 VSMain shader_mesh_simple_vert
sombreador $D dynindex shader_mesh_dynamic_indexing_pixel ps_6_0 PSMain shader_mesh_dynamic_indexing_pixel
datos $D dynindex occcity.bin

# E2.3b: el COMPUTO de verdad (y su hilo, sus colas y su GS). Los cuatro
# .cso como su proyecto: el CS, y de ParticleDraw.hlsl el VS, el GS y el PS.
N=D3D12nBodyGravity/src
rm -rf nbody
construir $N nbody.exe
sombreador $N nbody nBodyGravityCS cs_6_0 CSMain nBodyGravityCS
sombreador $N nbody ParticleDraw vs_6_0 VSParticleDraw ParticleDraw_VS
sombreador $N nbody ParticleDraw gs_6_0 GSParticleDraw ParticleDraw_GS
sombreador $N nbody ParticleDraw ps_6_0 PSParticleDraw ParticleDraw_PS

# E2.4: ExecuteIndirect y su culling por computo (Append en un UAV con
# contador). Sus .cso: los de shaders.hlsl (los hace `construir`) y el CS.
I=D3D12ExecuteIndirect/src
rm -rf indirect
construir $I indirect.exe
sombreador $I indirect compute cs_6_0 CSMain compute

# E2.7: las consultas de oclusion y la predicacion. Sus .cso, los de
# shaders.hlsl (los hace `construir`).
P=D3D12PredicationQueries/src
rm -rf predica
construir $P predica.exe

# E2.3a: el juez del COMPUTO, NUESTRO y no de Microsoft (`../computo.cpp`,
# de consola, con su CS de `../computo.dxil` dentro por `.incbin`): las
# mismas cabeceras y el mismo UCRT que las muestras.
cp "$AQUI/../computo.cpp" "$AQUI/../computo.dxil" .
$G -specs=ucrt.specs -D__MSVCRT_VERSION__=0xE00 -D_UCRT -std=c++17 -O2 -Idxh/include/directx -Idxh/include -c computo.cpp -o computo.o
$G -specs=ucrt.specs -static -static-libgcc -static-libstdc++ -s -Wl,--no-insert-timestamp -o computo.exe computo.o -ld3d12
sha256sum computo.exe

# N5.13: el juez de las INSTANCIAS, NUESTRO tambien (`../instancias.cpp`,
# con sus tres sombreadores de `../instancias_*.dxil` dentro).
cp "$AQUI/../instancias.cpp" "$AQUI"/../instancias_*.dxil .
$G -specs=ucrt.specs -D__MSVCRT_VERSION__=0xE00 -D_UCRT -std=c++17 -O2 -Idxh/include/directx -Idxh/include -c instancias.cpp -o instancias.o
$G -specs=ucrt.specs -static -static-libgcc -static-libstdc++ -s -Wl,--no-insert-timestamp -o instancias.exe instancias.o -ld3d12
sha256sum instancias.exe

# N5.3b y N5.3c: el juez de las VISTAS, NUESTRO (`../vistas.cpp`, con sus
# tres CS de `../vistas_*.dxil` dentro).
cp "$AQUI/../vistas.cpp" "$AQUI"/../vistas_*.dxil .
$G -specs=ucrt.specs -D__MSVCRT_VERSION__=0xE00 -D_UCRT -std=c++17 -O2 -Idxh/include/directx -Idxh/include -c vistas.cpp -o vistas.o
$G -specs=ucrt.specs -static -static-libgcc -static-libstdc++ -s -Wl,--no-insert-timestamp -o vistas.exe vistas.o -ld3d12
sha256sum vistas.exe

# N5.16: el juez de los render targets de FLOAT, NUESTRO (`../hdr.cpp`, con
# sus tres sombreadores de `../hdr_*.dxil` dentro).
cp "$AQUI/../hdr.cpp" "$AQUI"/../hdr_*.dxil .
$G -specs=ucrt.specs -D__MSVCRT_VERSION__=0xE00 -D_UCRT -std=c++17 -O2 -Idxh/include/directx -Idxh/include -c hdr.cpp -o hdr.o
$G -specs=ucrt.specs -static -static-libgcc -static-libstdc++ -s -Wl,--no-insert-timestamp -o hdr.exe hdr.o -ld3d12
sha256sum hdr.exe

# 05-10: el juez de los UAV escritos desde un DIBUJO, NUESTRO
# (`../uavpixel.cpp`, con sus tres sombreadores de `../uavpixel_*.dxil`).
cp "$AQUI/../uavpixel.cpp" "$AQUI"/../uavpixel_*.dxil .
$G -specs=ucrt.specs -D__MSVCRT_VERSION__=0xE00 -D_UCRT -std=c++17 -O2 -Idxh/include/directx -Idxh/include -c uavpixel.cpp -o uavpixel.o
$G -specs=ucrt.specs -static -static-libgcc -static-libstdc++ -s -Wl,--no-insert-timestamp -o uavpixel.exe uavpixel.o -ld3d12
sha256sum uavpixel.exe
