# El fondo de BMO-X

## `ciudad.webp` -- la ciudad del gato

El arte del propietario: el gato de BMO-X sentado en una calle de neon, con los
carteles de `BMO-X METAKERNEL`, `RING 0 SECURE`, `BEF FORMAT` y el kanji del gato (neko). Es el fondo
del escritorio desde el 2026-09-22 (*"si, pon la ciudad de fondo"*).

```text
   1530 x 1024, WebP, 445 KB    lo que se guarda aqui
   1920 x 1080, QOI, 3,7 MB     lo que lee el DIRECTOR (sys/fondo.qoi)
```

## Por que aqui y no el `.qoi`

Por la regla de `Nuevo-Fondo` en `ejemplos.ps1`: **un binario en el repo no se
lee en un diff**. El `.qoi` es un PRODUCTO -- lo hace el build con
`toolchain/tools/fondo/a_qoi.py` (cubrir la pantalla con Lanczos, QOI, y la ida
y vuelta comprobada con Pillow). Lo que no se puede generar es el arte, y el
arte vive en `activos/`, como `neko.wav`: un activo que desaparece al
reconstruir no es un activo.

Sin python o sin Pillow, el build se queda con la noche generada de siempre y
lo dice en amarillo.

## La paleta sale de aqui

Los colores del sistema (`tema.maqueta`) se midieron sobre este arte y el logo
(`docs/arte/bmo-x-gato.jpg`): el acento es el ojo del gato (#5EF2E6), el panel
negro con pelo violeta, el cielo la noche de esta ciudad (#161236).
