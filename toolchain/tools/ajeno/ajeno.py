#!/usr/bin/env python3
"""ajeno -- el disco PERSONAL se lee y NUNCA se escribe; C: ni se mira.

Por que existe
==============

El 2026-09-29 el kernel aprendio a leer NTFS (N1a, `bmo-ntfs` y
`ring0/dev/disk/ajeno.rs`) para encontrar el Cyberpunk 2077 del propietario
en `Personal (D:)`, OTRO SSD SATA en el MISMO controlador AHCI que el disco de
BMO-X. El propietario: *"pon guardianes tambien y eso es en disco PERSONAL no
en C que es principal que yo vivo"*. Un disco donde vive su vida no se
escribe, ni por error, ni dentro de un ano, ni con un cambio deprisa: por eso
esto.

Las reglas (todas ESTRICTAS, sin trinquete):

    A1 CERROJO 2   cada llamada del kernel que ESCRIBE en un disco AHCI
                   (`bmo_ahci::write_sectors_phys`, `trim_phys`,
                   `flush_cache`) lleva antes, en las 6 lineas de arriba,
                   `ajeno::escribible(`; y `bmo_ahci::emitir` solo manda
                   ordenes de LEER (`ATA_CMD_READ...`)
    A2 CERROJO 1   `disk/ajeno.rs` no nombra nada que escriba (`write_sectors`,
                   `trim_phys`, `flush_cache`, `emitir`, `run_command`,
                   `ATA_CMD_WRITE`); su `fn write` devuelve
                   `Err(BlockError::ReadOnly)`, su `fn writable` es `false`,
                   y `fn escribible` niega su puerto (`puerto() == Some(p)`)
    A3 UN DISCO    `bmo_block::register(` solo con `&AHCI_DISK`: el disco
                   ajeno nunca es "el disco" del sistema de ficheros
    A4 EL LECTOR   `bmo-ntfs` (src/, sin las pruebas) no llama a `.write(`
                   ni a `.flush(` del dispositivo
    A5 C: NO       el NVMe (`C:`, donde vive el propietario) ni se mira:
                   `ring0/dev/disk/` no nombra `Nvme`, y el kernel no enlaza
                   un driver NVMe (`bmo-nvme`, `bmo_nvme`)

    --check   lo que corre el build
"""
import glob
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..'))
K = os.path.join(RAIZ, 'Ultra_kernel_x86-64', 'kernel')
SRC = os.path.join(K, 'src')
DISCO = os.path.join(SRC, 'ring0', 'dev', 'disk')
AJENO = os.path.join(DISCO, 'ajeno.rs')
NTFS = os.path.join(RAIZ, 'platform', 'drivers', 'storage', 'ntfs', 'src')

RX_ESCRIBE = re.compile(r'bmo_ahci::(write_sectors_phys|trim_phys|flush_cache)\s*\(')
RX_EMITIR = re.compile(r'bmo_ahci::emitir\s*\(')
RX_REGISTRA = re.compile(r'bmo_block::register\s*\(([^)]*)\)')
PROHIBIDO_EN_AJENO = ('write_sectors', 'trim_phys', 'flush_cache', 'emitir', 'run_command', 'ATA_CMD_WRITE')


def leer(ruta):
    with open(ruta, 'rb') as f:
        return f.read().decode('utf-8', 'replace').replace('\r\n', '\n')


def sin_comentarios(texto):
    return '\n'.join(l.split('//', 1)[0] for l in texto.split('\n'))


def rel(ruta):
    return os.path.relpath(ruta, RAIZ).replace('\\', '/')


def fuentes(d):
    return sorted(glob.glob(os.path.join(d, '**', '*.rs'), recursive=True))


def cuerpo(texto, firma):
    """El cuerpo `{...}` de la primera `fn` cuyo nombre es `firma`."""
    m = re.search(r'\bfn\s+%s\s*\(' % firma, texto)
    if not m:
        return None
    i = texto.find('{', m.end())
    if i < 0:
        return None
    nivel = 0
    for j in range(i, len(texto)):
        if texto[j] == '{':
            nivel += 1
        elif texto[j] == '}':
            nivel -= 1
            if nivel == 0:
                return texto[i + 1:j]
    return None


def cerrojo_2(fallos):
    n = 0
    for r in fuentes(SRC):
        lineas = sin_comentarios(leer(r)).split('\n')
        for i, l in enumerate(lineas):
            if RX_ESCRIBE.search(l):
                n += 1
                antes = '\n'.join(lineas[max(0, i - 6):i + 1])
                if 'ajeno::escribible(' not in antes:
                    fallos.append('A1: %s:%d escribe en un disco AHCI sin mirar antes `ajeno::escribible(`' % (rel(r), i + 1))
            if RX_EMITIR.search(l):
                orden = ' '.join(lineas[i:i + 3])
                if 'ATA_CMD_READ' not in orden:
                    fallos.append('A1: %s:%d `bmo_ahci::emitir` con una orden que no es de LEER' % (rel(r), i + 1))
    if n == 0:
        fallos.append('A1: ninguna escritura AHCI en el kernel: el guardian ya no ve el camino (se movio?)')
    return n


def cerrojo_1(fallos):
    if not os.path.exists(AJENO):
        fallos.append('A2: falta %s' % rel(AJENO))
        return
    t = sin_comentarios(leer(AJENO))
    for p in PROHIBIDO_EN_AJENO:
        if p in t:
            fallos.append('A2: %s nombra `%s`: el disco ajeno no escribe' % (rel(AJENO), p))
    w = cuerpo(t, 'write')
    if w is None or w.strip() != 'Err(BlockError::ReadOnly)':
        fallos.append('A2: `fn write` de %s no es solo `Err(BlockError::ReadOnly)`' % rel(AJENO))
    e = cuerpo(t, 'writable')
    if e is None or e.strip() != 'false':
        fallos.append('A2: `fn writable` de %s no es `false`' % rel(AJENO))
    s = cuerpo(t, 'escribible')
    if s is None or not re.search(r'puerto\(\)\s*==\s*Some\(p\)', s) or 'return false' not in s:
        fallos.append('A2: `fn escribible` de %s falta, o no niega (`return false`) el puerto del ajeno (`puerto() == Some(p)`)' % rel(AJENO))


def un_disco(fallos):
    n = 0
    for r in fuentes(SRC):
        for m in RX_REGISTRA.finditer(sin_comentarios(leer(r))):
            n += 1
            if m.group(1).strip() != '&AHCI_DISK':
                fallos.append('A3: %s registra `%s` como el disco: solo `&AHCI_DISK`' % (rel(r), m.group(1).strip()))
    return n


def el_lector(fallos):
    n = 0
    for r in fuentes(NTFS):
        if os.path.basename(r) == 'pruebas.rs':
            continue
        n += 1
        t = sin_comentarios(leer(r))
        for p in ('.write(', '.flush('):
            if p in t:
                fallos.append('A4: %s llama a `%s` del dispositivo: el lector de NTFS solo lee' % (rel(r), p))
    if n == 0:
        fallos.append('A4: falta %s' % rel(NTFS))
    return n


def c_no(fallos):
    for r in fuentes(DISCO):
        if re.search(r'Nvme|NVMe|nvme', sin_comentarios(leer(r))):
            fallos.append('A5: %s nombra el NVMe: C: ni se mira' % rel(r))
    for r in [os.path.join(K, 'Cargo.toml')] + fuentes(SRC):
        t = sin_comentarios(leer(r)) if r.endswith('.rs') else '\n'.join(l.split('#', 1)[0] for l in leer(r).split('\n'))
        if re.search(r'bmo[-_]nvme', t):
            fallos.append('A5: %s enlaza un driver NVMe' % rel(r))


def main():
    fallos = []
    n_escrituras = cerrojo_2(fallos)
    cerrojo_1(fallos)
    n_registros = un_disco(fallos)
    n_ntfs = el_lector(fallos)
    c_no(fallos)
    if fallos:
        print('FAIL: el disco PERSONAL o C: quedaron al alcance por %d sitio(s)' % len(fallos))
        for f in fallos:
            print('  ' + f)
        return 1
    print('clean: %d escrituras AHCI, todas tras `ajeno::escribible`; el disco ajeno no sabe escribir (ReadOnly, writable=false); %d registro(s) de disco, solo AHCI_DISK; bmo-ntfs (%d ficheros) solo lee; C: (NVMe) ni se nombra'
          % (n_escrituras, n_registros, n_ntfs))
    return 0


if __name__ == '__main__':
    if '--check' not in sys.argv[1:]:
        print(__doc__)
        sys.exit(0)
    sys.exit(main())
