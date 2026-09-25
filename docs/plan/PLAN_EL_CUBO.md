# PLAN EL CUBO -- lo que salio de estudiar Direct3D, y por que era inesperado

> El propietario (2026-09-25), despues de verlo: *"documentar TODO eso y
> porque y que potencial tienen eso, ya que el motivo ES INESPERADO"*.
>
> Empezo como una curiosidad -- *"crear un cubo en DX12 para estudiar como se
> porta"* -- y acabo dandole a BMO-X algo que no tenia: **un juez de 3D
> validado por un tercero**. Esta es la historia, lo que se midio, por que
> importa y lo que abre.

---

## 0. La respuesta corta

```text
   lo que se buscaba   entender como Direct3D le habla a la GPU
   lo que salio        un rasterizador por CPU, sin plataforma, que da
                       LOS MISMOS PIXELES que la RTX 3060 bajo D3D12 --
                       en Windows, en Linux y en el soft-float de BMO-X
   por que sorprende   nadie documenta COMO redondea una GPU. Hubo que
                       descubrirlo midiendo, y salieron 4 reglas
   lo que abre         medir la 3060 SIN Windows contra una imagen que
                       Windows ya certifico (X5), y un juez para todo el
                       3D que venga (el RHI, PROTON-X, los juegos)
```

---

## 1. Como empezo (la cadena, en orden)

| paso | que paso | donde |
|---|---|---|
| 1 | La pregunta: *"un cubo en DX12 para estudiar, BMO-X puede ahorrarse el esfuerzo?"* | esta sesion, 25-09 |
| 2 | Otra IA, en el repo EPICX del propietario, escribe un cubo por cada Direct3D (9, 10, 11, 12). Descubre que el antiguo `cube_dx12.rs` **dibujaba por CPU**: el D3D12 de verdad es el nuevo | EPICX, `c1cfc40` |
| 3 | El olfato del propietario: *"algo huele que esta contaminado"*. Lo estaba: los envoltorios se llamaban `Bmox9..12` y eran la API de Microsoft con otro nombre | -- |
| 4 | **La purga**: se separa lo NEUTRO (malla, matrices, luz, el orden de un fotograma) de lo que es de Windows (COM, DXGI, la ventana). Lo neutro va a un crate aparte, `cubo-neutro`, `#![no_std]` y sin dependencias; lo demas se llama `estudio-d3d` y no cruza | EPICX, rama `estudio-d3d`, `5d51bc3`..`412542e` |
| 5 | Se le pide al crate un JUEZ: dibujar el cubo por CPU y compararlo con la GPU, fotograma a fotograma (rotacion por numero de fotograma, no por reloj) | `11f4fa9` |
| 6 | **Lo inesperado**: al comparar, las diferencias no eran ruido. Eran REGLAS. Se midieron hasta dejar 1 pixel en 360 fotogramas | `13f64c8`, `412542e` |
| 7 | Aqui: el juez se trae como `bmo-cubo`, se comprueba en Linux y en el soft-float de BMO-X, y nace `gpu cubo` | `platform/shared/bmo-cubo` |

---

## 2. Lo inesperado: las 4 reglas que ninguna documentacion da

Un triangulo en la pantalla parece matematica exacta. No lo es: la GPU
redondea en cuatro sitios, y cada uno aparecio como una diferencia MEDIDA
contra la RTX 3060 (1280x720), no leido en ningun manual:

| # | la regla | con ella | sin ella | que es |
|---|---|---:|---:|---|
| 1 | **el centro del pixel**: D3D10+ muestrea en (x+0.5, y+0.5); D3D9 en (x, y) | 0 | 732 | una convencion vieja de D3D9, corregida en D3D10 |
| 2 | **subpixel, empates al PAR**: las posiciones van a 1/256 de pixel y un empate exacto (x*256 = n+0.5) va al par | 0 | 1 | un vertice del fotograma 60 cae justo en 218350.5 |
| 3 | **el orden del viewport**: `ndc*(W/2) + W/2`, no `(ndc*0.5+0.5)*W` | 0 | 1 | con x*256 ~ 2*10^5 un `f32` solo tiene 1/64 de subpixel: dos ordenes, dos redondeos |
| 4 | **el color a 8 bits**: la 3060 TRUNCA a 12 bits y despues redondea (`(floor(x*4096)*255 + 2048)/4096`) | 0 | 74.348 | D3D permite hasta 0,6 de error: otra GPU puede hacerlo distinto |

Con las cuatro, en la vuelta entera (360 fotogramas, 332 millones de pixeles)
queda **1 pixel** sin explicar (fotograma 32, pixel (523, 199)), despues de
probar FMA, cinco ordenes de viewport y +-1 ulp en `1/w`. Se dice asi, sin
esconderlo.

**Por que es inesperado:** las especificaciones dan TOLERANCIAS (D3D: "hasta
0,6 unidades"), no el algoritmo. Que una CPU reproduzca a una GPU bit a bit
exige saber el algoritmo, y ese solo se consigue midiendo -- que es lo que
BMO-X hace con todo (`docs/maestro/OPTIMIZACION_MAESTRO.md`, ley 0).

---

## 3. La prueba cruzada: tres sistemas, el mismo numero

Lo que convierte "coincide en Windows" en "es un juez":

| donde corre el juez | como hace las cuentas | fotogramas 0, 30, 60 contra D3D12 en la 3060 |
|---|---|---|
| Windows, `estudio_cpu` (EPICX) | `f32` con SSE | 0 pixeles distintos |
| **Linux** (esta sesion) | `f32` con SSE | **0 de 921.600**, los tres |
| **`x86_64-unknown-none`, freestanding** (esta sesion) | **soft-float: 0 instrucciones SSE**, como el director de BMO-X | **las tres huellas IGUALES** |

La tercera fila es la que importa para BMO-X: el escritorio se compila sin
SSE, y un juez que cambiara con la forma de hacer las cuentas no seria juez.
No cambia porque el seno, el coseno y la raiz son PROPIOS (`num.rs`, sin
libm) y las cuatro operaciones de IEEE-754 dan el mismo bit con SSE que con
soft-float.

Las huellas (`platform/shared/bmo-cubo/src/referencia.rs`) salen de las PNG
de D3D12 decodificadas en el anfitrion, **no del juez**: si salieran de el,
compararlo con ellas seria compararlo consigo mismo.

---

## 4. Por que importa: el principio de BMO-X, un escalon mas arriba

BMO-X ya vivia de un juez: cada trabajo de la 3060 (fractal, triangulo,
escena, pantalla, video) se rehace en la CPU y se compara
(`dev/gpu_trabajo.rs`, `[eje] CORRECCION`). Pero esos jueces los escribio
BMO-X para SUS programas: CPU y GPU hacian la cuenta que BMO-X decidio.

El cubo es distinto en tres cosas:

```text
   hasta hoy                          el cubo
   la cuenta la invento BMO-X         la cuenta la fija el HARDWARE (sus 4 reglas)
   la GPU la corre BMO-X              la GPU la corrio WINDOWS + el driver de NVIDIA
   el juez se valida a si mismo       el juez lo valida un TERCERO independiente
```

**Es la primera imagen de BMO-X certificada por alguien que no es BMO-X.**
Cuando la 3060 dibuje el cubo desde BMO-X (X5) y de la misma huella, no sera
"BMO-X dice que BMO-X esta bien": sera "BMO-X dibuja lo mismo que Windows con
el driver de NVIDIA, en el mismo silicio".

---

## 5. El potencial (lo que abre, de lo mas cerca a lo mas lejos)

| | que | por que el cubo lo hace posible |
|---|---|---|
| **1** | **X5: la 3060 dibuja el cubo sin Windows** | la imagen exacta ya existe; lo nuevo es el depth buffer (raster, color y semaforos ya funcionan) |
| **2** | **contestar la regla 4 en el silicio** | si BMO-X ve el mismo truncado a 12 bits, es del hardware; si no, era como el driver de Windows configuraba la salida de color. Nadie lo publica: se sabra midiendo |
| **3** | **el RHI de BMO-X con juez** (Ludoteca 16) | el backend CPU del RHI YA EXISTE: es este juez. Todo lo que se dibuje por el RHI tiene contra que compararse |
| **4** | **una GPU alternativa** | las 4 preguntas del README son el protocolo para medir cualquier tarjeta: el campo `Reglas` dice que cambia. Es el mismo aislamiento que el `Motor` del pase |
| **5** | **un "CTS de la casa"** | cada regla es una prueba; con texturas, mezcla y MSAA salen mas. Es lo que Khronos hace con su CTS, a la medida de lo que BMO-X dibuja |
| **6** | **PROTON-X** | el diccionario (cada llamada de D3D12 y su efecto en memoria y sincronia) y el censo; y la leccion: `d3d12.dll` muestra 2 importaciones, el resto va por las vtables COM que ningun censo ve |
| **7** | **fallos reproducibles** | un fotograma se reproduce con un NUMERO, en cualquier maquina: un bug de dibujo se manda como `gpu cubo 32`, no como una foto |

---

## 6. Lo que NO dice, dicho antes

- **1 pixel en 360 fotogramas** sigue sin explicacion.
- **Tres huellas, no 360.** El barrido entero se hizo en Windows; aqui se
  fijan tres. Mas capturas = mas huellas (X6).
- **Solo 1280x720, sin texturas, sin mezcla, sin MSAA.** El juez sabe
  exactamente lo que hace el cubo, y nada mas.
- **La regla 4 se midio a traves del driver de Windows.** Puede ser del
  silicio o de su configuracion: por eso es la pregunta de X5.
- **El soft-float de BMO-X es lento**: en el anfitrion el juez hace un
  fotograma en ~29 ms; en el Ryzen lo dira la fila `cubo`.

---

## 7. Las piezas, y donde estan

| pieza | donde |
|---|---|
| el estudio (Windows, no cruza) | EPICX-FRAMEWORK-DirectX12, rama `estudio-d3d`, carpeta `estudio-d3d/` (su censo, su diccionario y `scripts/reglas.ps1`): https://github.com/AndreeSalazar/EPICX-FRAMEWORK-DirectX12/tree/estudio-d3d |
| el juez, original | la misma rama, `cubo-neutro/` |
| el juez en BMO-X | `platform/shared/bmo-cubo` (con `PROCEDENCIA.md`: rama, hash, y lo unico que cambio) |
| las huellas de la 3060 | `platform/shared/bmo-cubo/src/referencia.rs` |
| la orden | `gpu cubo [fotograma]` (`Ultra_userspace/services/director/src/commands/gspcubo.rs`) |
| el plan que lo encadena | `docs/plan/PLAN_LA_LUDOTECA.md`, seccion 16 |

---

## 8. Las casillas

- [x] **X4a -- el juez, traido y comprobado.** `bmo-cubo` con su procedencia;
      17 pruebas (las 14 del original y 3 de `referencia`, entre ellas
      `el_juez_da_las_huellas_de_la_3060`). **Como se sabe:** `cargo test -p
      bmo-cubo`, y el binario freestanding soft-float da las tres huellas.
- [ ] **X4 -- `gpu cubo` en el Ryzen.** **En codigo (25-09), falta el metal.**
      El director dibuja el fotograma 30 por CPU (soft-float), lo pinta
      centrado y compara su huella. **Como se sabe:** la fila dice `IGUAL, bit
      a bit, a lo que D3D12 dibujo en la RTX 3060 bajo Windows`, y los us de
      CPU.
- [ ] **X5 -- el cubo por la 3060, sin Windows.** El mismo fotograma por
      AMPERE_B con depth buffer, y su huella contra la de D3D12. **Como se
      sabe:** la misma fila, con `la 3060` en vez de `la CPU`; y si no cuadra,
      el juez con `unorm8 = a_unorm8` (redondeo exacto) dice si la regla 4 era
      del driver.
- [ ] **X6 -- mas huellas.** Las 360 de la vuelta, generadas en Windows por
      `estudio-d3d` y apuntadas en `referencia.rs` (2,9 KiB). **Como se sabe:**
      `gpu cubo N` tiene contra que medirse para cualquier N de 0 a 359.
