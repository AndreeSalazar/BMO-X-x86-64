#!/usr/bin/env python3
"""Sesiones declaradas y copias medidas; no deduce causas de fallos GPU.

[carril] VERDE -- archivos del anfitrion, JSON en stdout
[consumo] NADA -- consulta explicita
[cuesta] NADA -- una comparacion, sin autoridad de aparato
"""
import argparse
import json
import sys

import inventario as inv

SCHEMA = "bmo-nvidia-session-v1"
CONTEXT = ("case", "outcome", "boot", "point", "variable")
OUTCOMES = ("good", "bad", "unknown")
BOOTS = ("cold", "warm", "unknown")


def validate(data):
    if not isinstance(data, dict) or set(data) != {"schema", "context", "inventory"}:
        raise ValueError("campos de sesion invalidos")
    if data["schema"] != SCHEMA:
        raise ValueError("esquema de sesion desconocido")
    context = data["context"]
    if not isinstance(context, dict) or set(context) != set(CONTEXT):
        raise ValueError("contexto de sesion incompleto")
    for key, value in context.items():
        if not isinstance(value, str) or not value.strip() or len(value) > 256:
            raise ValueError("contexto invalido: " + key)
    if context["outcome"] not in OUTCOMES or context["boot"] not in BOOTS:
        raise ValueError("resultado o arranque desconocido")
    inv.validate(data["inventory"])
    return data


def make(artifacts, observation, context):
    return validate({"schema": SCHEMA, "context": context,
                     "inventory": inv.inventory(artifacts, observation)})


def compare(left, right):
    validate(left)
    validate(right)
    missing = {}
    for name, session in (("before", left), ("after", right)):
        observation = session["inventory"]["observation"]
        gaps = [key for key in inv.OBS_KEYS if observation[key] is None]
        gaps += ["artifact:" + role for role in ("gsp", "booter", "logs", "report")
                 if role not in session["inventory"]["artifacts"]]
        gaps += ["context:" + key for key in ("outcome", "boot", "point", "variable")
                 if session["context"][key] == "unknown"]
        missing[name] = gaps
    return {
        "schema": "bmo-nvidia-session-comparison-v1",
        "context_origin": "user_declared_not_measured_by_this_tool",
        "context_changes": {key: {"before": left["context"][key],
                                  "after": right["context"][key]}
                            for key in CONTEXT if left["context"][key] != right["context"][key]},
        "missing": missing,
        "evidence": inv.compare(left["inventory"], right["inventory"]),
        "conclusion": "differences_only_not_causation_or_hardware_validation",
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    record = sub.add_parser("registrar")
    record.add_argument("--artefacto", action="append", required=True)
    for key in ("build", "pci-id", "firmware-release"):
        record.add_argument("--" + key)
    record.add_argument("--attempt", type=int)
    record.add_argument("--case", required=True)
    record.add_argument("--outcome", choices=OUTCOMES, default="unknown")
    record.add_argument("--boot", choices=BOOTS, default="unknown")
    record.add_argument("--point", default="unknown")
    record.add_argument("--variable", default="unknown")
    diff = sub.add_parser("comparar")
    diff.add_argument("antes")
    diff.add_argument("despues")
    args = parser.parse_args(argv)
    try:
        if args.command == "registrar":
            observation = {key: getattr(args, key) for key in inv.OBS_KEYS}
            result = make(args.artefacto, observation,
                          {key: getattr(args, key) for key in CONTEXT})
        else:
            result = compare(inv.load(args.antes), inv.load(args.despues))
    except (OSError, ValueError, RecursionError) as error:
        print("sesion GPU: " + str(error), file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2, sort_keys=True, ensure_ascii=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
