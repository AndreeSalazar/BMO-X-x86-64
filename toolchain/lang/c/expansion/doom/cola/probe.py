"""Probe BMO C against DOOM, one translation unit at a time.

Why one at a time instead of the unity build first: a unity build stops at its
first error and tells you exactly one thing. Compiling the 81 core files
separately gives 81 first-errors in a single run, which is a distribution --
what breaks most often, and what breaks only once.

The stub headers in include/standards/C are reached through BMO_MODS, the same
override mechanism the toolchain already documents. They declare and implement
nothing: their only job is that an undeclared identifier cannot be mistaken for
a missing language feature.

Usage:  python probe.py [--unity]
"""

import os
import re
import subprocess
import sys
import collections

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = r"C:\Users\Salazar\Documents\BMO"
CFRONT = os.path.join(REPO, "target", "release", "bmo-c-front.exe")
DOOM = os.path.join(os.path.dirname(HERE), "doom", "doomgeneric", "doomgeneric")
INCLUDE = os.path.join(HERE, "include")
OUT = os.path.join(HERE, "out")

# Platform backends the port replaces with its own doomgeneric_bmo.c, plus the
# sound backends that pull in SDL. Excluded because their errors would be about
# X11 and SDL, not about DOOM.
SKIP_PREFIX = ("doomgeneric_",)
SKIP = {"i_allegromusic.c", "i_allegrosound.c", "i_sdlmusic.c", "i_sdlsound.c",
        "icon.c", "i_cdmus.c"}

# Buckets, most specific first: the first pattern that matches wins.
BUCKETS = [
    ("no linking (expected)", r"no hay enlazado|no linking|simbolo no definido|undefined symbol"),
    ("#include not found", r"#include: file not found"),
    ("unsupported declaration", r"declarac|declaration"),
    ("unsupported type", r"\btipo\b|\btype\b"),
    ("unsupported statement", r"sentencia|statement"),
    ("unsupported expression", r"expresion|expression"),
    ("preprocessor", r"#define|#if|macro|preprocesador"),
    ("parse error", r"esperaba|expected|inesperado|unexpected"),
]


def bucket(msg):
    for name, pat in BUCKETS:
        if re.search(pat, msg, re.I):
            return name
    return "other"


def compile_one(path, out_name):
    env = dict(os.environ)
    env["BMO_MODS"] = INCLUDE
    proc = subprocess.run(
        [CFRONT, path, "-o", os.path.join(OUT, out_name)],
        capture_output=True, text=True, env=env, cwd=REPO, timeout=300,
    )
    err = (proc.stderr or "").strip().splitlines()
    first = err[0] if err else ""
    return proc.returncode, first


def main():
    os.makedirs(OUT, exist_ok=True)
    if not os.path.exists(CFRONT):
        sys.exit("build the compiler first: cargo build -p bmo-c-front --release")

    if "--unity" in sys.argv:
        unity = os.path.join(DOOM, "bmo_unity.c")
        code, first = compile_one(unity, "unity.bef")
        print("unity build ->", "OK" if code == 0 else first)
        return

    files = sorted(f for f in os.listdir(DOOM)
                   if f.endswith(".c") and not f.startswith(SKIP_PREFIX)
                   and f not in SKIP)

    # "no main" is not a compiler failure here: a translation unit that is not
    # the one holding main() is not supposed to have one. It means the whole
    # file parsed and reached codegen, which is the thing being measured.
    NO_ENTRY = "no hay funcion 'main'"

    ok, fails = [], []
    for f in files:
        code, first = compile_one(os.path.join(DOOM, f), f[:-2] + ".bef")
        if code == 0 or NO_ENTRY in first:
            ok.append(f)
        else:
            fails.append((f, first))
        print(".", end="", flush=True)
    print()

    print("\n%d of %d translation units parse and reach codegen.\n"
          % (len(ok), len(files)))
    print("   " + " ".join(sorted(ok)) + "\n")

    tally = collections.Counter(bucket(m) for _, m in fails)
    for name, n in tally.most_common():
        print("%4d  %s" % (n, name))

    print("\n-- first error per file " + "-" * 40)
    for f, m in fails:
        print("%-16s %s" % (f, m[:110]))


main()
