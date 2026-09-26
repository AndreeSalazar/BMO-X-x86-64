"""Mide la frecuencia fundamental de un .wav mono de 16 bits (autocorrelacion,
sin numpy). Uso: python frecuencia.py fichero.wav [desde_s] [largo_s]"""
import struct
import sys

f = open(sys.argv[1], "rb").read()
hz = struct.unpack_from("<I", f, 24)[0]
datos = f[44:]
m = struct.unpack("<%dh" % (len(datos) // 2), datos)
desde = int(float(sys.argv[2]) * hz) if len(sys.argv) > 2 else int(0.2 * hz)
largo = int(float(sys.argv[3]) * hz) if len(sys.argv) > 3 else int(0.25 * hz)
x = m[desde:desde + largo]
media = sum(x) / len(x)
x = [v - media for v in x]
energia = sum(v * v for v in x)
if energia == 0:
    print("silencio")
    sys.exit(0)
mejor = None
# Retardos de 30 Hz a 4 kHz: el primer pico alto de la autocorrelacion.
ret_max = hz // 30
r = []
for lag in range(hz // 4000, ret_max):
    s = 0
    for i in range(0, len(x) - lag, 2):
        s += x[i] * x[i + lag]
    r.append((lag, s))
tope = max(s for _, s in r)
for i in range(1, len(r) - 1):
    lag, s = r[i]
    if s > 0.85 * tope and s >= r[i - 1][1] and s >= r[i + 1][1]:
        mejor = lag
        break
pico = max(abs(v) for v in x)
print("%s: %.1f Hz  (pico %d)" % (sys.argv[1], hz / mejor if mejor else 0, pico))
