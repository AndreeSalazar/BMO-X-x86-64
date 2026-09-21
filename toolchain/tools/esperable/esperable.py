"""esperable -- nadie concede `RIGHT_WAIT` sobre un objeto que `WAIT` no sabe esperar.

== De donde sale, y con fecha ==

`WAIT` es el segundo de los dos syscalls congelados de BMO-X: bloquea hasta que
la secuencia de un esperable pase de la que el llamante vio, o venza el plazo.
El despachador (`syscall/mod.rs::wait`) tiene un brazo por cada `KIND_` que sabe
esperar. Y el derecho de esperar se concede aparte, en cada objeto, con
`cap::grant(pid, KIND_X, ... | RIGHT_WAIT, ...)`.

El 2026-09-20 (`PLAN_LA_VIDA_UTIL` 3b) las dos mitades no decian lo mismo:
`obj/file.rs` concedia `RIGHT_WAIT` sobre `KIND_ARCHIVO` y prometia en su
cabecera que *"el `wait` sabe hacerlo"*, y el despachador lo mandaba a
`unsupported()`. Un derecho que no se puede ejercer no es un derecho: es una
promesa escrita contra un mecanismo que no existe. Y `WAIT` ya habia mentido
una vez con fecha (`01c09d94`: no bloqueo NUNCA hasta que alguien lo miro).

== Que comprueba ==

  1. Cada `grant(...)` del kernel que lleve `RIGHT_WAIT` nombra un `KIND_` que
     tiene su brazo en `wait()` (`r.kind == cap::KIND_X` o
     `resolved.kind == cap::KIND_X`).
  2. Al reves se AVISA, no se para: un brazo sin ningun `grant` con
     `RIGHT_WAIT` es codigo que nadie puede alcanzar, y se dice.

Los dos lados se leen del fuente. No hay lista de kinds aqui: si manana aparece
`KIND_MEMORIA` con su brazo y su `grant`, pasa solo.

== Antes de juzgar, demuestra que ve ==

Sin `fn wait(` en el despachador, sin un brazo o sin un `grant` con
`RIGHT_WAIT` no ha mirado nada: MUERTO, no "limpio".
"""

import argparse
import os
import re
import sys

RE_GRANT = re.compile(r"grant\s*\(\s*([^;]*?)\)", re.S)
RE_KIND = re.compile(r"\bKIND_[A-Z_]+\b")
RE_BRAZO = re.compile(r"\.kind\s*==\s*(?:cap::)?(KIND_[A-Z_]+)")


def raiz():
    return os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))


def leer(p):
    with open(p, "rb") as fh:
        return fh.read().decode("utf-8", "replace")


def sin_comentarios(s):
    return "\n".join(l.split("//")[0] for l in s.split("\n"))


def concesiones(kernel_src):
    """{KIND: [fichero:linea]} de cada grant con RIGHT_WAIT."""
    con = {}
    for base, _d, files in os.walk(kernel_src):
        for f in files:
            if not f.endswith(".rs"):
                continue
            p = os.path.join(base, f)
            s = sin_comentarios(leer(p))
            for m in RE_GRANT.finditer(s):
                args = m.group(1)
                if "RIGHT_WAIT" not in args:
                    continue
                kinds = RE_KIND.findall(args)
                linea = s[:m.start()].count("\n") + 1
                rel = os.path.relpath(p, kernel_src).replace("\\", "/")
                if not kinds:
                    con.setdefault("?", []).append("%s:%d" % (rel, linea))
                    continue
                con.setdefault(kinds[0], []).append("%s:%d" % (rel, linea))
    return con


def brazos(despachador):
    s = sin_comentarios(leer(despachador))
    i = s.find("fn wait(")
    if i < 0:
        return None
    # El cuerpo de `wait`: hasta la siguiente `fn ` al nivel del modulo.
    j = s.find("\nfn ", i + 1)
    k = s.find("\nextern ", i + 1)
    fin = min(x for x in (j, k, len(s)) if x > 0)
    cuerpo = s[i:fin]
    return set(RE_BRAZO.findall(cuerpo))


def comprobar(kernel_src, despachador):
    if not os.path.isdir(kernel_src):
        print("guardian MUERTO: no existe el fuente del kernel: " + kernel_src)
        return 1
    b = brazos(despachador) if os.path.exists(despachador) else None
    if b is None:
        print("guardian MUERTO: no encuentro `fn wait(` en " + despachador)
        return 1
    if not b:
        print("guardian MUERTO: `wait()` no tiene ni un brazo `.kind == KIND_`")
        return 1
    con = concesiones(kernel_src)
    if not con:
        print("guardian MUERTO: ni un `grant` con RIGHT_WAIT en el kernel")
        return 1
    fallos = []
    if "?" in con:
        fallos.append("grant con RIGHT_WAIT sin un KIND_ que este juez pueda leer: " + ", ".join(con["?"]))
    for kind, sitios in sorted(con.items()):
        if kind == "?":
            continue
        if kind not in b:
            fallos.append("%s se concede con RIGHT_WAIT y `wait()` NO sabe esperarlo (unsupported): %s"
                          % (kind, ", ".join(sitios)))
    huerfanos = sorted(k for k in b if k not in con)
    if fallos:
        for f in fallos:
            print(f)
        return 1
    aviso = ""
    if huerfanos:
        aviso = " -- [!] brazo(s) sin ningun grant que los alcance: " + ", ".join(huerfanos)
    print("clean: %d kind(s) con RIGHT_WAIT (%s) y todos tienen brazo en wait(); %d brazo(s)%s"
          % (len(con), ", ".join(sorted(con)), len(b), aviso))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--check", action="store_true", help="modo build: 0 limpio, 1 si no")
    r = raiz()
    ap.add_argument("--fuente", default=os.path.join(r, "Ultra_kernel_x86-64", "kernel", "src"))
    ap.add_argument("--wait", default=os.path.join(r, "Ultra_kernel_x86-64", "kernel", "src", "ring0", "syscall", "mod.rs"))
    a = ap.parse_args()
    sys.exit(comprobar(a.fuente, a.wait))


if __name__ == "__main__":
    main()
