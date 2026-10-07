#!/usr/bin/env python3
"""hilos-casa -- cada `unsafe impl Sync` de PROTON-X dice QUE le pasa el dia de los doce directores.

Por que existe
==============

H0.2 de `docs/plan/PLAN_LAS_TRES_GRANDES.md` (seccion 4), en el orden de
`docs/plan/PLAN_LOS_DOCE_DIRECTORES.md`: los hilos de Windows de la casa son
COOPERATIVOS en una tarea (`hilos.rs`), y cada `unsafe impl Sync` de la casa
y de la app lo da por hecho: "una tarea, hilos cooperativos". El dia que dos
hilos corran A LA VEZ (H1, los doce directores), cada uno es un sitio donde
dos `&mut` del mismo estado chocan. El plan contaba 42; el 07-10 eran 72.

Se leyeron los 72 y se etiquetaron con su clase. Este guardian hace que la
etiqueta no caduque: un `Sync` nuevo sin clase para el build, y lo PENDIENTE
(`cerrojo` y `por-hilo`) es un trinquete -- solo puede bajar.

    [hilos] uno       solo se lee, o se llena UNA vez al empezar: no choca
                      (con SMP, lo de "una vez" va con un Once)
    [hilos] cerrojo   estado de todo el proceso que tocan los hilos del
                      juego: necesita un cerrojo (H2.1)
    [hilos] por-hilo  es de CADA hilo (errno, la excepcion en vuelo, el
                      bufer de inet_ntoa): pasa al hilo (su TEB) (H2.1)
    [hilos] hecho     ya tiene su cerrojo o ya es de cada hilo

La etiqueta va en una linea `// [hilos] clase -- motivo` justo encima del
`unsafe impl Sync`. Lo que dice es un hecho del codigo de HOY: si un cambio
lo hace mentir, se cambia, y el diff lo muestra.

    --check   lo que corre el build
    --lista   cada uno con su clase
"""
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..', '..'))
CARPETAS = [
    os.path.join(RAIZ, 'platform', 'shared', 'proton-x-casa', 'src'),
    os.path.join(RAIZ, 'Ultra_userspace', 'apps', 'proton-x', 'src'),
]
BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'LINEA_BASE.txt')
CLASES = ('uno', 'cerrojo', 'por-hilo', 'hecho')
PENDIENTES = ('cerrojo', 'por-hilo')
RX_SYNC = re.compile(r'^\s*unsafe impl Sync for ([A-Za-z_0-9]+)')
RX_TAG = re.compile(r'//\s*\[hilos\]\s+([a-z-]+)')


def censar():
    cuenta = {c: 0 for c in CLASES}
    sin, mal, todos = [], [], []
    for carpeta in CARPETAS:
        for raiz, _, ficheros in os.walk(carpeta):
            for nombre in sorted(ficheros):
                if not nombre.endswith('.rs'):
                    continue
                ruta = os.path.join(raiz, nombre)
                with open(ruta, 'rb') as f:
                    lineas = f.read().decode('utf-8', 'replace').replace('\r\n', '\n').split('\n')
                for n, l in enumerate(lineas, 1):
                    m = RX_SYNC.match(l)
                    if not m:
                        continue
                    sitio = '%s:%d %s' % (os.path.relpath(ruta, RAIZ), n, m.group(1))
                    t = None
                    # La etiqueta, en las lineas de comentario justo encima.
                    k = n - 2
                    while k >= 0 and lineas[k].strip().startswith('//'):
                        t = RX_TAG.search(lineas[k]) or t
                        k -= 1
                    if not t:
                        sin.append(sitio)
                    elif t.group(1) not in CLASES:
                        mal.append(sitio + ' -> ' + t.group(1))
                    else:
                        cuenta[t.group(1)] += 1
                        todos.append((t.group(1), sitio))
    return cuenta, sin, mal, todos


def main():
    if '--check' not in sys.argv and '--lista' not in sys.argv:
        print(__doc__)
        return 2
    cuenta, sin, mal, todos = censar()
    if '--lista' in sys.argv:
        for clase, sitio in sorted(todos):
            print('%-9s %s' % (clase, sitio))
    if sin:
        print('unsafe impl Sync en PROTON-X SIN su clase (// [hilos] %s -- motivo):' % '|'.join(CLASES))
        for s in sin:
            print('    ' + s)
        return 1
    if mal:
        print('unsafe impl Sync con una clase que no existe:')
        for s in mal:
            print('    ' + s)
        return 1
    if not os.path.exists(BASE):
        print('no hay LINEA_BASE.txt: este guardian no sabe contra que comparar')
        return 1
    with open(BASE) as f:
        base = int(f.read().strip() or '0')
    pendientes = sum(cuenta[c] for c in PENDIENTES)
    total = sum(cuenta.values())
    if pendientes > base:
        print('lo PENDIENTE SUBIO: %d Sync de PROTON-X necesitan cerrojo o ser de cada hilo, y la linea base es %d.'
              % (pendientes, base))
        print('  Cada uno es un choque el dia de los doce directores (PLAN_LOS_DOCE_DIRECTORES, H2.1).')
        print('  O se hace con su cerrojo (y es `hecho`), o se SUBE la cifra a mano en')
        print('  toolchain/tools/hilos-casa/LINEA_BASE.txt diciendo por que en el commit.')
        return 1
    print('clean: %d unsafe impl Sync en PROTON-X y todos dicen su clase -- uno %d  cerrojo %d  '
          'por-hilo %d  hecho %d (pendientes %d, base %d: solo puede bajar)'
          % (total, cuenta['uno'], cuenta['cerrojo'], cuenta['por-hilo'], cuenta['hecho'], pendientes, base))
    if pendientes < base:
        print('clean: lo pendiente BAJO de %d a %d -- baja la cifra en LINEA_BASE.txt' % (base, pendientes))
    return 0


if __name__ == '__main__':
    sys.exit(main())
