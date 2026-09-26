# PLAN VERRANO -- la API de dibujo de BMO-X, con el BSF debajo

> El propietario (2026-09-25), despues de ver el cubo IGUAL sin Windows:
> *"vamos con VERRANO pero no olvides con BSF = ese mismo BMO shader format
> para que la GPU no pierda el tiempo"*. El nombre lo puso el 23-09
> ([`PLAN_EL_SOMBREADOR.md`](PLAN_EL_SOMBREADOR.md)); el sitio en el mapa,
> la seccion 16 de [`PLAN_LA_LUDOTECA.md`](PLAN_LA_LUDOTECA.md) (el RHI).

---

## 0. La respuesta corta

```text
   que es        la API con la que un juego o una app de BMO-X DIBUJA:
                 un fotograma de vertices y una imagen; debajo, backends
   los backends  la CPU (el juez de siempre: da las huellas de D3D12)
                 la 3060 (dos programas FIJOS del BSF y los datos en un buffer)
   la regla      la GPU NO COMPILA NADA: los programas viajan ya traducidos
                 en el BSF (kind SM86), con su SPIR-V de origen y sus hashes;
                 de un fotograma a otro solo cambian los DATOS
   lo nuevo      en X5 cada triangulo llevaba su programa con los numeros
                 dentro (compilar en marcha, en chico); en V0 el programa es
                 uno y los numeros van en un buffer
```

---

## 1. Las piezas de V0 (y donde viven)

| pieza | donde | que hace |
|---|---|---|
| la API y el backend CPU | `platform/shared/verrano` (`bmo-verrano`, puro) | `Frame`, `Vertex`, `Image`, `Backend`; `cpu::Cpu::LA_3060` es el juez con la regla 4 |
| la fuente de los programas | `platform/drivers/gpu/ga10x/sombreadores/cubo.vert`, `cubo.frag` | GLSL 450; su SPIR-V (glslang de Khronos, en el anfitrion) al lado |
| el SASS de SM86 | `platform/drivers/gpu/ga10x/src/tuberia.rs` | los dos programas a mano, con codificadores que dan bit a bit instrucciones YA corridas en el metal |
| el sobre | `sombreadores/cubo.bsf` (2.928 B) | BSF con `kind::SM86`, `abi::SM86_V1`; lo fabrica y lo lee `ga10x/tests/bsf_sm86.rs` |
| el backend 3060 | el escritorio (`gspcubo/verrano.rs`) y el kernel (`gpu_trabajo/cubo.rs`, `CUBO_VERRANO`) | la app toma el codigo del BSF (hashes comprobados), arma el paquete y el kernel lo sube TAL CUAL |

**El ABI `SM86_V1`** (lo que el BSF promete de su codigo): la ranura `k`
de la tabla de buffers, su direccion (u64) en una VA que fija el dibujante
(V0: `0x2_0000_E600`); el de vertice recibe el numero de vertice en
`a[0x2fc]` y deja la posicion en `a[0x70]` y el generico 0 en `a[0x80]`; el
de pixel lee el generico 0 por IPA y deja el color en R0..R3; el codigo es
la SPH (128 B) y detras las instrucciones.

---

## 2. Las casillas

- [x] **V0a -- la API y su backend CPU.** `bmo-verrano`: `Frame` (limpieza y
      vertices), `Backend::draw`, `check`, y `cpu::Cpu` con `Unorm8::Exact`
      o `Truncate12`. **Como se sabe:** `cargo test -p bmo-verrano`:
      `da_las_huellas_de_d3d12` (0, 30, 60, con y sin la regla 4) y
      `da_el_modelo_de_la_3060` (23, 32 y 102 contra
      `referencia::como_la_3060`, salvo el pixel sin explicar, que no es de
      la cuenta).
- [x] **V0b -- el BSF aprende la 3060.** `kind::SM86 = 2`,
      `abi::SM86_V1 = 2`; `cubo.bsf` lleva el SPIR-V de `cubo.vert` y
      `cubo.frag` y su SASS. **Como se sabe:** `cargo test -p bmo-gpu-ga10x
      --test bsf_sm86`: lo que se toma del sobre es byte a byte lo que el
      kernel escribe, el fichero del repositorio es el que sale
      (determinista), y un bit cambiado del codigo no pasa.
- [ ] **V0c -- `gpu verrano` en el Ryzen.** La 3060 dibuja el cubo con los
      dos programas del BSF y los vertices en un buffer, en UN dibujo. Lo
      nuevo en el metal: los `LDG` en un programa de VERTICE (hasta hoy solo
      en computo), el IPA del cuarto canal y un dibujo de `3n` vertices.
      **Como se sabe:** la fila dice `IGUAL: VERRANO en la 3060 = VERRANO en
      la CPU = D3D12 en la 3060 bajo Windows` en el 0, el 30 y el 60; `IGUAL,
      pixel a pixel` en los demas (el 32, con su pixel sin explicar dicho
      aparte). Si no, `el primero en (x, y)` y la escalera de siempre.
      **Metal 25-09 20:19: NO, la escalera se para en los VERTICES** (el
      dibujo con el rasterizador apagado: solo corre el programa de
      vertice), sin excepcion e INTR/EXCEPTION/STATUS a 0. El programa de
      vertice colgado. **El arreglo (26-09):** su SPH no decia que lee
      memoria. NAK (`sph.rs`: `set_does_load_or_store(uses_global_mem)`) y
      nvc0 (`hdr[0] |= 1 << 26` con `io.globalAccess`) ponen
      `CommonWord0.DoesLoadOrStore` (bit 26) en cuanto un programa hace
      LDG/STG; el computo no lo necesita porque no tiene SPH (manda su QMD),
      y por eso el LDG de `giro` y `blur` corria. `tuberia::sph_vertice` lo
      pone, `raster::LEE_O_ESCRIBE` lo nombra, y `cubo.bsf` se refabrico
      (`BSF_FIJAR=1`): la misma medida, otra huella. Si aun asi se para en
      los VERTICES, lo siguiente es el LDG mismo: la direccion de la tabla
      (`0x2_0000_E600`) en el espacio del canal de GR.
- [ ] **V1 -- el cubo en MOVIMIENTO, con fps.** N fotogramas seguidos por
      VERRANO sin leer de vuelta (solo la huella de algunos), para poner los
      fps de BMO-X al lado de la tabla de `estudio-d3d` (D3D12 ~3.800 fps en
      Windows). **Como se sabe:** `gpu verrano banco` dice los fps y que las
      huellas medidas siguen IGUALES.
- [ ] **V2 -- la profundidad y el culling** (X5b de
      [`PLAN_EL_CUBO.md`](PLAN_EL_CUBO.md)): dos cubos que se tapan.
      **Como se sabe:** contra el juez con z-buffer.
- [ ] **V3 -- las constantes.** La matriz en un buffer y el programa de
      vertice multiplicando (lo que hace D3D): la pregunta del FMA, medida.
      **Como se sabe:** la huella de D3D12 sale con las cuentas en la 3060.
- [ ] **V4 -- el emisor SPIR-V a SM86.** El `kind` SM86 deja de ser "a
      mano": el SASS sale de `cubo.vert.spv` en el anfitrion, como el x86-64
      de S4. **Como se sabe:** `Bsf::reproduce` comprueba tambien los
      objetivos SM86.
- [ ] **V5 -- Vulkan a VERRANO.** Las 67 funciones de vkQuake 0.50
      (Ludoteca 16) traducidas a VERRANO: el primer juego por la 3060.
      **Como se sabe:** vkQuake dibuja su primer fotograma y el backend CPU
      da lo mismo.

---

## 3. Lo que V0 NO es, dicho antes

- **No es Vulkan.** Es mas chico a proposito: un fotograma entra y sale
  entero. Vulkan se TRADUCE a VERRANO (V5), no al reves.
- **Un solo par de programas** (el del cubo) y un solo tipo de vertice.
- **Sin profundidad ni culling**: el cubo es convexo y la app le da las
  caras de delante (probado en `bmo_cubo::tanda`).
- **El SASS es a mano** (V4 lo cambia); el SPIR-V es la fuente y el BSF los
  ata por hash.
- **La capa 5 del BSF** (`deep`) no mira aun los modulos de vertice y de
  pixel: el lector de SPIR-V solo sabe de `GLCompute`.
