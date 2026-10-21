#!/usr/bin/env python3
"""Traduce fotos externas HS de CAIDA.TXT; no consulta ni modifica la GPU.

[carril] VERDE -- texto e inventario del anfitrion
[consumo] NADA -- llamada explicita, salida JSON
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

from inventario import fingerprint

MAX_BYTES = 8 * 1024 * 1024
REGS = (
    (0x40, "MAILBOX0"), (0x44, "MAILBOX1"), (0x100, "CPUCTL"),
    (0x1668, "BCR_CTRL"), (0x1388, "RISCV_CPUCTL"), (0x108, "HWCFG"),
    (0x12C, "HWCFG1"), (0xF4, "HWCFG2"), (0x3C0, "ENGINE"),
    (0x10C, "DMACTL"), (0x600, "TRANSCFG"), (0x110, "DMATRFBASE"),
    (0x128, "DMATRFBASE1"), (0x114, "DMATRFMOFFS"), (0x11C, "DMATRFFBOFFS"),
    (0x118, "DMATRFCMD"), (0x104, "BOOTVEC"), (0x1210, "BROM_PARAADDR"),
    (0x119C, "BROM_ENGIDMASK"), (0x1198, "BROM_UCODE_ID"), (0x1180, "MOD_SEL"),
)
BASES = {0x110000: "GSP", 0x840000: "SEC2"}
LINEA = re.compile(r"^\[\s*gpu\]\s+(HS .*) =([0-9]+)\s*$")


def pri(v):
    return v == 0xFFFFFFFF or v >> 20 == 0xBAD


def cerrar(foto, motivo=None):
    errores = [] if motivo is None else [motivo]
    errores.extend(foto.pop("anomalias", []))
    if not 1 <= foto["id_operacion_local"] <= 32:
        errores.append("id_fuera_de_formato")
    if foto.get("tsc_inicio") is None or foto.get("tsc_fin") is None:
        errores.append("faltan_tiempos")
    elif foto["tsc_fin"] < foto["tsc_inicio"]:
        errores.append("tiempos_invertidos")
    datos = foto.pop("datos")
    if len(datos) != len(REGS):
        errores.append("lista_incompleta")
    base = datos[0][0] - REGS[0][0] if datos else 0
    if base not in BASES:
        errores.append("motor_desconocido")
    mascara = 0
    filas = []
    for i, (direccion, crudo, es_pri, identificador) in enumerate(datos):
        if crudo > 0xFFFFFFFF:
            errores.append("crudo_fuera_de_32_bits")
        if identificador != foto["id_operacion_local"]:
            errores.append("id_cambio_en_registro")
        if i >= len(REGS) or direccion != base + REGS[i][0]:
            errores.append("orden_o_direccion_invalida")
            break
        if pri(crudo) != es_pri:
            errores.append("clasificacion_pri_incoherente")
        mascara |= int(es_pri) << i
        fila = {"registro": REGS[i][1], "direccion": f"0x{direccion:08x}",
                "crudo": f"0x{crudo:08x}", "lectura": "PRI" if es_pri else "legible"}
        if not es_pri:
            if fila["registro"] == "DMATRFCMD":
                fila.update(full=bool(crudo & 1), idle=bool(crudo & 2))
            elif fila["registro"] == "RISCV_CPUCTL":
                fila.update(activo=bool(crudo & 128), parado=bool(crudo & 16))
            elif fila["registro"] == "BCR_CTRL":
                fila.update(riscv_seleccionado=bool(crudo & 16))
        filas.append(fila)
    if mascara != foto.get("mascara_pri"):
        errores.append("mascara_pri_incoherente")
    foto.update(motor=BASES.get(base), registros=filas, completa=not errores,
                errores=sorted(set(errores)))
    return foto


def analizar(texto):
    fotos = []
    actual = None
    for numero, linea in enumerate(texto.splitlines(), 1):
        m = LINEA.fullmatch(linea)
        if m is None:
            continue
        mensaje, valor = m.group(1), int(m.group(2))
        if mensaje.startswith("HS foto externa"):
            if actual is not None:
                fotos.append(cerrar(actual, "foto_interrumpida"))
            actual = {"id_operacion_local": valor, "fase": mensaje,
                      "linea_inicio": numero, "datos": []}
        elif actual is not None:
            if mensaje.startswith("HS foto: TSC antes"):
                if "tsc_inicio" in actual:
                    actual.setdefault("anomalias", []).append("campo_duplicado")
                actual["tsc_inicio"] = valor
            elif mensaje.startswith("HS foto: TSC despues"):
                if "tsc_fin" in actual:
                    actual.setdefault("anomalias", []).append("campo_duplicado")
                actual["tsc_fin"] = valor
            elif mensaje.startswith("HS foto: mascara PRI"):
                if "mascara_pri" in actual:
                    actual.setdefault("anomalias", []).append("campo_duplicado")
                actual["mascara_pri"] = valor
            elif mensaje.startswith(("HS foto legible:", "HS foto PRI:")):
                actual["datos"].append((valor & 0xFFFFFF, valor >> 32,
                                         mensaje.startswith("HS foto PRI:"), (valor >> 24) & 255))
            elif mensaje.startswith("HS limite:"):
                actual["linea_fin"] = numero
                fotos.append(cerrar(actual, None if valor == actual["id_operacion_local"] else "id_cambio"))
                actual = None
    if actual is not None:
        fotos.append(cerrar(actual, "falta_cierre"))
    return {"schema": "bmo-hs-lente-v1", "ambito": "texto_externo_no_autenticacion_hs",
            "identidad": "id_local_no_uuid_ni_intento_booter; conservar informe/build por separado",
            "fotos": fotos, "fotos_completas": sum(f["completa"] for f in fotos)}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("archivo", help="CAIDA.TXT de una copia identificada")
    args = p.parse_args()
    try:
        firma = fingerprint(args.archivo)
        with Path(args.archivo).open("rb") as stream:
            datos = stream.read(MAX_BYTES + 1)
        if len(datos) > MAX_BYTES or hashlib.sha256(datos).hexdigest() != firma["sha256"]:
            raise ValueError("archivo demasiado grande o cambiado durante lectura")
        resultado = analizar(datos.decode("utf-8", errors="replace"))
        resultado["entrada"] = firma
        print(json.dumps(resultado, indent=2, ensure_ascii=True))
        return 0
    except (OSError, ValueError) as e:
        print("lente HS: " + str(e), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
