#!/usr/bin/env python3
"""Desensambla una region declarada RISC-V; nunca autodetecta Falcon o SM86.

[carril] VERDE -- copia regular del anfitrion; JSON en stdout
[consumo] NADA -- consulta explicita
[cuesta] NADA -- interpretacion, sin ejecucion ni carga
"""
import argparse
import hashlib
import json
import os
import stat
import sys

MAX_REGION = 65536  # Limite de esta herramienta, no de la ISA.


def region(path, offset, length):
    if type(offset) is not int or offset < 0 or type(length) is not int or not 1 <= length <= MAX_REGION:
        raise ValueError("offset >= 0 y longitud entre 1 y 65536")
    flags = os.O_RDONLY | getattr(os, "O_NONBLOCK", 0) | getattr(os, "O_BINARY", 0)
    with os.fdopen(os.open(path, flags), "rb") as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode) or offset + length > before.st_size:
            raise ValueError("region fuera de archivo regular")
        stream.seek(offset)
        data = stream.read(length)
        after = os.fstat(stream.fileno())
    if len(data) != length or any(getattr(before, k) != getattr(after, k)
                                for k in ("st_size", "st_mtime_ns", "st_ctime_ns")):
        raise ValueError("copia cambiada durante lectura")
    return data


def decode(data, isa, address, compressed=False):
    if isa not in ("rv32", "rv64") or type(address) is not int or address < 0:
        raise ValueError("ISA o direccion invalida")
    bits = 32 if isa == "rv32" else 64
    alignment = 2 if compressed else 4
    if address % alignment or address + len(data) > 1 << bits or not 1 <= len(data) <= MAX_REGION:
        raise ValueError("direccion, alineacion o region fuera de limites")
    try:
        import capstone as cs
    except ImportError as error:
        raise ValueError("falta Capstone con soporte RISC-V; ver LABORATORIO.md") from error
    mode = cs.CS_MODE_RISCV32 if isa == "rv32" else cs.CS_MODE_RISCV64
    if compressed:
        mode |= cs.CS_MODE_RISCVC
    instructions = []
    consumed = 0
    try:
        engine = cs.Cs(cs.CS_ARCH_RISCV, mode)
        engine.skipdata = False
        for instruction in engine.disasm(data, address):
            instructions.append({"address": instruction.address, "region_offset": consumed,
                                 "bytes": bytes(instruction.bytes).hex(),
                                 "mnemonic": instruction.mnemonic, "operands": instruction.op_str})
            consumed += instruction.size
    except cs.CsError as error:
        raise ValueError("Capstone: " + str(error)) from error
    return {"schema": "bmo-riscv-region-v1", "isa_declared": isa,
            "compressed_declared": compressed, "address": address,
            "region_bytes": len(data), "region_sha256": hashlib.sha256(data).hexdigest(),
            "decoder": {"name": "capstone", "version": cs.__version__},
            "instructions": instructions, "consumed": consumed,
            "undecoded_hex": data[consumed:].hex(),
            "stop": "end" if consumed == len(data) else "unrecognized_or_truncated",
            "limits": "declared_isa_not_chip_identification_execution_or_semantic_proof"}


def integer(value):
    return int(value, 0)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("file")
    parser.add_argument("--isa", choices=("rv32", "rv64"), required=True)
    parser.add_argument("--offset", type=integer, required=True)
    parser.add_argument("--length", type=integer, required=True)
    parser.add_argument("--address", type=integer, required=True)
    parser.add_argument("--compressed", action="store_true")
    args = parser.parse_args(argv)
    try:
        result = decode(region(args.file, args.offset, args.length), args.isa,
                        args.address, args.compressed)
        result["file_offset"] = args.offset
    except (OSError, ValueError) as error:
        print("RISC-V: " + str(error), file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2, sort_keys=True, ensure_ascii=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
