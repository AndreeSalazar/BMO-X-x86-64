#!/usr/bin/env python3
"""rayosx -- que PIDE de fuera un programa de otro sistema (PE o ELF x86-64).

== Por que existe (2026-09-25) ==
El propietario: *"todos los frontends van por AST, pero lo que se emite en
x86-64 nunca cambia: entonces se pueden analizar"*. Es verdad para las
INSTRUCCIONES: el codigo x86-64 de un juego de Windows ya corre tal cual en el
Ryzen (Wine no traduce instrucciones). Lo que un sistema tendria que DAR para
que ese juego corra son sus IMPORTACIONES: las funciones de otras bibliotecas
que el binario llama. Y esas estan escritas en el propio binario, en una
tabla. Esta herramienta la lee y la cuenta: es la cuenta de lo que habria
que verificar (PLAN_LA_LUDOTECA, seccion 8), medida en vez de adivinada.

== Que es ==
Un OBRERO de medida, no un guardian: no para ningun build. Lee sin ejecutar.

    python rayosx.py juego.exe            PE32 / PE32+ de Windows
    python rayosx.py "C:\\...\\Cyberpunk 2077"  una CARPETA: busca sus .exe
    python rayosx.py juego.x86_64         ELF64 de Linux (los de GOG para Linux)
    python rayosx.py --prueba             su banco: un PE y un ELF hechos aqui

== Que dice ==
El tipo, la maquina, cada biblioteca con cuantas funciones pide, y una
cuenta por FAMILIA (graficos, sonido, entrada, sistema): la superficie que
tendria que existir. Para comparar: el nivel 1 de "devorar" un ELF estatico
son ~15 llamadas (docs/identidad/ENTRAR_EN_SU_ECOSISTEMA.md).
"""
import struct
import sys

FAMILIAS = [
    ("graficos", ("d3d", "dxgi", "opengl", "vulkan", "libgl", "libegl", "ddraw", "sdl")),
    ("sonido", ("dsound", "xaudio", "winmm", "openal", "libasound", "libpulse", "fmod")),
    ("entrada", ("xinput", "dinput", "hid", "libudev")),
    ("red", ("ws2_32", "wininet", "winhttp", "libcurl", "libssl", "steam_api", "galaxy")),
]


def familia(biblio):
    b = biblio.lower()
    for nombre, marcas in FAMILIAS:
        if any(m in b for m in marcas):
            return nombre
    return "sistema"


def cadena(d, o):
    fin = d.index(b"\0", o)
    return d[o:fin].decode("latin-1")


# ---------------------------------------------------------------- PE --------

def pe(d):
    """(maquina, {dll: [funciones]}) de un PE32/PE32+."""
    if d[:2] != b"MZ":
        return None
    e = struct.unpack_from("<I", d, 0x3C)[0]
    if d[e:e + 4] != b"PE\0\0":
        return None
    maquina, nsec, _, _, _, tam_opc, _ = struct.unpack_from("<HHIIIHH", d, e + 4)
    opc = e + 24
    magia = struct.unpack_from("<H", d, opc)[0]
    ancho = 8 if magia == 0x20B else 4
    dirs = opc + (112 if ancho == 8 else 96)
    imp_rva = struct.unpack_from("<I", d, dirs + 8)[0]
    # La 13: las importaciones RETRASADAS (se cargan al primer uso; los
    # juegos grandes ponen ahi media API).
    retr_rva = struct.unpack_from("<I", d, dirs + 13 * 8)[0]
    base = struct.unpack_from("<Q" if ancho == 8 else "<I", d, opc + (24 if ancho == 8 else 28))[0]
    secs = []
    for k in range(nsec):
        s = opc + tam_opc + 40 * k
        vtam, vdir, rtam, rdir = struct.unpack_from("<IIII", d, s + 8)
        secs.append((vdir, max(vtam, rtam), rdir))

    def off(rva):
        for vdir, tam, rdir in secs:
            if vdir <= rva < vdir + tam:
                return rva - vdir + rdir
        raise ValueError("RVA 0x%x fuera de toda seccion" % rva)

    imps = {}
    if imp_rva:
        p = off(imp_rva)
        while True:
            oft, _, _, nombre, ft = struct.unpack_from("<IIIII", d, p)
            if not (oft or nombre or ft):
                break
            dll = cadena(d, off(nombre))
            funcs = []
            t = off(oft or ft)
            while True:
                v = struct.unpack_from("<Q" if ancho == 8 else "<I", d, t)[0]
                if v == 0:
                    break
                if v >> (ancho * 8 - 1):
                    funcs.append("#%d" % (v & 0xFFFF))
                else:
                    funcs.append(cadena(d, off(v & 0x7FFFFFFF) + 2))
                t += ancho
            imps.setdefault(dll, []).extend(funcs)
            p += 20
    if retr_rva:
        p = off(retr_rva)
        while True:
            attr, nombre, _, _, int_rva = struct.unpack_from("<IIIII", d, p)
            if not nombre:
                break
            # attr bit 0: RVA; sin el, direcciones virtuales (los muy viejos).
            rva = (lambda v: v) if attr & 1 else (lambda v: v - base)
            dll = cadena(d, off(rva(nombre)))
            funcs = []
            t = off(rva(int_rva))
            while True:
                v = struct.unpack_from("<Q" if ancho == 8 else "<I", d, t)[0]
                if v == 0:
                    break
                if v >> (ancho * 8 - 1):
                    funcs.append("#%d" % (v & 0xFFFF))
                else:
                    funcs.append(cadena(d, off(rva(v) & 0x7FFFFFFF) + 2))
                t += ancho
            imps.setdefault(dll + " (retrasada)", []).extend(funcs)
            p += 32
    return ({0x8664: "x86-64", 0x14C: "x86 (32 bits)"}.get(maquina, "0x%x" % maquina), imps)


# ---------------------------------------------------------------- ELF -------

def elf(d):
    """(maquina, {biblioteca: [simbolos]}) de un ELF64. Los simbolos sin
    definir no dicen de QUE biblioteca vienen: van todos a `(sin biblioteca
    fijo)` y las bibliotecas NEEDED se listan aparte."""
    if d[:4] != b"\x7fELF" or d[4] != 2:
        return None
    maquina = struct.unpack_from("<H", d, 18)[0]
    shoff = struct.unpack_from("<Q", d, 0x28)[0]
    shentsize, shnum = struct.unpack_from("<HH", d, 0x3A)
    secs = [struct.unpack_from("<IIQQQQIIQQ", d, shoff + k * shentsize) for k in range(shnum)]
    necesita, simbolos = [], []
    for s in secs:
        tipo, off, tam, link, entsize = s[1], s[4], s[5], s[6], s[9]
        if tipo == 6:  # SHT_DYNAMIC
            strs = secs[link][4]
            for k in range(0, tam, 16):
                tag, val = struct.unpack_from("<qQ", d, off + k)
                if tag == 1:  # DT_NEEDED
                    necesita.append(cadena(d, strs + val))
        if tipo == 11:  # SHT_DYNSYM
            strs = secs[link][4]
            for k in range(entsize, tam, entsize or 24):
                nombre, info, _, shndx = struct.unpack_from("<IBBH", d, off + k)
                if shndx == 0 and nombre:
                    simbolos.append(cadena(d, strs + nombre))
    imps = {n: [] for n in necesita}
    imps["(sin biblioteca fija)"] = simbolos
    return ({62: "x86-64", 3: "x86 (32 bits)"}.get(maquina, "0x%x" % maquina), imps)


# ---------------------------------------------------------------- informe ---

def informe(ruta, d):
    for tipo, leer in (("PE (Windows)", pe), ("ELF (Linux)", elf)):
        r = leer(d)
        if r:
            break
    else:
        print("%s: ni PE ni ELF64" % ruta)
        return 1
    maquina, imps = r
    total = sum(len(v) for v in imps.values())
    print("%s: %s, %s" % (ruta, tipo, maquina))
    print("  pide %d funcion(es) de %d biblioteca(s) de fuera" % (total, len([k for k in imps if not k.startswith("(")])))
    cuenta = {}
    for biblio, funcs in sorted(imps.items()):
        print("    %-28s %5d" % (biblio, len(funcs)))
        f = familia(biblio)
        cuenta[f] = cuenta.get(f, 0) + len(funcs)
    print("  por familia: " + ", ".join("%s %d" % kv for kv in sorted(cuenta.items())))
    # Los de GRAFICOS, con nombre: dicen QUE API usa (DirectX 9, 11, 12...).
    graf = sorted({f for b, fs in imps.items() if familia(b) == "graficos" for f in fs})
    if graf:
        print("  graficos, por nombre: " + ", ".join(graf[:12]) + (" ..." if len(graf) > 12 else ""))
    print("  para comparar: devorar un ELF ESTATICO de nivel 1 son ~15 llamadas")
    return 0


# ---------------------------------------------------------------- banco -----

def pe_de_prueba():
    """Un PE32+ minimo: una seccion, una DLL (kernel32.dll) con dos funciones
    por nombre y una por ordinal."""
    sec_rva, sec_raw = 0x1000, 0x200
    datos = bytearray(0x200)
    # nombres (hint + nombre) en +0x100
    def poner(o, b):
        datos[o:o + len(b)] = b
    poner(0x100, b"\0\0ExitProcess\0")
    poner(0x110, b"\0\0CreateFileA\0")
    poner(0x120, b"kernel32.dll\0")
    # tabla de nombres (ILT) en +0x60: 3 entradas + 0
    struct.pack_into("<QQQQ", datos, 0x60, sec_rva + 0x100, sec_rva + 0x110, (1 << 63) | 7, 0)
    # descriptor en +0x00, y uno a cero detras
    struct.pack_into("<IIIII", datos, 0, sec_rva + 0x60, 0, 0, sec_rva + 0x120, sec_rva + 0x60)
    # una RETRASADA, como un juego de DirectX 12: d3d12.dll!D3D12CreateDevice
    poner(0x180, b"d3d12.dll\0")
    poner(0x1C0, b"\0\0D3D12CreateDevice\0")
    struct.pack_into("<QQ", datos, 0x1A0, sec_rva + 0x1C0, 0)
    struct.pack_into("<IIIIIIII", datos, 0x140, 1, sec_rva + 0x180, 0, sec_rva + 0x1A0, sec_rva + 0x1A0, 0, 0, 0)
    cab = bytearray(0x200)
    cab[:2] = b"MZ"
    struct.pack_into("<I", cab, 0x3C, 0x40)
    cab[0x40:0x44] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", cab, 0x44, 0x8664, 1, 0, 0, 0, 240, 0x22)
    opc = 0x58
    struct.pack_into("<H", cab, opc, 0x20B)
    struct.pack_into("<II", cab, opc + 112 + 8, sec_rva, 40)
    struct.pack_into("<II", cab, opc + 112 + 13 * 8, sec_rva + 0x140, 64)
    s = opc + 240
    cab[s:s + 8] = b".idata\0\0"
    struct.pack_into("<IIII", cab, s + 8, 0x200, sec_rva, 0x200, sec_raw)
    return bytes(cab + datos)


def prueba():
    maquina, imps = pe(pe_de_prueba())
    assert maquina == "x86-64", maquina
    assert imps == {"kernel32.dll": ["ExitProcess", "CreateFileA", "#7"], "d3d12.dll (retrasada)": ["D3D12CreateDevice"]}, imps
    assert familia("d3d11.dll") == "graficos" and familia("XINPUT1_4.dll") == "entrada"
    assert familia("libSDL2-2.0.so.0") == "graficos" and familia("kernel32.dll") == "sistema"
    assert pe(b"MZ" + b"\0" * 62) is None and elf(b"nada") is None
    # Un ELF de verdad del anfitrion, si lo hay: tiene que pedir su libc.
    for ruta in ("/bin/ls", "/usr/bin/env"):
        try:
            d = open(ruta, "rb").read()
        except OSError:
            continue
        r = elf(d)
        if r:
            assert any(k.startswith("libc.so") for k in r[1]), r[1].keys()
            break
    print("rayosx: banco en verde (un PE hecho aqui, y un ELF del anfitrion si lo hay)")
    return 0


def exes_de(carpeta):
    """Los .exe de una carpeta y sus subcarpetas, del mas grande al mas chico
    (el del juego suele ser el mas grande; los lanzadores y el reportero de
    fallos, chicos)."""
    import os
    todos = []
    for raiz, _, ficheros in os.walk(carpeta):
        for f in ficheros:
            if f.lower().endswith(".exe"):
                r = os.path.join(raiz, f)
                todos.append((os.path.getsize(r), r))
    return [r for _, r in sorted(todos, reverse=True)]


def uno(ruta):
    import os
    if not os.path.exists(ruta):
        print("%s: no existe. Pon la ruta de TU juego entre comillas; o la CARPETA del juego, y se buscan sus .exe" % ruta)
        return 1
    if os.path.isdir(ruta):
        exes = exes_de(ruta)
        if not exes:
            print("%s: carpeta sin ningun .exe" % ruta)
            return 1
        print("%s: %d .exe; primero el mas grande" % (ruta, len(exes)))
        return max(uno(r) for r in exes[:4])
    try:
        return informe(ruta, open(ruta, "rb").read())
    except (ValueError, struct.error, IndexError) as e:
        print("%s: no se pudo leer entero (%s)" % (ruta, e))
        return 1


def main(args):
    if args == ["--prueba"]:
        return prueba()
    if not args:
        print(__doc__)
        return 1
    return max(uno(r) for r in args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
