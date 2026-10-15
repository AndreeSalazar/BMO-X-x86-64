"""perfil-campos -- el guardian que compara CADA perfil con el codigo, campo a campo.

Por que existe
==============

`perfil` comprueba que cada perfil diga a quien expone y que ese alguien exista.
Eso es la FLECHA. Esto es el CONTENIDO: que lo que el perfil AFIRMA sea lo que el
codigo dice.

`perfil-placa` ya lo hacia para la placa. Este cubre los otros cinco -- y lo
primero que hizo falta al escribirlo fue mirar, uno por uno, QUE SE PUEDE
COMPARAR DE VERDAD. La respuesta no fue "todo".

Lo que se puede comparar, y lo que NO
======================================

    CPU     fabricante y microarquitectura SI: son constantes del perfil
            ** nucleos, hilos, ccx y ccd NO: el codigo los MIDE en el arranque
               (`t.total_cores`), no los declara. Y eso esta BIEN -- un numero
               medido no necesita que un fichero de texto lo confirme; lo que
               necesita es que nadie lo suponga. Aqui se dice para que la
               ausencia de comprobacion no se lea como un descuido
            ** y desde el 2026-09-18 la TABLA DE EXTENSIONES, contra
               `cpu_vendor/features/`: las filas son exactamente los nombres
               del censo (`Feat::name`), lo que `usage.rs` declara `Yes` tiene
               que estar `si` en `esperado` y no estar `no` en `visto`, y se
               cuentan las filas sin foto. Es la primera tabla de PERFIL/ que
               lee una maquina, y existe para que un emisor pueda preguntar

    GPU     `pci_vendor` y que `pci_devices` siga VACIO. Lo segundo importa mas
            que lo primero: en cuanto alguien meta una SKU ahi sin tarjeta
            delante, `claims()` dira que si a algo que nadie ha visto

    GPU_3060  (desde el 2026-09-28) `pci_vendor` y `chipset` contra
            `lectura/identidad.rs` del crate ga10x, y que `pci_device` este
            en su lista `DISPOSITIVOS`. ** Los relojes, la banda y el resto
            NO: se MIDEN con docs/metal/sonda_cuda, y un numero medido no se
            compara con codigo

    RED     `pci_vendor` SI, contra `VENDOR_REALTEK` del driver
            ** `pci_device` NO: el driver no lo declara como constante. Se
               comprueba contra el PCI en el arranque, no aqui

    DISCO   ** LOS TRES CIERRES DE `discos.ps1`, y es la comprobacion mas
            importante de todo el repo. No compara un numero: comprueba que
            siguen ESTANDO. Si alguien quita uno, el build para

    RAM     NADA. No hay un solo campo suyo que el codigo declare -- su unico
            numero de verdad se MIDE con la orden `banda`, y no se ha medido.
            Se cuenta como profile sin comprobar, y se DICE

** Un guardian que solo informa de lo que comprueba deja creer que comprueba
todo. Por eso el renglon limpio de este dice las dos cifras: cuantos campos se
compararon y cuantos perfiles no tienen ni uno que comparar.
"""

import argparse
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
PERFIL = os.path.join(RAIZ, "PERFIL")
K = os.path.join(RAIZ, "Ultra_kernel_x86-64", "kernel", "src", "ring0")

# (campo del perfil, fichero de codigo, regex que saca el valor, como comparar)
COMPARABLES = {
    "CPU.txt": [
        ("fabricante", os.path.join(K, "cpu_vendor", "ryzen_5_5600x", "mod.rs"),
         r'vendor:\s*"([^"]*)"', "texto"),
        ("microarquitectura", os.path.join(K, "cpu_vendor", "ryzen_5_5600x", "mod.rs"),
         r'microarch:\s*"([^"]*)"', "texto"),
    ],
    "GPU.txt": [
        ("pci_vendor", os.path.join(RAIZ, "platform", "drivers", "gpu", "rdna4", "src", "lib.rs"),
         r"pci_vendor:\s*(0x[0-9A-Fa-f]+)", "numero"),
    ],
    # The card that IS in the box. `pci_device` is checked apart (below): the
    # code keeps a LIST of accepted devices, not one constant.
    "GPU_3060.txt": [
        ("pci_vendor", os.path.join(RAIZ, "platform", "drivers", "gpu", "ga10x", "src", "lectura", "identidad.rs"),
         r"pub const NVIDIA:\s*u16\s*=\s*(0x[0-9A-Fa-f]+)", "numero"),
        ("chipset", os.path.join(RAIZ, "platform", "drivers", "gpu", "ga10x", "src", "lectura", "identidad.rs"),
         r"pub const CHIPSET:\s*u16\s*=\s*(0x[0-9A-Fa-f]+)", "numero"),
    ],
    "ENTRADA.txt": [
        ("latido_ms", os.path.join(K, "dev", "usb", "bus.rs"),
         r"const BUS_PERIOD_MS:\s*u64\s*=\s*(\d+)", "decimal"),
        ("subclase_exigida",
         os.path.join(RAIZ, "platform", "drivers", "usb", "uhid", "src", "enumera.rs"),
         r"pub const SUBCLASE_BOOT:\s*u8\s*=\s*(\d+)", "decimal"),
        # ** `distribucion` NO se compara: es una TABLA de scancodes, no una
        # constante. Y es justo el campo cuyo fallo no da error nunca. Se dice
        # aqui para que su ausencia no se lea como que esta cubierto.
    ],
    "RED.txt": [
        ("pci_vendor", os.path.join(K, "red", "mod.rs"),
         r"const VENDOR_REALTEK:\s*u16\s*=\s*(0x[0-9A-Fa-f]+)", "numero"),
    ],
}

# Perfiles que no tienen NI UN campo comparable, y por que. Estar aqui es una
# respuesta escrita, no un olvido.
SIN_COMPARAR = {
    "RAM.txt": "su unico numero de verdad se MIDE con `banda`, y no se ha medido",
    # DISCO no se comprueba por campos: se comprueba por CIERRES, abajo. Y esa
    # comprobacion es la mas importante de todo el repo.
    "DISCO.txt": "no se comprueba por campos sino por los tres CIERRES de discos.ps1",
}

# El de la placa tiene guardian propio.
SUYO = {"PERFIL.txt"}

# *** LOS TRES CIERRES DEL DESPLIEGUE. No es un campo: es que sigan existiendo.
DISCOS = os.path.join(RAIZ, "Ultra_kernel_x86-64", "build", "discos.ps1")
CIERRES = [
    ("rechaza la unidad del sistema", r"\$env:SystemDrive"),
    ("exige FAT o FAT32", r"'FAT32'"),
    ("exige que la unidad exista", r"Test-Path"),
]


# -- CPU: la tabla de extensiones contra el censo del kernel ----------------
FEATURES = os.path.join(K, "cpu_vendor", "features")


def extensiones_del_perfil(texto):
    """Las filas `nombre | esperado | visto | mano | llvm` de CPU.txt, en orden.
    Las dos ultimas son la DECISION del propietario (B4, 14-10)."""
    filas = []
    for m in re.finditer(r"^\s*(\S[^|]*?)\s*\|\s*(si|no)\s*\|\s*(si|no|\?)\s*\|\s*(si|no|-)\s*\|\s*(si|no|-)\s*$", texto, re.M):
        filas.append(m.groups())
    return filas


# -- B4: la DECISION del propietario, a codigo ------------------------------
CATALOGO = os.path.join(RAIZ, "platform", "shared", "bmo-carga-juicio", "src", "instrucciones.rs")
DECIDIDO = os.path.join(RAIZ, "platform", "shared", "bmo-carga-juicio", "src", "decidido.rs")


def catalogo():
    """`[(bit, nombre)]` del catalogo de lo que un `.bex` declara."""
    with open(CATALOGO, "r", encoding="utf-8") as fh:
        return [(int(k), n) for k, n in re.findall(r'^\s*e\((\d+),\s*"([^"]+)"', fh.read(), re.M)]


def decidido_rs(filas):
    """El texto de `decidido.rs`: las dos columnas como mascaras del catalogo."""
    dec = {n: (mano, llvm) for n, _, _, mano, llvm in filas}
    def mascara(col):
        bits = [(k, n) for k, n in catalogo() if dec.get(n, ("-", "-"))[col] == "si"]
        if not bits:
            return "0 /* ninguna */"
        return " | ".join("1 << %d" % k for k, _ in bits) + " /* " + ", ".join(n for _, n in bits) + " */"
    return (
        "//! **LO QUE DECIDIO EL PROPIETARIO** (B4 de `docs/plan/EL_FOCO.md`):\n"
        "//! las columnas `mano` y `llvm` de la tabla de extensiones de\n"
        "//! `PERFIL/CPU.txt`, como mascaras de [`crate::instrucciones`].\n"
        "//!\n"
        "//! ** GENERADO por `toolchain/tools/perfil-campos/perfil_campos.py --apply`.\n"
        "//! No se escribe a mano: se cambia el perfil y se regenera. El guardian\n"
        "//! falla si este fichero y el perfil no dicen lo mismo.\n"
        "\n"
        "/// Lo que el codigo de la CASA puede usar: `intrinsics.toml` y el `asm!` de\n"
        "/// los crates. `sem-asm` no carga una fila fuera de esto y `bex-link` para\n"
        "/// el build de un programa de Rust que lo lleve.\n"
        "pub const MANO: u64 = %s;\n"
        "\n"
        "/// Lo que el COMPILADOR (LLVM) puede elegir por su cuenta. Lo exige el\n"
        "/// guardian `llvm-obedece` en cada objetivo de Rust.\n"
        "pub const LLVM: u64 = %s;\n"
        "\n"
        "const _: () = assert!(LLVM & !MANO == 0, \"LLVM no elige lo que la casa no puede\");\n"
    ) % (mascara(0), mascara(1))


def comprobar_decision(filas, quejas, aplicar):
    """Las reglas de las dos columnas, y `decidido.rs` igual al perfil."""
    cat = [n for _, n in catalogo()]
    por = {f[0]: f for f in filas}
    for n in cat:
        if n not in por:
            quejas.append("CPU: el catalogo de `bmo-carga-juicio` tiene %s y el perfil no tiene esa fila -- "
                          "un nombre, una forma de escribirlo" % n)
    for n, esperado, _, mano, llvm in filas:
        if n in cat:
            if mano == "-" or llvm == "-":
                quejas.append("CPU: %s esta en el catalogo: el propietario tiene que decir `si` o `no` en "
                              "`mano` y `llvm`, no `-`" % n)
            if llvm == "si" and mano != "si":
                quejas.append("CPU: %s dice `llvm: si` y `mano: no` -- el compilador no elige lo que la "
                              "casa no puede" % n)
            if mano == "si" and esperado != "si":
                quejas.append("CPU: %s dice `mano: si` y el perfil no espera que el silicio la tenga" % n)
        elif mano != "-" or llvm != "-":
            quejas.append("CPU: %s no esta en el catalogo de `bmo-carga-juicio`: su `mano`/`llvm` no los "
                          "lee nadie, y una decision que nadie lee es una que no se cumple. Va `-`" % n)
    quiere = decidido_rs(filas)
    try:
        with open(DECIDIDO, "r", encoding="utf-8") as fh:
            hay = fh.read()
    except OSError:
        hay = ""
    if hay != quiere:
        if aplicar:
            with open(DECIDIDO, "w", encoding="utf-8", newline="\n") as fh:
                fh.write(quiere)
            print("  decidido.rs reescrito desde PERFIL/CPU.txt")
        else:
            quejas.append("CPU: `bmo-carga-juicio/src/decidido.rs` no dice lo que el perfil decide -- "
                          "`python toolchain/tools/perfil-campos/perfil_campos.py --apply`")


APLICAR = False


def censo_del_kernel():
    """`(nombres en orden, {nombre: usa})` desde `features/mod.rs` y `usage.rs`."""
    with open(os.path.join(FEATURES, "mod.rs"), "r", encoding="utf-8", errors="replace") as fh:
        modrs = fh.read()
    cuerpo = modrs[modrs.index("pub const fn name(self)"):]
    variante_a_nombre = re.findall(r'Feat::(\w+)\s*=>\s*"([^"]+)"', cuerpo)
    with open(os.path.join(FEATURES, "usage.rs"), "r", encoding="utf-8", errors="replace") as fh:
        usage = fh.read()
    usa = {}
    for variante, que in re.findall(r"Feat::(\w+)\s*=>\s*Use::(Yes|No)\(", usage):
        usa[variante] = que == "Yes"
    nombres = [n for _, n in variante_a_nombre]
    usa_por_nombre = {n: usa.get(v, False) for v, n in variante_a_nombre}
    return nombres, usa_por_nombre


def comprobar_extensiones(texto, quejas):
    """Devuelve `(filas comparadas, filas sin foto)`."""
    if not os.path.isdir(FEATURES):
        quejas.append("CPU: no existe cpu_vendor/features -- la tabla de extensiones no tiene con que compararse")
        return 0, 0
    filas = extensiones_del_perfil(texto)
    nombres, usa = censo_del_kernel()
    if not filas:
        quejas.append("CPU: no hay tabla de extensiones (`nombre | esperado | visto | mano | llvm`)")
        return 0, 0
    del_perfil = [f[0] for f in filas]
    if del_perfil != nombres:
        faltan = [n for n in nombres if n not in del_perfil]
        sobran = [n for n in del_perfil if n not in nombres]
        if faltan:
            quejas.append("CPU: el censo del kernel tiene filas que el perfil no: %s" % ", ".join(faltan))
        if sobran:
            quejas.append("CPU: el perfil tiene filas que el censo del kernel no: %s" % ", ".join(sobran))
        if not faltan and not sobran:
            quejas.append("CPU: las filas de extensiones no van en el orden del censo (`features::Feat`)")
        return 0, 0
    comprobar_decision(filas, quejas, APLICAR)
    sin_foto = 0
    for nombre, esperado, visto, _, _ in filas:
        if visto == "?":
            sin_foto += 1
        if usa[nombre] and esperado != "si":
            quejas.append("CPU: `usage.rs` dice que BMO USA %s y el perfil espera que no este" % nombre)
        if usa[nombre] and visto == "no":
            quejas.append("CPU: `usage.rs` dice que BMO USA %s y el Ryzen dijo que NO lo tiene -- "
                          "la mentira esta en usage.rs, no aqui" % nombre)
    return len(filas), sin_foto


# -- CPU: la tabla de caches contra `cache.rs::esperado_5600x` -------------
CACHE_RS = os.path.join(K, "cpu_vendor", "ryzen_5_5600x", "cache.rs")
CACHES = ("L1d", "L1i", "L2", "L3")


def caches_del_perfil(texto):
    """`{nombre: (kib, linea, vias, hilos, visto)}` de la tabla de CPU.txt."""
    filas = {}
    patron = r"^\s*(L1d|L1i|L2|L3)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(si|no|\?)\s*$"
    for m in re.finditer(patron, texto, re.M):
        filas[m.group(1)] = (int(m.group(2)), int(m.group(3)), int(m.group(4)), int(m.group(5)), m.group(6))
    return filas


def caches_del_kernel():
    """`{nombre: (kib, linea, vias, hilos)}` de `esperado_5600x()`. La linea es
    64 en todas (`fila()` la fija) y el medida puede venir como `32 * 1024`."""
    with open(CACHE_RS, "r", encoding="utf-8", errors="replace") as fh:
        texto = fh.read()
    cuerpo = texto[texto.index("pub const fn esperado_5600x()"):]
    linea = int(re.search(r"line_size_bytes:\s*(\d+)", texto).group(1))
    filas = {}
    for campo_rs, nombre in (("l1d", "L1d"), ("l1i", "L1i"), ("l2", "L2"), ("l3", "L3")):
        m = re.search(r"%s:\s*Some\(fila\(\d+,\s*([\d\s\*]+?),\s*(\d+),\s*(\d+)," % campo_rs, cuerpo)
        if m:
            kib = 1
            for trozo in m.group(1).split("*"):
                kib *= int(trozo.strip())
            filas[nombre] = (kib, linea, int(m.group(2)), int(m.group(3)))
    return filas


def comprobar_caches(texto, quejas):
    """Devuelve `(filas comparadas, filas sin foto)`."""
    if not os.path.exists(CACHE_RS):
        quejas.append("CPU: no existe cpu_vendor/ryzen_5_5600x/cache.rs -- la tabla de caches no tiene con que compararse")
        return 0, 0
    perfil = caches_del_perfil(texto)
    kernel = caches_del_kernel()
    comparadas, sin_foto = 0, 0
    for nombre in CACHES:
        if nombre not in perfil:
            quejas.append("CPU: la tabla de caches no tiene la fila %s" % nombre)
            continue
        if nombre not in kernel:
            quejas.append("CPU: `esperado_5600x` ya no declara %s -- o se renombro, o se quito" % nombre)
            continue
        kib, linea, vias, hilos, visto = perfil[nombre]
        if (kib, linea, vias, hilos) != kernel[nombre]:
            quejas.append("CPU: la cache %s dice %s en el perfil y %s en `esperado_5600x`"
                          % (nombre, (kib, linea, vias, hilos), kernel[nombre]))
            continue
        if visto == "no":
            quejas.append("CPU: el Ryzen contesto OTRA cosa para %s (`visto: no`) y lo esperado "
                          "sigue igual -- corrige la fila y `esperado_5600x` con los numeros de la foto" % nombre)
            continue
        if visto == "?":
            sin_foto += 1
        comparadas += 1
    return comparadas, sin_foto


def campo(texto, nombre):
    # Los campos van indentados en los perfiles: `  fabricante: AMD`.
    m = re.search(r"^\s*%s:\s*(.+?)\s*$" % re.escape(nombre), texto, re.M)
    return m.group(1) if m else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--apply", action="store_true", help="reescribe decidido.rs desde el perfil")
    args = ap.parse_args()
    global APLICAR
    APLICAR = args.apply

    if not os.path.isdir(PERFIL):
        print("la carpeta NO EXISTE: falta PERFIL/")
        return 1 if args.check else 0

    quejas = []
    comparados = 0
    sin_comparar = 0
    extensiones_sin_foto = 0
    caches_sin_foto = 0

    for dirpath, _, files in os.walk(PERFIL):
        for f in sorted(files):
            if not f.endswith(".txt") or f in SUYO:
                continue
            ruta = os.path.join(dirpath, f)
            with open(ruta, "r", encoding="utf-8", errors="replace") as fh:
                texto = fh.read()

            if f in SIN_COMPARAR:
                sin_comparar += 1
                continue

            reglas = COMPARABLES.get(f)
            if reglas is None:
                # Un perfil nuevo sin reglas ni excusa escrita. Se dice: la
                # alternativa es que aparezcan perfiles que nadie comprueba y
                # que nadie sepa que nadie los comprueba.
                quejas.append(
                    "%s no tiene ni reglas de comparacion ni una linea en "
                    "SIN_COMPARAR -- decide cual de las dos es" % f)
                continue

            if f == "CPU.txt":
                n, sin_foto = comprobar_extensiones(texto, quejas)
                comparados += n
                extensiones_sin_foto = sin_foto
                n, sin_foto = comprobar_caches(texto, quejas)
                comparados += n
                caches_sin_foto = sin_foto

            for nombre, fichero, patron, modo in reglas:
                dice = campo(texto, nombre)
                if dice is None:
                    quejas.append("%s no declara `%s`" % (f, nombre))
                    continue
                if not os.path.exists(fichero):
                    quejas.append("%s: %s no existe" % (f, os.path.relpath(fichero, RAIZ)))
                    continue
                with open(fichero, "r", encoding="utf-8", errors="replace") as fh:
                    m = re.search(patron, fh.read())
                if not m:
                    quejas.append(
                        "%s: en %s ya no esta el valor de `%s` -- o se renombro, "
                        "o se quito" % (f, os.path.relpath(fichero, RAIZ), nombre))
                    continue
                codigo = m.group(1)
                if modo == "numero":
                    igual = int(dice, 16) == int(codigo, 16)
                elif modo == "decimal":
                    # El perfil puede llevar una coletilla ("4", "4 ms").
                    igual = dice.split()[0] == codigo
                else:
                    igual = dice == codigo
                if not igual:
                    quejas.append("%s: `%s` dice '%s' y el codigo dice '%s'"
                                  % (f, nombre, dice, codigo))
                else:
                    comparados += 1

    # -- GPU: que `pci_devices` siga VACIO ---------------------------------
    #
    # ** Esta no es una comparacion de campo, es un CIERRE. Una SKU escrita ahi
    # sin tarjeta delante hace que `claims()` diga que si a algo que nadie ha
    # visto, y eso es una mentira que compila.
    gpu = os.path.join(RAIZ, "platform", "drivers", "gpu", "rdna4", "src", "lib.rs")
    if os.path.exists(gpu):
        with open(gpu, "r", encoding="utf-8", errors="replace") as fh:
            if not re.search(r"pci_devices:\s*&\[\s*\]", fh.read()):
                quejas.append(
                    "GPU: `pci_devices` ya NO esta vacio. Si hay tarjeta en el "
                    "banco, actualiza PERFIL/GPU.txt y quita este cierre; si no "
                    "la hay, `claims()` esta reclamando hardware que nadie ha visto")
            else:
                comparados += 1

    # -- GPU_3060: the profiled device must be one the crate accepts -------
    #
    # identidad.rs keeps `DISPOSITIVOS: [u16; N] = [..]`. A profile naming a
    # device outside that list describes a card the crate would refuse.
    profile_3060 = os.path.join(PERFIL, "GPU_3060.txt")
    identity_rs = os.path.join(RAIZ, "platform", "drivers", "gpu", "ga10x", "src", "lectura", "identidad.rs")
    if os.path.exists(profile_3060):
        with open(profile_3060, "r", encoding="utf-8", errors="replace") as fh:
            said = campo(fh.read(), "pci_device")
        accepted = None
        if os.path.exists(identity_rs):
            with open(identity_rs, "r", encoding="utf-8", errors="replace") as fh:
                m = re.search(r"pub const DISPOSITIVOS:\s*\[u16;\s*\d+\]\s*=\s*\[([^\]]*)\]", fh.read())
            if m:
                accepted = [int(x.strip(), 16) for x in m.group(1).split(",") if x.strip()]
        if said is None:
            quejas.append("GPU_3060.txt no declara `pci_device`")
        elif accepted is None:
            quejas.append("GPU_3060: en lectura/identidad.rs ya no esta `DISPOSITIVOS` -- "
                          "o se renombro, o se quito")
        elif int(said.split()[0], 16) not in accepted:
            quejas.append("GPU_3060: `pci_device` dice %s y `DISPOSITIVOS` solo acepta %s"
                          % (said, ", ".join("0x%04X" % d for d in accepted)))
        else:
            comparados += 1

    # -- DISCO: los tres cierres, y es lo mas importante de aqui -----------
    if not os.path.exists(DISCOS):
        quejas.append("DISCO: no existe build/discos.ps1 -- el despliegue no "
                      "tiene los tres cierres porque no tiene fichero")
    else:
        with open(DISCOS, "r", encoding="utf-8", errors="replace") as fh:
            d = fh.read()
        for que, patron in CIERRES:
            if re.search(patron, d):
                comparados += 1
            else:
                quejas.append(
                    "** DISCO: FALTA EL CIERRE '%s' en build/discos.ps1. Ese "
                    "fichero es el unico del repo que puede escribir en el disco "
                    "del propietario -- ver PERFIL/DISCO.txt" % que)

    if quejas:
        print("los perfiles y el codigo NO dicen lo mismo:")
        for q in quejas:
            print("  " + q)
        print("")
        print("  cada perfil esta en PERFIL/ y dice a quien expone. Lo que este")
        print("  guardian puede y NO puede comparar esta en su cabecera.")
        return 1 if args.check else 0

    print("clean: %d campo(s) y cierre(s) comparados con el codigo, y %d perfil(es) "
          "sin nada que comparar (dicho, no olvidado)" % (comparados, sin_comparar))
    if extensiones_sin_foto:
        print("  [i] CPU: %d extension(es) sin foto del Ryzen (`visto: ?`) -- "
              "se rellenan con la orden `ext`" % extensiones_sin_foto)
    if caches_sin_foto:
        print("  [i] CPU: %d cache(s) sin foto del Ryzen (`visto: ?`) -- "
              "se rellenan con la orden `cache`" % caches_sin_foto)
    return 0


if __name__ == "__main__":
    sys.exit(main())
