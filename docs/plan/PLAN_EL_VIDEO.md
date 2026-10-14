# PLAN EL VIDEO -- ver video en BMO-X con la 3060, y la plataforma como pala

> Escrito el **2026-10-14**. El propietario: *"BMO-X no necesita YouTube;
> DAME buena razon para que YouTube entre en BMO-X"*: YouTube llega a 60 fps
> y a 8K, y mete anuncios que nadie vigila. Y: *"yo solo les vendere la
> pala"* -- BMO-X no opera una plataforma de video; da el software para que
> una empresa la monte.
>
> La pieza de la 3060 ya estaba anotada: **M6** de
> [`PLAN_LA_3060.md`](PLAN_LA_3060.md) (NVDEC, el video comprimido) y su mitad
> HECHA, `video` (M6 V0: NV12 a color y agrandado por la 3060, directo a la
> pantalla). El canal de HERMES la usa (H10 de [`PLAN_HERMES.md`](PLAN_HERMES.md)).
> Este plan junta lo que falta, en orden.

---

## 0. Las reglas

- **Lo que hace la 3060, lo hace la 3060.** El video lo descomprime NVDEC y lo
  presenta la 3060; la CPU solo abre la caja (el contenedor) y lleva la
  cuenta de los fotogramas de referencia. Sin decodificador por software.
- **AV1 primero.** Libre de patentes, lo sirve YouTube, y NVDEC de Ampere lo
  decodifica en hardware. H.264 despues (lo que mas hay en `.mp4`).
- **Ni un atajo contra las condiciones de nadie.** YouTube en BMO-X va por la
  ANTENA (un navegador de verdad en otra maquina) hasta que haya un acuerdo;
  sacar sus videos por fuera de su API rompe sus condiciones y se rompe cada
  vez que cambian. El DRM (Widevine) no se toca: solo lo dan a dispositivos
  certificados.
- **Lo publico se trae, lo que no esta se mide**: la clase de video del RM y
  sus estructuras salen de open-gpu-doc y de los modulos abiertos de NVIDIA;
  lo demas se mide en el metal, como el GR.

---

## 1. La 3060 descomprime

### [ ] V1 -- NVDEC: la clase de video del RM, pedida y encendida

Como el GR: el objeto del decodificador sobre su canal (la clase de Ampere,
de open-gpu-doc), su memoria y su primer trabajo pagado con un semaforo. El
banco: un fotograma AV1 conocido, decodificado, y su huella contra la de un
decodificador de referencia en el anfitrion (dav1d, solo para el banco).

### [ ] V2 -- la caja: leer el contenedor (WebM/MKV y MP4)

En TITAN++: las cabeceras y la lista de muestras (donde empieza cada
fotograma comprimido). Pocos KB de estado. Lo que no se entiende se dice,
con su caja y su motivo.

### [ ] V3 -- el reproductor en TITAN++

Pide los bytes (disco, red o antena), se los da a NVDEC, y M6 V0 presenta:
NV12 en VRAM -> color -> agrandado -> pantalla. Con el ritmo del VBLANK (E2,
ya medido) para que no se parta la imagen. Medida: los fps y la CPU casi
dormida.

---

## 2. La pantalla

### [ ] V4 -- el modo de la pantalla: 74 Hz

El monitor del propietario da 74 Hz; BMO-X usa el modo que dejo el GOP (1920
x 1080 a 60, medido 60,001 Hz). Subirlo es programar el motor de pantalla de
la 3060 (o pedirselo al RM): otra pieza, con su banco. Un video a 120 fps se
ve al refresco del monitor, ni uno mas.

---

## 3. YouTube y la plataforma

### [ ] V5 -- YouTube por la ANTENA (hoy)

Lo que ya hay: el navegador vive en la antena y BMO-X pinta la lamina
(`apps/navegar`). Para video, la antena manda el video tal cual (AV1) y BMO-X
lo reproduce con V3, en vez de una lamina por fotograma.

### [ ] V6 -- la app oficial, si un dia hay acuerdo

Las teles no usan Chromium para YouTube: usan Cobalt (de Google, abierto),
con una certificacion de Google. Si BMO-X tiene usuarios algun dia, el camino
es ese: pedir la certificacion, con un reproductor que gasta poquisimo como
argumento. Sin acuerdo, no.

### [ ] V7 -- la pala: el software para compartir video

Lo que BMO-X venderia (como PeerTube, pero con lo de BMO-X): el servidor que
guarda y sirve video, y el reproductor. Lo que una plataforma propia daria
que YouTube no:

```text
   120 / 240 fps   gameplays, deportes, animacion (lo que se nota)
   anuncios        con reglas y su guardian: el que no cumple no se publica
   consumo         servir y reproducir con el minimo de vatios, medido
```

Lo que paga quien OPERA, no quien vende la pala: guardar y transmitir video
(lo mas caro), la ley (derechos de autor, contenido ilegal, moderacion) y
atraer a los creadores.
