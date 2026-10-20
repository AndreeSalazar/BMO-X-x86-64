"""Consulta GSP por NVML en Windows. Solo lectura; no cambia el modo del driver."""
import ctypes as c
import json
import os


def consultar():
    if os.name != "nt":
        raise RuntimeError("esta consulta requiere Windows y nvml.dll instalada")
    # Ruta del sistema, sin buscar una DLL del directorio de trabajo.
    ruta = os.path.join(os.environ["SystemRoot"], "System32", "nvml.dll")
    n = c.WinDLL(ruta)

    def funcion(nombre, args):
        f = getattr(n, nombre)
        f.argtypes = args
        f.restype = c.c_int
        return f

    error = n.nvmlErrorString
    error.argtypes = [c.c_int]
    error.restype = c.c_char_p

    def estado(rc):
        return {"status": rc, "message": error(rc).decode("ascii", errors="replace")}

    rc = funcion("nvmlInit_v2", [])()
    resultado = {"contract": "BMO_WINDOWS_GSP_QUERY_V1", "device_index": 0, "init": estado(rc)}
    if rc != 0:
        return resultado
    try:
        h = c.c_void_p()
        rc = funcion("nvmlDeviceGetHandleByIndex_v2", [c.c_uint, c.POINTER(c.c_void_p)])(0, c.byref(h))
        resultado["device"] = estado(rc)
        if rc != 0:
            return resultado
        enabled, default = c.c_uint(0xFFFFFFFF), c.c_uint(0xFFFFFFFF)
        rc = funcion("nvmlDeviceGetGspFirmwareMode", [c.c_void_p, c.POINTER(c.c_uint), c.POINTER(c.c_uint)])(h, c.byref(enabled), c.byref(default))
        resultado["mode"] = dict(estado(rc), enabled=bool(enabled.value) if rc == 0 else None, default=bool(default.value) if rc == 0 else None)
        version = c.create_string_buffer(256)
        rc = funcion("nvmlDeviceGetGspFirmwareVersion", [c.c_void_p, c.c_char_p])(h, version)
        resultado["version"] = dict(estado(rc), value=version.value.decode("ascii", errors="replace") if rc == 0 else None)
        return resultado
    finally:
        resultado["shutdown"] = estado(funcion("nvmlShutdown", [])())


if __name__ == "__main__":
    try:
        print(json.dumps(consultar(), indent=2))
    except (OSError, AttributeError, RuntimeError, KeyError) as exc:
        print(json.dumps({"contract": "BMO_WINDOWS_GSP_QUERY_V1", "error": str(exc)}))
        raise SystemExit(2)
