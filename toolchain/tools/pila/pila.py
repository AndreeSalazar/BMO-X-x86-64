"""pila -- cuanto baja el kernel por cada pila de tarea, medido en el BINARIO.

== De donde sale, y con fecha ==

El 2026-09-20 por la tarde el escritorio empezo a morir al lanzar DOOM:
`faltan 2160/2160 pag desde 0xE0000000`, `se corta en el PD (tabla ... ocupada
TABLA)`, `pantalla MUERTA`. Su PD entero a cero, enlazado, en uso, y NADIE lo
habia soltado. Por la luego DOOM se jugaba. Entre las dos horas entro B7: un
`bmo_hash::hash(indice)` mas en la admision.

Un syscall corre sobre la pila de kernel de la tarea que lo pide, y por un
syscall pasa el cargador ENTERO (`LANZAR` desde el escritorio). Sumando los
marcos del binario: `syscall_entry` + `dispatch` + `lanzar` + `ruta` +
`con_buffer` + `admit_payload_desde` + `bmo_hash::hash` = **14.232 bytes de
16.384**. Cualquier interrupcion encima --el tick, o el aviso del AHCI que llega
justo detras del DMA de la firma, o sea justo antes del hash-- pone su marco
de `iretq`, 15 `push` y el area XSAVE **por debajo del fondo**: en el marco
fisico vecino. En el Ryzen ese vecino era una tabla del escritorio.

Nadie lo vio porque el numero no existia: `MIN_TASK_STACK` comprueba que quepa
UN contexto, no que quepa el trabajo. Este guardian pone el numero donde el
build lo mira.

== Que mide ==

Lee el kernel con `llvm-objdump` y, funcion por funcion, cuanto reserva de
pila (los `push` del prologo, el `sub rsp, N`, la direccion de retorno y la
holgura del `and rsp, -64` si lo hay). Con el grafo de `call` calcula el
camino ESTATICO mas hondo desde:

  1. `syscall_entry`         lo que un syscall puede bajar
  2. cada puerta de la IDT    lo que una interrupcion o un fault ponen ENCIMA
  3. cada hilo de kernel      lo que arranca `spawn_kernel`

Y exige, con UNA pagina de margen (la misma holgura que `MIN_TASK_STACK`):

    syscall + marco de la CPU + la puerta mas honda  <=  KERNEL_STACK_PAGES * 4096 - 4096
    hilo    + marco de la CPU + la puerta mas honda  <=  TASK_STACK_PAGES   * 4096 - 4096

Los dos topes se LEEN del fuente (`task/proc.rs` y `task/scheduler/verde.rs`),
no se copian aqui: subir el numero en el kernel sube el tope del juez.

== Lo que NO ve, dicho para que nadie lo suponga ==

  - Llamadas INDIRECTAS (punteros a funcion, `dyn`): no estan en el grafo.
  - RECURSION: un ciclo se corta al segundo paso.
  - Interrupciones ANIDADAS: se cuenta UNA puerta encima del syscall.

O sea que mide un SUELO, no un techo. Un suelo que ya se sale es un veredicto;
un suelo que cabe con una pagina de margen es lo que hoy se puede prometer.

== Antes de juzgar, demuestra que ve ==

Sin `syscall_entry`, sin una puerta de la IDT, sin un hilo o sin los dos topes
no ha mirado nada: MUERTO, no "limpio".
"""

import argparse
import glob
import os
import re
import subprocess
import sys

PAGINA = 4096
MARGEN = PAGINA
# Lo que la CPU empuja sola al entrar por una puerta: ss, rsp, rflags, cs, rip,
# y el codigo de error cuando lo hay. Seis palabras.
MARCO_CPU = 6 * 8

RE_FUNC = re.compile(r"^([0-9a-f]{16}) <([^>]+)>:$", re.M)
RE_SUB = re.compile(r"subq\s+\$0x([0-9a-f]+),\s*%rsp")
RE_PUSH = re.compile(r"^\s*[0-9a-f]+:\s+pushq\s", re.M)
RE_ALINEA = re.compile(r"andq\s+\$-0x40,\s*%rsp")
RE_CALL = re.compile(r"callq\s+0x[0-9a-f]+\s+<([^>+]+)>")

RE_KERNEL_PAGES = re.compile(r"pub\(crate\)\s+const\s+KERNEL_STACK_PAGES\s*:\s*u64\s*=\s*(\d+)\s*;")
RE_TASK_PAGES = re.compile(r"pub\(super\)\s+const\s+TASK_STACK_PAGES\s*:\s*u64\s*=\s*(\d+)\s*;")
RE_PUERTA = re.compile(r"_gate\(\s*(?:crate::)?([A-Za-z0-9_:]+)\s+as\s+")
RE_HILO = re.compile(r"spawn_kernel\(\s*(?:crate::)?([A-Za-z0-9_:]+)\s+as\s+")


def raiz():
    return os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))


def elf_por_defecto():
    return os.path.join(raiz(), "Ultra_kernel_x86-64", "target", "kernel",
                        "x86_64-unknown-none", "release", "bmo-kernel")


def objdump_por_defecto():
    home = os.path.expanduser("~")
    for patron in ("*/lib/rustlib/*/bin/llvm-objdump.exe", "*/lib/rustlib/*/bin/llvm-objdump"):
        hits = sorted(glob.glob(os.path.join(home, ".rustup", "toolchains", patron)))
        if hits:
            return hits[0]
    return None


def legible(mangled):
    """`_RNvNtNtCs..._10bmo_kernel5ring04task6launch4ruta` -> `bmo_kernel::ring0::task::launch::ruta`.

    Solo lo justo para leer un renglon: los segmentos `<largo><nombre>` que
    siguen al hash del crate. Lo que no entienda se deja como esta."""
    m = re.match(r"^_R.*?Cs[A-Za-z0-9]+_(\d.*)$", mangled)
    if not m:
        return mangled
    resto = m.group(1)
    partes = []
    while resto:
        if resto[0].isdigit():
            n = re.match(r"(\d+)", resto).group(1)
            largo = int(n)
            nombre = resto[len(n):len(n) + largo]
            if not nombre:
                break
            partes.append(nombre)
            resto = resto[len(n) + largo:]
            continue
        # Entre segmento y segmento van marcas de forma (`Nt`, `Nv`, `M`) y
        # referencias hacia atras (`B2_`): se saltan, no nombran nada.
        salto = re.match(r"(B[0-9a-z]*_|[A-Za-z])", resto)
        if not salto:
            break
        resto = resto[salto.end():]
    return "::".join(partes) if partes else mangled


def desensamblar(objdump, elf):
    r = subprocess.run([objdump, "-d", "--no-show-raw-insn", elf], capture_output=True)
    if r.returncode != 0:
        return None
    return r.stdout.decode("utf-8", "replace")


def medir(texto):
    """{simbolo: (marco, callees)}."""
    marcos = {}
    pos = [(m.start(), m.group(2)) for m in RE_FUNC.finditer(texto)]
    for i, (ini, nombre) in enumerate(pos):
        fin = pos[i + 1][0] if i + 1 < len(pos) else len(texto)
        cuerpo = texto[ini:fin]
        prologo = cuerpo[:6000]
        subs = RE_SUB.findall(prologo)
        marco = (int(subs[0], 16) if subs else 0) + 8 * len(RE_PUSH.findall(prologo)) + 8
        if RE_ALINEA.search(prologo):
            marco += 63
        marcos[nombre] = (marco, set(RE_CALL.findall(cuerpo)))
    return marcos


def profundidad(marcos):
    memo = {}

    def bajar(n, pila):
        if n in memo:
            return memo[n]
        if n not in marcos or n in pila:
            return (0, [])
        marco, callees = marcos[n]
        mejor = (0, [])
        pila = pila | {n}
        for c in callees:
            d = bajar(c, pila)
            if d[0] > mejor[0]:
                mejor = d
        r = (marco + mejor[0], [(n, marco)] + mejor[1])
        memo[n] = r
        return r

    return bajar


def simbolo_de(marcos, corto):
    """El simbolo del binario cuyo camino acaba en `corto` (`timer_entry`, `usb::bus::bus_thread`)."""
    cola = corto.split("::")[-1]
    candidatos = [s for s in marcos if s == cola or legible(s).split("::")[-1] == cola]
    if not candidatos:
        return None
    # Si hay varios con el mismo nombre corto, el que mas se parezca al camino.
    candidatos.sort(key=lambda s: -sum(1 for p in corto.split("::") if p in legible(s).split("::")))
    return candidatos[0]


def leer_fuente(kernel_src):
    topes = {}
    puertas = set()
    hilos = set()
    for base, _dirs, files in os.walk(kernel_src):
        for f in files:
            if not f.endswith(".rs"):
                continue
            p = os.path.join(base, f)
            with open(p, "rb") as fh:
                s = fh.read().decode("utf-8", "replace")
            m = RE_KERNEL_PAGES.search(s)
            if m:
                topes["KERNEL_STACK_PAGES"] = int(m.group(1))
            m = RE_TASK_PAGES.search(s)
            if m:
                topes["TASK_STACK_PAGES"] = int(m.group(1))
            for m in RE_PUERTA.finditer(s):
                puertas.add(m.group(1))
            for m in RE_HILO.finditer(s):
                hilos.add(m.group(1))
    return topes, puertas, hilos


def cadena(camino, cuantos=6):
    return " > ".join("%s(%d)" % (legible(n).split("::")[-1], f) for n, f in camino[:cuantos])


def comprobar(elf, objdump, kernel_src, hablar):
    fallos = []
    if not os.path.exists(elf):
        print("guardian MUERTO: no existe el kernel a medir: " + elf)
        return 1
    if not objdump or not os.path.exists(objdump):
        print("guardian MUERTO: no hay llvm-objdump (rustup component add llvm-tools)")
        return 1
    texto = desensamblar(objdump, elf)
    if not texto:
        print("guardian MUERTO: llvm-objdump no pudo leer " + elf)
        return 1
    marcos = medir(texto)
    topes, puertas, hilos = leer_fuente(kernel_src)
    if "KERNEL_STACK_PAGES" not in topes or "TASK_STACK_PAGES" not in topes:
        print("guardian MUERTO: no encuentro KERNEL_STACK_PAGES / TASK_STACK_PAGES en el fuente")
        return 1
    if not puertas or not hilos:
        print("guardian MUERTO: no encuentro puertas de la IDT (%d) o hilos de kernel (%d) en el fuente"
              % (len(puertas), len(hilos)))
        return 1
    bajar = profundidad(marcos)

    sys_sim = simbolo_de(marcos, "syscall_entry")
    if not sys_sim:
        print("guardian MUERTO: el binario no tiene `syscall_entry`")
        return 1
    d_sys = bajar(sys_sim, frozenset())

    # La puerta mas honda: lo que una interrupcion o un fault ponen encima.
    d_puertas = []
    for p in sorted(puertas):
        s = simbolo_de(marcos, p)
        if s:
            d_puertas.append((bajar(s, frozenset()), p))
    if not d_puertas:
        print("guardian MUERTO: ninguna puerta de la IDT aparece en el binario")
        return 1
    d_puertas.sort(key=lambda x: -x[0][0])
    (hondo, camino_puerta), puerta = d_puertas[0]
    encima = MARCO_CPU + hondo

    d_hilos = []
    for h in sorted(hilos):
        s = simbolo_de(marcos, h)
        if s:
            d_hilos.append((bajar(s, frozenset()), h))
    if not d_hilos:
        print("guardian MUERTO: ningun hilo de `spawn_kernel` aparece en el binario")
        return 1
    d_hilos.sort(key=lambda x: -x[0][0])
    (hilo_hondo, camino_hilo), hilo = d_hilos[0]

    tope_sys = topes["KERNEL_STACK_PAGES"] * PAGINA
    tope_hilo = topes["TASK_STACK_PAGES"] * PAGINA
    total_sys = d_sys[0] + encima
    total_hilo = hilo_hondo + encima

    if hablar:
        print("syscall mas hondo: %d bytes" % d_sys[0])
        print("    " + cadena(d_sys[1], 9))
        print("puerta mas honda (%s): %d bytes + %d de la CPU" % (puerta, hondo, MARCO_CPU))
        print("    " + cadena(camino_puerta, 6))
        print("hilo mas hondo (%s): %d bytes" % (hilo, hilo_hondo))
        print("    " + cadena(camino_hilo, 6))
        print("marcos mas gordos:")
        for n, (m, _c) in sorted(marcos.items(), key=lambda kv: -kv[1][0])[:12]:
            print("    %6d  %s" % (m, legible(n)))
        print()

    if total_sys > tope_sys - MARGEN:
        fallos.append("un syscall baja %d + %d de interrupcion = %d, y la pila de tarea de Ring 3 son %d "
                      "(KERNEL_STACK_PAGES=%d) con %d de margen: SE SALE POR EL FONDO"
                      % (d_sys[0], encima, total_sys, tope_sys, topes["KERNEL_STACK_PAGES"], MARGEN))
        fallos.append("    " + cadena(d_sys[1], 9))
        fallos.append("    encima: " + cadena(camino_puerta, 5))
    if total_hilo > tope_hilo - MARGEN:
        fallos.append("el hilo %s baja %d + %d de interrupcion = %d, y la pila de hilo son %d "
                      "(TASK_STACK_PAGES=%d) con %d de margen: SE SALE POR EL FONDO"
                      % (hilo, hilo_hondo, encima, total_hilo, tope_hilo, topes["TASK_STACK_PAGES"], MARGEN))
        fallos.append("    " + cadena(camino_hilo, 6))

    if fallos:
        for f in fallos:
            print(f)
        return 1
    print("clean: syscall %d + puerta %d = %d de %d (Ring 3, %d libres); hilo %d + puerta %d = %d de %d (%d libres); "
          "%d funciones, %d puertas, %d hilos"
          % (d_sys[0], encima, total_sys, tope_sys, tope_sys - total_sys,
             hilo_hondo, encima, total_hilo, tope_hilo, tope_hilo - total_hilo,
             len(marcos), len(d_puertas), len(d_hilos)))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true", help="modo build: 0 cabe, 1 si no")
    ap.add_argument("--elf", default=elf_por_defecto(), help="el kernel a medir (ELF)")
    ap.add_argument("--objdump", default=objdump_por_defecto(), help="llvm-objdump")
    ap.add_argument("--fuente", default=os.path.join(raiz(), "Ultra_kernel_x86-64", "kernel", "src"),
                    help="el fuente del kernel, para leer los topes y las puertas")
    a = ap.parse_args()
    sys.exit(comprobar(a.elf, a.objdump, a.fuente, hablar=not a.check))


if __name__ == "__main__":
    main()
