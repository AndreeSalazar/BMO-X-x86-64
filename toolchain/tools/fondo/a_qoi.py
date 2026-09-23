"""**EL FONDO DEL ESCRITORIO: una imagen -> `sys/fondo.qoi`** (2026-09-22).

    python a_qoi.py <imagen> <salida.qoi> <ancho> <alto>

El propietario puso su ciudad de neon con el gato (`activos/fondo/ciudad.webp`)
de fondo de BMO-X. Hasta ese dia el fondo lo GENERABA `ejemplos.ps1::Nuevo-Fondo`
--una noche con luna, en codigo--, y el motivo sigue en pie: un binario de 3,5 MB
en el repo no se lee en un diff. Por eso en el repo va el ARTE (en `activos/`, como
`neko.wav`) y esto, que se lee entero, y el `.qoi` lo hace el build.

Lo que hace, en orden:

    1. CUBRIR la pantalla: escala (Lanczos) hasta tapar ancho x alto y recorta
       lo que sobra, centrado -- lo mismo que `scene/fondo.rs`, pero aqui con un
       buen filtro y una sola vez, en vez de vecino mas cercano en el Ryzen.
    2. QOI, que el DIRECTOR ya lee (`bmo-imagen`): RGB, sin alfa.
    3. IDA Y VUELTA: Pillow lee QOI, y lo escrito tiene que decir los mismos
       pixeles que se pidieron. Un codificador que no se comprueba es una
       opinion.

Sin Pillow falla con codigo 1, y el build se queda con la noche generada y lo
dice. No inventa un fondo.
"""
import struct
import sys

try:
    from PIL import Image
except ImportError:
    sys.stderr.write("a_qoi: falta Pillow (pip install pillow)\n")
    sys.exit(1)


def cubrir(im, w, h):
    sw, sh = im.size
    s = max(w / sw, h / sh)
    nw, nh = round(sw * s), round(sh * s)
    im = im.resize((nw, nh), Image.LANCZOS)
    ox, oy = (nw - w) // 2, (nh - h) // 2
    return im.crop((ox, oy, ox + w, oy + h))


def qoi(px, w, h):
    """El formato entero de https://qoiformat.org, canales = 3."""
    out = bytearray(b"qoif" + struct.pack(">II", w, h) + bytes([3, 0]))
    vistos = [None] * 64
    prev = (0, 0, 0)
    run = 0
    n = w * h
    for i in range(n):
        c = (px[3 * i], px[3 * i + 1], px[3 * i + 2])
        if c == prev:
            run += 1
            if run == 62 or i == n - 1:
                out.append(0xC0 | (run - 1))
                run = 0
            continue
        if run:
            out.append(0xC0 | (run - 1))
            run = 0
        r, g, b = c
        k = (r * 3 + g * 5 + b * 7 + 255 * 11) % 64
        if vistos[k] == c:
            out.append(k)
        else:
            vistos[k] = c
            dr = (r - prev[0] + 128) % 256 - 128
            dg = (g - prev[1] + 128) % 256 - 128
            db = (b - prev[2] + 128) % 256 - 128
            if -2 <= dr <= 1 and -2 <= dg <= 1 and -2 <= db <= 1:
                out.append(0x40 | ((dr + 2) << 4) | ((dg + 2) << 2) | (db + 2))
            elif -32 <= dg <= 31 and -8 <= dr - dg <= 7 and -8 <= db - dg <= 7:
                out.append(0x80 | (dg + 32))
                out.append(((dr - dg + 8) << 4) | (db - dg + 8))
            else:
                out += bytes([0xFE, r, g, b])
        prev = c
    out += bytes([0, 0, 0, 0, 0, 0, 0, 1])
    return bytes(out)


def main():
    if len(sys.argv) != 5:
        sys.stderr.write("uso: a_qoi.py <imagen> <salida.qoi> <ancho> <alto>\n")
        sys.exit(2)
    src, dst, w, h = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
    im = cubrir(Image.open(src).convert("RGB"), w, h)
    px = im.tobytes()
    datos = qoi(px, w, h)
    # `scene/fondo.rs` no lee mas de 32 MiB, y `bmo-imagen` no pasa de 4096 de
    # lado: mejor fallar aqui que en el Ryzen, con el escritorio en degradado.
    if len(datos) > 32 * 1024 * 1024 or w > 4096 or h > 4096:
        sys.stderr.write("a_qoi: %d bytes / %dx%d no caben en el DIRECTOR\n" % (len(datos), w, h))
        sys.exit(1)
    with open(dst, "wb") as f:
        f.write(datos)
    back = Image.open(dst).convert("RGB")
    if back.size != (w, h) or back.tobytes() != px:
        sys.stderr.write("a_qoi: el QOI no hace ida y vuelta\n")
        sys.exit(1)
    print("a_qoi: %s %dx%d, %d bytes" % (dst, w, h, len(datos)))


if __name__ == "__main__":
    main()
