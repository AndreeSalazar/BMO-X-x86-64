#!/usr/bin/env python3
"""Traduce fotos WPR de CAIDA; tablas/copia CPU, no transacciones de GPU.

[carril] VERDE -- lee una copia privada; emite JSON sin modificarla
[consumo] NADA -- invocacion explicita
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
from inventario import fingerprint

MAX_BYTES = 8 * 1024 * 1024
MAX = (1 << 64) - 1
LINEA = re.compile(r"^\[\s*gpu\]\s+(WPR foto: .*) =([0-9]+)\s*$")
CAMPOS = {
    "fase": "fase", "TSC antes": "tsc_inicio", "TSC despues": "tsc_fin",
    "IOVA": "iova", "fisica propia": "fisica_esperada", "fisica inicio": "fisica_inicio",
    "fisica fin": "fisica_fin", "permisos": "permisos", "estado host": "estado_host",
    "copia observada": "copia_disponible", "mascara diferencias": "mascara_diferencias",
}
ESTADOS = ("coincide", "sin_traduccion", "pagina_distinta", "sin_escritura",
           "sin_copia", "contenido_distinto", "base_invalida")
NOMBRES = ("magic", "revision", "sysmemAddrOfRadix3Elf", "sizeOfRadix3Elf",
    "sysmemAddrOfBootloader", "sizeOfBootloader", "bootloaderCodeOffset",
    "bootloaderDataOffset", "bootloaderManifestOffset", "sysmemAddrOfSignature",
    "sizeOfSignature", "gspFwRsvdStart", "nonWprHeapOffset", "nonWprHeapSize",
    "gspFwWprStart", "gspFwHeapOffset", "gspFwHeapSize", "gspFwOffset",
    "bootBinOffset", "frtsOffset", "frtsSize", "gspFwWprEnd", "fbSize",
    "vgaWorkspaceOffset", "vgaWorkspaceSize", "bootCount", "unionRpc_208",
    "unionRpc_216", "sysmemAddrOfCrashReportQueue", "sizeOfCrashReportQueue",
    "vf_flags_paging_reservado", "verified")


def cerrar(f, error=None):
    e = f["errores"]
    if error:
        e.append(error)
    if any(k not in f for k in CAMPOS.values()):
        e.append("faltan_campos")
    if f["id_local"] not in (1, 2) or f.get("fase") not in (1, 2):
        e.append("identidad_invalida")
    if f.get("tsc_fin", -1) < f.get("tsc_inicio", 0):
        e.append("tiempos_invalidos")
    filas = f.pop("filas")
    copia = f.get("copia_disponible")
    if copia not in (0, 1) or [r["indice"] for r in filas] != list(range(32)):
        e.append("lista_incompleta")
    for r in filas:
        if "esperada" not in r or ("observada" in r) != (copia == 1):
            e.append("palabra_incompleta")
    if not e:
        base, p = f["fisica_esperada"], f["permisos"]
        if f["iova"] == 0 or f["iova"] & 4095 or f["iova"] > MAX - 255:
            e.append("iova_invalida")
        if p > 15 or p & 2 and not p & 1 or p & 8 and not p & 4:
            e.append("permisos_invalidos")
        if (not p & 1 and f["fisica_inicio"] != MAX) or (not p & 4 and f["fisica_fin"] != MAX):
            e.append("traduccion_incoherente")
        estado = (6 if base == 0 or base & 4095 or base > MAX - 255 else
                  1 if p & 5 != 5 else
                  2 if f["fisica_inicio"] != base or f["fisica_fin"] != base + 255 else
                  3 if p != 15 else 0)
        if estado and copia:
            e.append("copia_con_enlace_invalido")
        delta = sum(1 << i for i, r in enumerate(filas)
                    if copia and r["esperada"] != r["observada"])
        if not estado:
            estado = 4 if not copia else 5 if delta else 0
        if f["estado_host"] != estado or f["mascara_diferencias"] != delta:
            e.append("resultado_incoherente")
    f["estado"] = ESTADOS[f["estado_host"]] if f.get("estado_host", 99) in range(7) else "desconocido"
    f["palabras"] = []
    for r in filas:
        i = r["indice"]
        f["palabras"].append({"offset": i * 8, "campo": NOMBRES[i] if i in range(32) else "fuera",
            **{k: f"0x{r[k]:016x}" for k in ("esperada", "observada") if k in r}})
    f["completa"] = not e
    return f


def analizar(texto):
    fotos, actual = [], None
    for n, linea in enumerate(texto.splitlines(), 1):
        m = LINEA.match(linea)
        if not m:
            continue
        mensaje, valor = m[1][10:], int(m[2])
        if mensaje.startswith("id local"):
            if actual:
                fotos.append(cerrar(actual, "foto_interrumpida"))
            actual = {"id_local": valor, "linea_inicio": n, "filas": [], "errores": []}
        elif actual is not None:
            if valor > MAX:
                actual["errores"].append("fuera_de_u64")
            if mensaje.startswith("cierre"):
                actual["linea_fin"] = n
                fotos.append(cerrar(actual, None if valor == actual["id_local"] else "id_cambio"))
                actual = None
            elif mensaje.startswith("palabra indice"):
                actual["filas"].append({"indice": valor})
            elif mensaje.startswith(("palabra esperada", "palabra observada")):
                if not actual["filas"]:
                    actual["errores"].append("palabra_sin_indice")
                    continue
                fila = actual["filas"][-1]
                clave = "esperada" if "esperada" in mensaje else "observada"
                if clave in fila:
                    actual["errores"].append("palabra_duplicada")
                fila[clave] = valor
            else:
                for prefijo, clave in CAMPOS.items():
                    if mensaje.startswith(prefijo):
                        if clave in actual:
                            actual["errores"].append("campo_duplicado")
                        actual[clave] = valor
                        break
    if actual:
        fotos.append(cerrar(actual, "falta_cierre"))
    return {"schema": "bmo-wpr-foto-v1", "ambito": "tablas_y_copia_cpu_no_dma_ni_autenticacion",
            "identidad": "id_local_y_TSC_no_uuid; conservar build/informe por separado",
            "fotos": fotos, "fotos_completas": sum(f["completa"] for f in fotos)}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("archivo")
    args = p.parse_args()
    try:
        firma = fingerprint(args.archivo)
        with Path(args.archivo).open("rb") as stream:
            datos = stream.read(MAX_BYTES + 1)
        if len(datos) > MAX_BYTES or hashlib.sha256(datos).hexdigest() != firma["sha256"]:
            raise ValueError("archivo grande o cambiado durante lectura")
        resultado = analizar(datos.decode("utf-8", errors="replace"))
        resultado["entrada"] = firma
        print(json.dumps(resultado, indent=2, ensure_ascii=True))
        return 0
    except (OSError, ValueError) as e:
        print("WPR foto: " + str(e), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
