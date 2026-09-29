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
RE_SUB_R11 = re.compile(r"subq\s+\$0x([0-9a-f]+),\s*%r11")
RE_PUSH = re.compile(r"^\s*[0-9a-f]+:\s+pushq\s", re.M)
RE_ALINEA = re.compile(r"andq\s+\$-0x40,\s*%rsp")
RE_CALL = re.compile(r"callq\s+0x[0-9a-f]+\s+<([^>+]+)>")

RE_KERNEL_PAGES = re.compile(r"pub\(crate\)\s+const\s+KERNEL_STACK_PAGES\s*:\s*u64\s*=\s*(\d+)\s*;")
RE_TASK_PAGES = re.compile(r"pub\(super\)\s+const\s+TASK_STACK_PAGES\s*:\s*u64\s*=\s*(\d+)\s*;")
RE_PUERTA = re.compile(r"_gate\(\s*(?:crate::)?([A-Za-z0-9_:]+)\s+as\s+\*const\s*\(\)\s+as\s+u64\s*(?:,\s*(\d+))?\s*\)")
# La pila IST1 (la de #UD #DF #GP #PF), en el TSS del BSP que monta s1_cpu.
RE_IST1 = re.compile(r"pub\s+const\s+IST1_SIZE\s*:\s*usize\s*=\s*(\d+)\s*;")
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


def bajada(prologo):
    """Cuanto baja `rsp` el prologo, CONTANDO LA SONDA DE PILA (2026-09-28).

    Un marco de mas de una pagina no baja de golpe: LLVM lo baja de 4 KiB en
    4 KiB tocando cada pagina, en bucle (`sub $N, %r11` y luego `sub $0x1000,
    %rsp` hasta llegar) o desenrollado (`sub $0x1000, %rsp` k veces), y cierra
    con `sub $R, %rsp`. Esto antes cogia el PRIMER `sub` a `rsp` -- que en un
    marco con sonda es el `$0x1000` del paso: un marco de 384 KiB salia como 4.
    Es el marco que el 28-09 tumbo la maquina desde Ring 3."""
    r11 = RE_SUB_R11.search(prologo)
    if r11:
        # Bucle: N lo baja entero (el `$0x1000` del paso esta DENTRO de N).
        # El resto es el `sub` que sigue al `jne` del bucle, si lo hay.
        tras = prologo[r11.end():]
        jne = re.search(r"\bjne\s[^\n]*\n([^\n]*)", tras)
        resto = RE_SUB.search(jne.group(1)) if jne else None
        return int(r11.group(1), 16) + (int(resto.group(1), 16) if resto else 0)
    subs = [int(x, 16) for x in RE_SUB.findall(prologo)]
    # Desenrollada: los pasos seguidos de 0x1000, y el primero que no lo es.
    total = 0
    for v in subs:
        total += v
        if v != 0x1000:
            break
    return total


def medir(texto):
    """{simbolo: (marco, callees)}."""
    marcos = {}
    pos = [(m.start(), m.group(2)) for m in RE_FUNC.finditer(texto)]
    for i, (ini, nombre) in enumerate(pos):
        fin = pos[i + 1][0] if i + 1 < len(pos) else len(texto)
        cuerpo = texto[ini:fin]
        prologo = cuerpo[:6000]
        marco = bajada(prologo) + 8 * len(RE_PUSH.findall(prologo)) + 8
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
                # (nombre, ist): una puerta con IST NO baja por la pila de la
                # tarea -- el CPU cambia a la del TSS al entrar.
                puertas.add((m.group(1), int(m.group(2) or 0)))
            for m in RE_HILO.finditer(s):
                hilos.add(m.group(1))
    return topes, puertas, hilos


def leer_ist1():
    p = os.path.join(raiz(), "Ultra_kernel_x86-64", "faggin", "s1_cpu", "src", "descriptors.rs")
    try:
        with open(p, "rb") as fh:
            m = RE_IST1.search(fh.read().decode("utf-8", "replace"))
    except OSError:
        return None
    return int(m.group(1)) if m else None


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
    #
    # ** (2026-09-28) SOLO LAS QUE NO TIENEN IST. #UD #DF #GP #PF van por
    # IST1 (`faults::roja::init`): el CPU salta a la pila del TSS y en la de
    # la tarea no baja ni un byte. Contarlas encima de cada syscall ponia
    # `fault_report` (4 KiB) donde no esta; se miden aparte, contra IST1.
    d_puertas, d_ist = [], []
    for p, ist in sorted(puertas):
        s = simbolo_de(marcos, p)
        if s:
            (d_ist if ist else d_puertas).append((bajar(s, frozenset()), p))
    if not d_puertas or not d_ist:
        print("guardian MUERTO: faltan puertas de la IDT en el binario (sin IST %d, con IST %d)"
              % (len(d_puertas), len(d_ist)))
        return 1
    d_puertas.sort(key=lambda x: -x[0][0])
    (hondo, camino_puerta), puerta = d_puertas[0]
    encima = MARCO_CPU + hondo
    d_ist.sort(key=lambda x: -x[0][0])
    (hondo_ist, camino_ist), puerta_ist = d_ist[0]
    ist1 = leer_ist1()
    if not ist1:
        print("guardian MUERTO: no encuentro IST1_SIZE en faggin/s1_cpu/src/descriptors.rs")
        return 1

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
        print("fallo mas hondo en IST1 (%s): %d bytes de %d" % (puerta_ist, hondo_ist + MARCO_CPU, ist1))
        print("    " + cadena(camino_ist, 6))
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
    # El informe de un fallo es TERMINAL (o mata una tarea y vuelve): no se
    # anida sobre si mismo -- un segundo fallo reentra por ARRIBA de IST1 --,
    # asi que el margen es un octavo de la pila (1 KiB), no una pagina entera.
    total_ist = hondo_ist + MARCO_CPU
    if total_ist > ist1 - ist1 // 8:
        fallos.append("un fallo (%s) baja %d en IST1, que mide %d (IST1_SIZE) con %d de margen: "
                      "el informe SE SALE de su pila" % (puerta_ist, total_ist, ist1, ist1 // 8))
        fallos.append("    " + cadena(camino_ist, 6))
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
          "fallo %d de %d en IST1; %d funciones, %d puertas, %d hilos"
          % (d_sys[0], encima, total_sys, tope_sys, tope_sys - total_sys,
             hilo_hondo, encima, total_hilo, tope_hilo, tope_hilo - total_hilo,
             total_ist, ist1, len(marcos), len(d_puertas) + len(d_ist), len(d_hilos)))
    return 0


# ---------------------------------------------------------------------------
# ** RING 3 (2026-09-28): la pila de cada programa, medida igual.
#
# El 28-09 `Aparato::draw` del director pedia 395.432 B de marco (un
# `[Vertice; MAX_VERTICES]` que crecio con un numero de OTRO modulo) contra una
# pila de Ring 3 de 64 KiB, y la maquina cayo. Nada lo miraba: este guardian
# solo leia el kernel. Dos reglas, porque el grafo no ve las llamadas por
# `dyn` -- y `draw` es justo una (`bmo_verrano::Backend`):
#
#   1. el camino ESTATICO mas hondo desde `_start` + una pagina <= la pila
#   2. NINGUN marco suelto pasa de media pila, lo llame quien lo llame
# ---------------------------------------------------------------------------

RE_USER_STACK = re.compile(r"pub\s+const\s+USER_STACK_SIZE\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+|\d+)\s*;")
# `taller` (F1) desde el 29-09: su L1 bajaba 79.152 B y nadie lo miraba -- un
# script propio sumaba los `subq` y no veia los marcos que se reservan en bucle.
RING3_POR_DEFECTO = ("director", "proton-x", "coste", "sombra", "taller")


def leer_pila_ring3(kernel_src):
    p = os.path.join(kernel_src, "ring0", "mm", "vmm", "verde.rs")
    try:
        with open(p, "rb") as fh:
            m = RE_USER_STACK.search(fh.read().decode("utf-8", "replace"))
    except OSError:
        return None
    return int(m.group(1).replace("_", ""), 0) if m else None


def comprobar_ring3(elfs, objdump, kernel_src, hablar):
    tope = leer_pila_ring3(kernel_src)
    if not tope:
        print("guardian MUERTO: no encuentro USER_STACK_SIZE en mm/vmm/verde.rs")
        return 1
    if not objdump or not os.path.exists(objdump):
        print("guardian MUERTO: no hay llvm-objdump (rustup component add llvm-tools)")
        return 1
    fallos, limpios = [], []
    for elf in elfs:
        nombre = os.path.basename(elf)
        if not os.path.exists(elf):
            print("guardian MUERTO: no existe el programa a medir: " + elf)
            return 1
        texto = desensamblar(objdump, elf)
        if not texto:
            print("guardian MUERTO: llvm-objdump no pudo leer " + elf)
            return 1
        marcos = medir(texto)
        inicio = simbolo_de(marcos, "_start")
        if not inicio:
            print("guardian MUERTO: %s no tiene `_start`" % nombre)
            return 1
        hondo, camino = profundidad(marcos)(inicio, frozenset())
        gordo, gordo_marco = max(((n, m) for n, (m, _c) in marcos.items()), key=lambda x: x[1])
        if hablar:
            print("%s: camino mas hondo %d de %d" % (nombre, hondo, tope))
            print("    " + cadena(camino, 9))
            for n, (m, _c) in sorted(marcos.items(), key=lambda kv: -kv[1][0])[:6]:
                print("    %6d  %s" % (m, legible(n)))
        if hondo > tope - MARGEN:
            fallos.append("%s: desde `_start` baja %d, y la pila de Ring 3 son %d (USER_STACK_SIZE) con %d "
                          "de margen: SE SALE POR EL FONDO" % (nombre, hondo, tope, MARGEN))
            fallos.append("    " + cadena(camino, 9))
        for n, (m, _c) in sorted(marcos.items(), key=lambda kv: -kv[1][0]):
            if m <= tope // 2:
                break
            fallos.append("%s: el marco de %s pide %d, mas de media pila de Ring 3 (%d): una llamada "
                          "por `dyn` a el no sale en el grafo y se come la pila" % (nombre, legible(n), m, tope))
        limpios.append("%s %d (marco mayor %d)" % (nombre, hondo, gordo_marco))
    if fallos:
        for f in fallos:
            print(f)
        return 1
    print("clean: Ring 3, pila de %d: %s" % (tope, "; ".join(limpios)))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true", help="modo build: 0 cabe, 1 si no")
    ap.add_argument("--elf", default=elf_por_defecto(), help="el kernel a medir (ELF)")
    ap.add_argument("--objdump", default=objdump_por_defecto(), help="llvm-objdump")
    ap.add_argument("--fuente", default=os.path.join(raiz(), "Ultra_kernel_x86-64", "kernel", "src"),
                    help="el fuente del kernel, para leer los topes y las puertas")
    ap.add_argument("--ring3", nargs="*", metavar="ELF",
                    help="medir programas de Ring 3 en vez del kernel (sin nombres: los del build)")
    a = ap.parse_args()
    if a.ring3 is not None:
        base = os.path.join(raiz(), "Ultra_userspace", "target", "x86_64-unknown-none", "release")
        elfs = a.ring3 or [os.path.join(base, n) for n in RING3_POR_DEFECTO]
        sys.exit(comprobar_ring3(elfs, a.objdump, a.fuente, hablar=not a.check))
    sys.exit(comprobar(a.elf, a.objdump, a.fuente, hablar=not a.check))


if __name__ == "__main__":
    main()
