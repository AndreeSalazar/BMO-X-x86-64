#!/usr/bin/env python3
"""Copias de firmware y partes: huellas de archivo, sin aparato ni red.

[carril] VERDE -- lee archivos regulares; escribe solo JSON en stdout
[consumo] NADA -- consulta explicita en el anfitrion
[cuesta] NADA -- un inventario equivocado; no autoriza cargar firmware

No interpreta ELF: el lector de ga10x ya tiene ese propietario. Una huella
de archivo no acredita firma NVIDIA, coherencia DMA ni ejecucion en HS.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys


SCHEMA = "bmo-nvidia-hs-inventory-v1"
ROLES = ("vbios", "booter", "gsp", "logs", "log_dictionary", "report")
BLOCK = 1024 * 1024  # Lectura acotada: no carga los 61 MB del GSP de golpe.
LIMITS = "file_bytes_only_not_signature_dma_or_hardware_validation"
OBS_KEYS = ("build", "attempt", "pci_id", "firmware_release")


def fingerprint(path):
    """Detecta cambios observables durante la lectura; no promete atomicidad."""
    # O_NONBLOCK evita quedar esperando si la ruta apunta a un FIFO. En
    # Windows O_BINARY conserva los bytes originales al abrir el descriptor.
    flags = os.O_RDONLY | getattr(os, "O_NONBLOCK", 0) | getattr(os, "O_BINARY", 0)
    with os.fdopen(os.open(path, flags), "rb") as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ValueError("se requiere un archivo regular")
        digest = hashlib.sha256()
        count = 0
        for block in iter(lambda: stream.read(BLOCK), b""):
            digest.update(block)
            count += len(block)
        after = os.fstat(stream.fileno())
    fields = ("st_size", "st_mtime_ns", "st_ctime_ns")
    if count != before.st_size or any(
        getattr(before, key) != getattr(after, key) for key in fields
    ):
        raise ValueError("archivo cambiado durante la lectura; repetir la copia")
    if count == 0:
        raise ValueError("archivo vacio; no se registra como firmware o captura")
    return {"bytes": count, "sha256": digest.hexdigest()}


def inventory(artifacts, observation):
    items = {}
    for declaration in artifacts:
        role, sep, path = declaration.partition("=")
        if not sep or role not in ROLES or not path:
            raise ValueError("artefacto: PAPEL=ARCHIVO; papeles: " + ", ".join(ROLES))
        if role in items:
            raise ValueError("papel duplicado: " + role)
        items[role] = fingerprint(path)
    if not items:
        raise ValueError("falta al menos un artefacto")
    return {
        "schema": SCHEMA,
        "limits": LIMITS,
        "observation": {key: observation.get(key) for key in OBS_KEYS},
        "observation_origin": "user_declared_not_measured_by_this_tool",
        "artifacts": items,
    }


def validate(data):
    """Acepta solo el contrato v1 antes de comparar; no valida el firmware."""
    if not isinstance(data, dict) or data.get("schema") != SCHEMA:
        raise ValueError("esquema de inventario desconocido")
    if data.get("limits") != LIMITS or data.get("observation_origin") != (
        "user_declared_not_measured_by_this_tool"
    ):
        raise ValueError("faltan los limites o la procedencia del inventario")
    observation = data.get("observation")
    if not isinstance(observation, dict) or set(observation) != set(OBS_KEYS):
        raise ValueError("identidad declarada incompleta; usar null para ausencias")
    for key in ("build", "pci_id", "firmware_release"):
        if observation[key] is not None and (
            not isinstance(observation[key], str) or not observation[key].strip()
        ):
            raise ValueError("identidad invalida: " + key)
    attempt = observation["attempt"]
    if attempt is not None and (type(attempt) is not int or attempt < 1):
        raise ValueError("intento debe ser un entero positivo o null")
    pci = observation["pci_id"]
    if pci is not None and re.fullmatch(r"[0-9a-fA-F]{4}:[0-9a-fA-F]{4}", pci) is None:
        raise ValueError("pci_id debe tener forma 10de:2504")
    items = data.get("artifacts")
    if not isinstance(items, dict) or not items:
        raise ValueError("faltan artefactos")
    for role, item in items.items():
        if role not in ROLES or not isinstance(item, dict):
            raise ValueError("papel o artefacto desconocido: " + str(role))
        if set(item) != {"bytes", "sha256"}:
            raise ValueError("campos de artefacto desconocidos: " + role)
        size, digest = item.get("bytes"), item.get("sha256")
        if type(size) is not int or size < 1 or not isinstance(digest, str) or (
            re.fullmatch(r"[0-9a-f]{64}", digest) is None
        ):
            raise ValueError("medida o SHA-256 invalido: " + role)
    return data


def compare(left, right):
    validate(left)
    validate(right)
    a, b = left["artifacts"], right["artifacts"]
    changes = {}
    for role in sorted(a.keys() | b.keys()):
        status = "added" if role not in a else "removed" if role not in b else (
            "same_file_digest" if a[role] == b[role] else "different_file_digest"
        )
        changes[role] = {"status": status, "before": a.get(role), "after": b.get(role)}
    return {
        "schema": "bmo-nvidia-hs-comparison-v1",
        "limits": LIMITS,
        "observation_changes": {
            key: {"before": left["observation"][key], "after": right["observation"][key]}
            for key in OBS_KEYS if left["observation"][key] != right["observation"][key]
        },
        "artifacts": changes,
    }


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("clave JSON duplicada: " + key)
        result[key] = value
    return result


def load(path):
    # Un JSON no es una captura binaria: limite del CLI, no del silicio.
    with Path(path).open("rb") as stream:
        data = stream.read(BLOCK + 1)
    if len(data) > BLOCK:
        raise ValueError("inventario JSON mayor de 1 MiB")
    return json.loads(data, object_pairs_hook=unique_object)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    make = sub.add_parser("inventario", help="huellas de copias, sin ejecutar")
    make.add_argument("--artefacto", action="append", required=True, metavar="PAPEL=ARCHIVO")
    make.add_argument("--build")
    make.add_argument("--attempt", type=int)
    make.add_argument("--pci-id")
    make.add_argument("--firmware-release")
    diff = sub.add_parser("comparar", help="compara dos inventarios v1")
    diff.add_argument("antes")
    diff.add_argument("despues")
    args = parser.parse_args(argv)
    try:
        if args.command == "inventario":
            result = validate(inventory(args.artefacto, vars(args)))
        else:
            result = compare(load(args.antes), load(args.despues))
    except (OSError, ValueError, RecursionError) as error:
        print("inventario HS: " + str(error), file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2, sort_keys=True, ensure_ascii=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
