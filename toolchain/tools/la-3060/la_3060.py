#!/usr/bin/env python3
"""la-3060 -- la puerta de la GPU, sus registros, sus ordenes y sus esperas no se aflojan.

Por que existe
==============

El 2026-09-25 la 3060 funciono entera por primera vez (51 de 51, el
compositor por GPU, DOOM a 70 fps con 19866 tandas de la 3060 debajo). Y el
propietario pidio lo siguiente: *"aislar MAS con CARRIL y obligando a que mis
guardianes PROTEJAN de GPU para optimizacion"*. Optimizar es cambiar codigo
ROJO deprisa, y lo que se afloja deprisa no se ve: por eso esto.

Al estudiarla aparecio el primer agujero: la puerta de las 65 ordenes
(`OP_IOMMU`) solo miraba QUIEN TIENE LA PANTALLA, y el escritorio la PRESTA.
Con DOOM delante, DOOM podia prestarle RAM a la 3060, despertar su GSP o
apagar la IOMMU. Se cerro con la autoridad `MAQUINA` (task/autoridad.rs), y
este guardian hace que no se pueda volver a abrir sin que el build lo diga.

Las reglas (todas ESTRICTAS: hoy se cumplen enteras, sin trinquete, salvo
las esperas, que es la deuda que se quiere bajar):

    P  la PUERTA   `fn iommu_` de syscall/op_maquina.rs pide
                   `autoridad::MAQUINA` antes de mirar la orden
    R  REGISTROS   solo dev/gpu*, dev/gpu_trabajo/ y dev/vblank.rs tocan
                   los registros de la 3060 (`Bar0(`, `gpu::bar0()`)
    O  ORDENES     cada `IOMMU_OP_*` vale lo mismo en el kernel, el ABI y
                   userland; ninguna repite valor; cada una tiene nombre
                   (`nombre_iommu`, lo que dice la pantalla amarilla) y
                   brazo en op_maquina.rs
    M  MOTIVOS     cada `IOMMU_NO_*` vale lo mismo en los tres sitios, dos
                   nombres no comparten numero, y el escritorio tiene texto
                   para cada uno (`commands/iommu.rs::motivo`)
    E  ESPERAS     los `spin_loop()` de la 3060 en Ring 0 son un trinquete:
                   cada uno es un nucleo girando mientras la tarjeta trabaja,
                   y el camino es cambiarlos por esperas con interrupcion
    I  IDENTIDAD   la 3060 12G y SOLO ella (25-09): la lista de dispositivos
                   del crate (`identidad.rs`) y la del cargador
                   (`gpu_reinicio.rs`) son la misma; la sonda del kernel pide
                   `es_la_3060_12g` y el prestamo `vram_es_la_suya`
    O3 OPTIMIZADA  `bmo-gpu-ga10x` lleva `opt-level = 3` con nombre en el
                   perfil release del kernel y del userspace

    --check   lo que corre el build
"""
import glob
import os
import re
import sys
from collections import defaultdict

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..'))
K = os.path.join(RAIZ, 'Ultra_kernel_x86-64', 'kernel', 'src')
RING0 = os.path.join(K, 'ring0')
ABI = os.path.join(RAIZ, 'platform', 'abi', 'bmo-abi', 'src', 'syscalls', 'surface', 'tarea.rs')
USER = os.path.join(RAIZ, 'Ultra_userspace', 'userland', 'src', 'lib.rs')
MOTIVOS_TXT = os.path.join(RAIZ, 'Ultra_userspace', 'services', 'director', 'src', 'commands', 'iommu.rs')
BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'LINEA_BASE.txt')

RX_CONST = r'pub(?:\(crate\))?\s+const\s+(%s[A-Z0-9_]+)\s*:\s*u(?:32|64)\s*=\s*(0x[0-9A-Fa-f_]+|\d+)\s*;'
RX_REGISTROS = re.compile(r'\bBar0\(|gpu::bar0\(\)')


def leer(ruta):
    with open(ruta, 'rb') as f:
        return f.read().decode('utf-8', 'replace').replace('\r\n', '\n')


def sin_comentarios(texto):
    return '\n'.join(l.split('//', 1)[0] for l in texto.split('\n'))


def rel(ruta):
    return os.path.relpath(ruta, RAIZ).replace('\\', '/')


def constantes(rutas, prefijo):
    """nombre -> {(valor, fichero)}"""
    d = defaultdict(set)
    rx = re.compile(RX_CONST % prefijo)
    for r in rutas:
        for m in rx.finditer(sin_comentarios(leer(r))):
            d[m.group(1)].add((int(m.group(2).replace('_', ''), 0), rel(r)))
    return d


def valor(d):
    return {k: next(iter(v))[0] for k, v in d.items()}


def es_de_la_3060(ruta):
    r = rel(ruta)
    return (re.search(r'/ring0/dev/gpu[^/]*\.rs$', r) is not None
            or '/ring0/dev/gpu_trabajo/' in r
            or r.endswith('/ring0/dev/vblank.rs'))


def puerta(fallos):
    t = sin_comentarios(leer(os.path.join(RING0, 'syscall', 'op_maquina.rs')))
    i = t.find('fn iommu_(')
    if i < 0:
        fallos.append('P: no se encuentra `fn iommu_(` en syscall/op_maquina.rs: la puerta se movio y este guardian no mira')
        return
    cuerpo = t[i:i + 4000]
    llave = cuerpo.find('autoridad::MAQUINA')
    orden = cuerpo.find('match arg0')
    if llave < 0 or (orden >= 0 and llave > orden):
        fallos.append('P: `fn iommu_` ya no pide `autoridad::MAQUINA` antes de la orden: una app con la pantalla prestada volveria a mandar en la 3060')


def registros(fallos):
    for r in sorted(glob.glob(os.path.join(K, '**', '*.rs'), recursive=True)):
        if es_de_la_3060(r):
            continue
        t = sin_comentarios(leer(r))
        for n, l in enumerate(t.split('\n'), 1):
            if RX_REGISTROS.search(l):
                fallos.append('R: %s:%d toca los registros de la 3060 fuera de dev/gpu*: %s' % (rel(r), n, l.strip()))


def ordenes(fallos):
    kernel = os.path.join(RING0, 'syscall', 'ops.rs')
    k, a, u = (constantes([p], 'IOMMU_OP_') for p in (kernel, ABI, USER))
    if not k:
        fallos.append('O: CERO ordenes leidas de syscall/ops.rs: un guardian que no encuentra nada no ha mirado')
        return 0
    kv, av, uv = valor(k), valor(a), valor(u)
    for nombre, v in sorted(kv.items()):
        for donde, otro in (('el ABI', av), ('userland', uv)):
            if nombre not in otro:
                fallos.append('O: %s falta en %s' % (nombre, donde))
            elif otro[nombre] != v:
                fallos.append('O: %s vale 0x%X en el kernel y 0x%X en %s' % (nombre, v, otro[nombre], donde))
    for donde, otro in (('el ABI', av), ('userland', uv)):
        for nombre in sorted(set(otro) - set(kv)):
            fallos.append('O: %s esta en %s y no en el kernel' % (nombre, donde))
    por_valor = defaultdict(list)
    for nombre, v in kv.items():
        por_valor[v].append(nombre)
    for v, nombres in sorted(por_valor.items()):
        if len(nombres) > 1:
            fallos.append('O: 0x%X lo usan %s' % (v, ', '.join(sorted(nombres))))
    texto = sin_comentarios(leer(kernel))
    con_nombre = set(re.findall(r'(IOMMU_OP_[A-Z0-9_]+)\s*=>\s*"', texto))
    brazos = set(re.findall(r'IOMMU_OP_[A-Z0-9_]+', sin_comentarios(leer(os.path.join(RING0, 'syscall', 'op_maquina.rs')))))
    for nombre in sorted(kv):
        if nombre not in con_nombre:
            fallos.append('O: %s no tiene nombre en `nombre_iommu`: la pantalla amarilla diria "?"' % nombre)
        if nombre not in brazos:
            fallos.append('O: %s no tiene brazo en syscall/op_maquina.rs' % nombre)
    return len(kv)


def motivos(fallos):
    kernel = glob.glob(os.path.join(K, '**', '*.rs'), recursive=True)
    k, a, u = constantes(kernel, 'IOMMU_NO_'), constantes([ABI], 'IOMMU_NO_'), constantes([USER], 'IOMMU_NO_')
    if not k:
        fallos.append('M: CERO motivos leidos del kernel')
        return 0
    for nombre, sitios in sorted(k.items()):
        if len({v for v, _ in sitios}) > 1:
            fallos.append('M: %s tiene dos valores en el kernel: %s' % (nombre, sorted(sitios)))
    kv, av, uv = valor(k), valor(a), valor(u)
    for nombre, v in sorted(kv.items()):
        for donde, otro in (('el ABI', av), ('userland', uv)):
            if nombre not in otro:
                fallos.append('M: %s falta en %s' % (nombre, donde))
            elif otro[nombre] != v:
                fallos.append('M: %s vale %d en el kernel y %d en %s' % (nombre, v, otro[nombre], donde))
    for donde, otro in (('el ABI', av), ('userland', uv)):
        for nombre in sorted(set(otro) - set(kv)):
            fallos.append('M: %s esta en %s y no en el kernel' % (nombre, donde))
    por_valor = defaultdict(list)
    for nombre, v in kv.items():
        por_valor[v].append(nombre)
    for v, nombres in sorted(por_valor.items()):
        if len(nombres) > 1:
            fallos.append('M: el motivo %d lo usan %s: el escritorio diria lo de uno cuando paso lo del otro' % (v, ', '.join(sorted(nombres))))
    t = sin_comentarios(leer(MOTIVOS_TXT))
    i = t.find('fn motivo(')
    cuerpo = t[i:] if i >= 0 else ''
    con_texto = set(re.findall(r'bmo::(IOMMU_NO_[A-Z0-9_]+)', cuerpo))
    for nombre in sorted(set(uv) - con_texto):
        fallos.append('M: %s no tiene texto en commands/iommu.rs::motivo: el NO saldria sin su porque' % nombre)
    return len(kv)


IDENTIDAD = os.path.join(RAIZ, 'platform', 'drivers', 'gpu', 'ga10x', 'src', 'identidad.rs')
CARGADOR = os.path.join(RAIZ, 'Ultra_kernel_x86-64', 'faggin', 's1_cpu', 'src', 'gpu_reinicio.rs')
RX_LISTA = re.compile(r'const DISPOSITIVOS\s*:\s*\[u16;\s*\d+\]\s*=\s*\[([^\]]*)\]')


def lista(ruta):
    m = RX_LISTA.search(sin_comentarios(leer(ruta)))
    if not m:
        return None
    return sorted(int(x.strip().replace('_', ''), 0) for x in m.group(1).split(',') if x.strip())


def identidad(fallos):
    crate, cargador = lista(IDENTIDAD), lista(CARGADOR)
    if not crate:
        fallos.append('I: no se lee `DISPOSITIVOS` de %s' % rel(IDENTIDAD))
    if not cargador:
        fallos.append('I: no se lee `DISPOSITIVOS` de %s' % rel(CARGADOR))
    if crate and cargador and crate != cargador:
        fallos.append('I: el crate maneja %s y el cargador reinicia %s: la misma 3060 o ninguna'
                      % ([hex(x) for x in crate], [hex(x) for x in cargador]))
    for fichero, llamada in (('dev/gpu.rs', 'identidad::es_la_3060_12g('), ('dev/gpu_prestamo.rs', 'identidad::vram_es_la_suya(')):
        if llamada not in sin_comentarios(leer(os.path.join(RING0, *fichero.split('/')))):
            fallos.append('I: %s ya no llama a `%s`: el kernel manejaria otra tarjeta como si fuera la 3060 12G' % (fichero, llamada.rstrip('(')))
    return crate or []


def optimizada(fallos):
    for ws in ('Ultra_kernel_x86-64', 'Ultra_userspace'):
        ruta = os.path.join(RAIZ, ws, 'Cargo.toml')
        t = leer(ruta)
        m = re.search(r'\[profile\.release\.package\."bmo-gpu-ga10x"\]([^\[]*)', t)
        if not m or not re.search(r'^\s*opt-level\s*=\s*3\s*$', m.group(1), re.M):
            fallos.append('O3: %s no lleva `[profile.release.package."bmo-gpu-ga10x"] opt-level = 3`' % rel(ruta))


def esperas():
    cuenta = {}
    for r in sorted(glob.glob(os.path.join(RING0, 'dev', '**', '*.rs'), recursive=True)):
        if es_de_la_3060(r):
            n = sin_comentarios(leer(r)).count('spin_loop()')
            if n:
                cuenta[rel(r)] = n
    return cuenta


def linea_base():
    try:
        for l in leer(BASE).split('\n'):
            l = l.strip()
            if l and not l.startswith('#'):
                return int(l)
    except (OSError, ValueError):
        pass
    return None


def main():
    fallos = []
    puerta(fallos)
    registros(fallos)
    n_ordenes = ordenes(fallos)
    n_motivos = motivos(fallos)
    suyos = identidad(fallos)
    optimizada(fallos)
    giros = esperas()
    total = sum(giros.values())
    base = linea_base()
    if base is None:
        fallos.append('E: falta %s con la linea base de las esperas' % rel(BASE))
    elif total > base:
        fallos.append('E: los `spin_loop()` de la 3060 SUBIERON: %d, y la linea base es %d. Cada uno es un nucleo girando: espera con interrupcion, o cede' % (total, base))
        for r, n in sorted(giros.items()):
            fallos.append('     %3d  %s' % (n, r))
    if fallos:
        print('FAIL: la 3060 se aflojo por %d sitio(s)' % len(fallos))
        for f in fallos:
            print('  ' + f)
        return 1
    extra = '' if base is None or total == base else ' (bajo de %d: baja la linea base en %s)' % (base, rel(BASE))
    print('clean: la puerta pide MAQUINA; solo la 3060 12G (%s); registros solo en dev/gpu*; %d ordenes y %d motivos iguales en los tres sitios; opt-level 3; %d esperas girando%s'
          % ('/'.join('%04X' % x for x in suyos), n_ordenes, n_motivos, total, extra))
    return 0


if __name__ == '__main__':
    if '--check' not in sys.argv[1:]:
        print(__doc__)
        sys.exit(0)
    sys.exit(main())
