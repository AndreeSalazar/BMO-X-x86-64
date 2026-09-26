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
- [x] **V0c -- `gpu verrano` en el Ryzen.** **HECHO el 26-09 06:46:**
      `fotograma 30: la 3060 en 923 us, la CPU en 1796 us` e `IGUAL: VERRANO
      en la 3060 = VERRANO en la CPU = D3D12 en la 3060 bajo Windows`, con
      los programas del BSF (vertice 464 B, pixel 224 B) y 4 triangulos en UN
      dibujo. Lo que faltaba era R1 en vez de R14 (abajo). La 3060 dibuja el cubo con los
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
      **Metal 25-09 22:33, con el arranque POR DEFECTO (`init`):** `NO
      VERRANO en la 3060 no dibujo: motivo 71` -- sin el contexto de oro: el
      arranque ya no prepara el motor grafico. No llego a dibujar, asi que el
      bit 26 sigue sin medirse. Arreglo: `gpu verrano` y `gpu cubo 3060` dan
      antes los pasos que les falten hasta `lienzo`
      (`verificar::preparar_hasta`, en orden y con la marca `en curso`).
      **Metal 26-09 05:07, en frio:** `preparado: 24 paso(s)` y otra vez
      `estado SI vertices NO`, INTR/EXCEPTION/STATUS a 0: **el bit 26 no era
      (o no era solo)**. Dos instrumentos para el siguiente arranque: la fila
      `gsp aviso` detras de la escalera (un Xid 31 = FALLO DE PAGINA de lo que
      lee el LDG; un 13 = excepcion del sombreador) y `gpu verrano sinldg`,
      el mismo programa de vertice con los LDG cambiados por MOV de
      constantes: si ESE paga los vertices, el culpable es el LDG. Orden:
      primero `gpu cubo 3060` (X5, que dibujaba), despues `gpu verrano
      sinldg`, y al final `gpu verrano` -- un cuelgue deja el canal de GR
      muerto hasta reiniciar, asi que lo que falla va el ultimo.
      **Metal 26-09 06:33 -- LA CAUSA.** `gpu cubo 3060` IGUAL a D3D12 (el
      canal sano), y `gpu verrano sinldg` colgado en los VERTICES igual, pero
      ahora con `gsp aviso`: **Xid 13, "Graphics SM Warp Exception on (GPC 0,
      TPC 1, SM 0): Out Of Range Register"**: un REGISTRO. ([!] Esa
      corrida NO fue la variante sin LDG: `gpu.rs` pasaba " sinldg" con su
      espacio y la orden no lo reconocia, asi que corrio el programa normal.
      Arreglado el 26-09; la conclusion no depende de ella: el arreglo del
      registro dibujo IGUAL.) En Volta y despues, de los `REGISTROS = 16` que se le
      dan al programa, DOS se gastan en el contador de programa (NAK `sm70.rs`,
      `hw_reserved_gprs`): quedan R0..R13, y el de vertice usaba **R14** para
      `vertice * 32`. Arreglo: R1 (`tuberia::DESPLAZAMIENTO`), `cubo.bsf`
      refabricado, y el juez del SASS aprende la regla -- con ella, el de V0
      sale `TOMA TU BODRIO: R5 ... instruccion 6 (14)`, lo mismo que dijo la
      3060. Como se sabe: `gpu verrano` dice IGUAL.
- [x] **V1 -- el cubo en MOVIMIENTO, con fps.** N fotogramas seguidos por
      VERRANO sin leer de vuelta (solo la huella de algunos), para poner los
      fps de BMO-X al lado de la tabla de `estudio-d3d` (D3D12 ~3.800 fps en
      Windows). **Como se sabe:** `gpu verrano banco` dice los fps y que las
      huellas medidas siguen IGUALES.
      **Escrito (26-09), falta el metal:** `gpu verrano banco [N]` dibuja N
      fotogramas (360 por defecto, 1..3600) sin leer de vuelta, dice los fps
      de pared y los de solo la tarjeta, y al final lee el fotograma 30 y lo
      juzga contra `de_la_3060(30)` de D3D12.
      **Y afinado (26-09, tarde), tambien sin ver el metal:** el TABLERO en
      vivo debajo del cubo (fps, la 3060, preparar, el avance y una barra
      por fotograma; `gspcubo/tablero.rs`, repintado cada 66 ms y
      descontado de los fps); el kernel prepara EN CALIENTE del segundo
      fotograma en adelante (mismos programas, nadie mas en el GR, < 100
      ms: solo los vertices, sin releer; en frio eran ~4.000 lecturas por
      PCIe); `banco ligero` quita la escalera de T1c (2 WAIT_FOR_IDLE en vez
      de ~30); y el juez mira los programas del BSF antes de mandarlos. El
      fotograma juzgado va por el MISMO camino (caliente, y ligero si se
      pidio). **Como se sabe:** `gpu verrano banco` y `gpu verrano banco
      ligero` dicen IGUAL al final, y el `preparar` de los dos baja.
      **VISTO el 26-09 07:54** (`METAL_2026-09-25.md` 22): escalera 1246
      fps (la 3060 395 us, preparar 173), ligero 1726 fps (281 us, preparar
      168), el 30 IGUAL a D3D12 al final. Windows: ~3780. Por eso V1b.
- [x] **V1b -- EL ANILLO: la CPU ORQUESTA, no espera.** Lo que pidio el
      propietario tras ver 1726 contra ~3780: "mi CPU no tiene que esperar
      sino que ORQUESTE". `gpu verrano banco anillo`
      (`bmo_gpu_ga10x::anillo`, `gpu_trabajo/cubo.rs`):

      ```text
         los vertices   en RAM del PC: una pagina (IOVA 0x3A20_0000, VA
                        0x2_0013_0000, la entrada 304 de la PT del tramo),
                        prestada a la 3060 SOLO LECTURA. La CPU escribe a
                        velocidad de RAM, no ~0,5 us por palabra
         las ranuras    4: sus vertices (1 KiB) y sus ordenes (1 KiB de
                        EMPUJE). Las ordenes, las de `ligero` con UN semaforo
                        mas: la 3060 escribe en TABLA la direccion de los
                        vertices de su ranura (y WAIT_FOR_IDLE). Los programas
                        del BSF no cambian un bit
         la valla       un semaforo con el NUMERO del fotograma: la ranura
                        se reusa cuando la 3060 pago el de hace 4
         por fotograma  los vertices (RAM) y 17 palabras por la ventana (la
                        cola de las ordenes, la entrada y GP_PUT), en UNA
                        apertura y sin releer; el timbre. Sin invalidar la
                        MMU. Y se vuelve: el fotograma queda EN VUELO
         el escritorio  las tandas de los 360 angulos, contadas ANTES (la
                        coma flotante del escritorio es por software); y
                        `Backend::finish` espera lo que quede, DENTRO del
                        reloj
      ```

      El primer fotograma ARMA el anillo en frio (todo releido, y se espera
      a que se pague). Cualquier otro trabajo del GR (`gr_ocupado`) y LEER
      vacian el anillo antes: comparten el tramo. En el tablero, con
      fotogramas en vuelo, la columna del aparato es la ESPERA DE LA CPU
      por una ranura (0 si la 3060 va por delante), no lo que tarda la
      3060. **Como se sabe:** `gpu verrano banco anillo` dice IGUAL al
      final (el 30 va TAMBIEN por el anillo), `N en vuelo`, y los fps por
      encima de ligero; la meta, los ~3780 de Windows.
      **VISTO el 26-09 08:35** (`METAL_2026-09-25.md` 23): **2925 fps**
      contra 1378 de ligero en la misma sesion, preparar 21 us, 359 en
      vuelo, el 30 IGUAL. La CPU espera 247 us por ranura: ahora el cuello
      es la 3060 (en P8, relojes de reposo).
- [x] **V1c -- EXPRIMIR: quien refuerza a quien.** **VISTO el 26-09**
      (`METAL_2026-09-25.md` 24): anillo 2525 fps (la 3060 334 us), coopera
      7452 (103 us), maximo **28596** (18 us, la CPU espera 4): el 30 IGUAL
      en los tres. Ahora el cuello es la CPU (~35 us de pared). **Primer paso escrito
      (26-09, tras el metal de las 08:35): `gpu verrano banco coopera`.**
      La CPU esperaba el 72 % de cada fotograma; ahora, en vez de esperar,
      le QUITA trabajo a la 3060: sabe donde estaba el cubo y donde va a
      estar (`Frame::cover`, la caja de los vertices por el viewport, 2
      pixeles de margen), y la 3060 limpia SOLO esa union
      (`SET_CLEAR_RECT_*` + `USE_CLEAR_RECT`, `clc797.h`), no la ventana
      entera. El recorte viaja en la cabecera del paquete (bytes 20..28); el
      kernel lo usa en el anillo con `CUBO_COOPERA` y arma limpiando TODO.
      La limpieza pasa a la cola de cada ranura, y su espera es tambien la
      de la tabla: una espera menos. **Probado en el anfitrion:** en los 360
      fotogramas del giro y el 30, la imagen recortada es IGUAL a la entera,
      el margen minimo 2 pixeles, y se limpia un **14 %** de la ventana de
      media (`bmo_verrano::cpu`, `la_limpieza_recortada_da_lo_mismo`).
      **Como se sabe:** `gpu verrano banco coopera` dice IGUAL al final, la
      espera de la CPU baja y los fps suben sobre los 2925 del anillo.
      **Y la CPU EXIGE (26-09, sin metal todavia):**

      ```text
         exige    antes del banco, PERF_BOOST al GSP-RM (60 s): la CPU no
                  deja que la 3060 trabaje en P8; dice el P-state antes y
                  despues. `gpu verrano banco maximo` = coopera + exige
         marcas   cada fotograma deja el RELOJ de la 3060 al empezar y al
                  acabar (informe de cuatro palabras, `clc797.h`); el kernel
                  lee uno de cada cuatro antes de reusar la ranura. El
                  tablero vuelve a decir LA 3060 en us DE VERDAD, y la
                  ESPERA DE LA CPU en su propia columna
         menos    por fotograma solo se escriben por la ventana las palabras
                  que CAMBIAN (`anillo::cambian`: vertices, valla y, en
                  coopera, el recorte): 5 a 7 escrituras en vez de ~31
      ```

      **Como se sabe:** `gpu verrano banco maximo` dice `PERF_BOOST
      aceptado, P8 -> P0` (o por que no), LA 3060 baja, y el 30 IGUAL.

      **Lo que queda (la CPU es ahora el cuello, 35 us de pared contra 18
      de la 3060):** el paquete sin los ~700 B de programas en cada
      fotograma (paso 7); la cola de cada ranura en RAM y las entradas del
      GPFIFO escritas al armar (paso 2: una escritura por PCIe y el timbre);
      y medir el escritorio (el paquete, `Frame::cover` en coma flotante por
      software, la llamada) para saber de que son los ~20 us que no son del
      kernel.

      Lo pidio el propietario
      el 26-09: la CPU no espera ni le dice a la 3060 que hacer; la
      REFUERZA si hace falta. El anillo ya dice como decidirlo: la
      columna ESPERA DE LA CPU del tablero.

      ```text
         espera > 0   la 3060 es el cuello: la CPU va por delante y le
                      QUITA trabajo (menos esperas en sus ordenes, los
                      vertices en VRAM, mas ranuras) o hace otra cosa
                      (fisica, audio, el juego) en vez de girar
         espera = 0   la CPU es el cuello: lo que cuesta cada envio
      ```

      Lo que queda, en orden, cada paso una variable y juzgado por el 30
      contra D3D12 (las cifras son ESTIMADAS, no medidas):

      ```text
         1  MEDIR la 3060 en vuelo. Hoy, con el anillo, no se sabe cuanto
            tarda la tarjeta: solo cuanto espero la CPU. Un informe de
            CUATRO palabras (SET_REPORT_SEMAPHORE_D STRUCTURE_SIZE = 0,
            `clc797.h`) escribe ademas el reloj de la 3060 en ns: uno al
            empezar y otro al acabar cada fotograma. Sin esto, lo demas
            es a ciegas
         2  las ORDENES en RAM. La cola de cada ranura (14 palabras) hoy va
            por la ventana PRAMIN (~0,5 us por palabra). En RAM del PC,
            como los vertices, la CPU las escribe a velocidad de RAM; por
            la ventana quedan la entrada y GP_PUT (3 palabras). Y con las
            512 entradas del GPFIFO escritas al ARMAR, solo GP_PUT: 1
            escritura y el timbre, sin ninguna LECTURA por PCIe (la de la
            ventana tambien se va si se recuerda donde quedo). ~8 us -> ~1
         3  los VERTICES dentro de las ordenes. La clase 3D trae su propio
            copiador en linea (LINE_LENGTH_IN, OFFSET_OUT, LAUNCH_DMA,
            LOAD_INLINE_DATA): la 3060 los escribe en SU VRAM en orden,
            antes del dibujo. El programa de vertice lee VRAM y no PCIe, y
            la tabla deja de cambiar (sin su semaforo y su espera). Hay que
            ver si pide una espera antes del dibujo
         4  quitar ESPERAS. La de tras limpiar y la de antes de la valla
            probablemente sobran: la valla ya es "tras todas las escrituras,
            en todo el pipeline" (RELEASE bit 4, PIPELINE_LOCATION 15). Una
            a una
         5  SET_VERTEX_ID_BASE (0x1118, hoy a 0) como alternativa a la
            tabla: si el numero de vertice que ve el programa lo incluye,
            cada ranura es un desplazamiento y no un semaforo. Experimento
         6  mas ranuras (8) si la espera salta a rafagas
         7  el paquete SIN programas: hoy cada fotograma lleva los ~700 B
            del BSF y el kernel los huele (FNV). Con el anillo armado,
            solo los vertices
         8  V3 de verdad: la matriz (16 palabras) por LOAD_CONSTANT_BUFFER
            (0x238c) y los 8 vertices del cubo FIJOS en la VRAM: la 3060
            transforma, como D3D12. Programas nuevos en el BSF (y el juez)
      ```

      **Como se sabe:** cada paso sube los fps de `gpu verrano banco
      anillo` y el 30 sigue IGUAL; el 1 dice ademas cuanto de cada
      fotograma es de la 3060.
- [ ] **V2 -- la profundidad y el culling** (X5b de
      [`PLAN_EL_CUBO.md`](PLAN_EL_CUBO.md)): dos cubos que se tapan.
      **Como se sabe:** contra el juez con z-buffer.
- [ ] **V3 -- las constantes.** La matriz en un buffer y el programa de
      vertice multiplicando (lo que hace D3D): la pregunta del FMA, medida.
      **Como se sabe:** la huella de D3D12 sale con las cuentas en la 3060.
> **V3b y V4, hechos plan (26-09):** [`PLAN_LA_LENGUA_DE_LA_3060.md`](PLAN_LA_LENGUA_DE_LA_3060.md)
> -- los datos para estudiar (NAK, CUDA, el metal), donde vive cada pieza y
> las casillas J0..J2 (el juez) y E1..E5 (el emisor).

- [ ] **V3b -- EL JUEZ DEL SASS: si la GPU calla, el compilador habla.**
      Pedido del propietario (26-09): *"si la GPU no dice nada ... que el
      compilador diga ... que la GPU NO CALLE, si CALLA el compilador habla
      por nosotros"*. Un programa de SM86 mal hecho no da error: la 3060 se
      CUELGA o calcula basura en silencio (VERRANO V0, INTR/EXCEPTION/STATUS
      a 0). Asi que ANTES de que un programa entre en un BSF, un juez lo lee
      instruccion a instruccion y dice NO con el motivo: una lectura de un
      registro que carga un LDG/ALD sin esperar su barrera; una barrera que
      se espera y nadie escribe (o que se escribe y nadie espera); un stall
      mas corto que la latencia de la instruccion de antes; un registro por
      encima de `REGISTROS`; un LDG/STG con la SPH sin `DoesLoadOrStore`; un
      `AST` a un atributo que la SPH no declara; un `EXIT` con un `AST`
      pendiente. Es el guardian de la GPU: como `la-3060` protege el build
      del kernel, este protege lo que la 3060 va a ejecutar. **Como se
      sabe:** los programas que YA corrieron en el metal (T1c, T2a, X5,
      giro, blur, fractal) pasan; cada regla tiene un programa roto a
      proposito que el juez rechaza con SU motivo; y se pasa al de vertice
      de VERRANO V0 -- si dice NO, el motivo es la pista del cuelgue.
- [ ] **V4 -- el emisor SPIR-V a SM86.** El `kind` SM86 deja de ser "a
      mano": el SASS sale de `cubo.vert.spv` en el anfitrion, como el x86-64
      de S4, reusando el lector, el juez de SPIR-V y el oraculo de
      `PLAN_EL_SOMBREADOR.md` (neutros) y un emisor propio de SM86 que pone
      los bits de control -- espera, barreras, stall -- por regla, no a
      mano, y cuya salida pasa por el juez de V3b. **Como se sabe:**
      `Bsf::reproduce` comprueba tambien los objetivos SM86, y el juez de
      V3b no rechaza nada de lo que emite.
- [ ] **V5 -- Vulkan a VERRANO.** Las 67 funciones de vkQuake 0.50
      (Ludoteca 16) traducidas a VERRANO: el primer juego por la 3060.
      **Como se sabe:** vkQuake dibuja su primer fotograma y el backend CPU
      da lo mismo.

---

## 2c. EL ORDEN (26-09, tras los 28596 fps): tres carriles

Lo pidio el propietario tras ver `maximo`: *"organizar por completo esos 3,
pero que la CPU EXPRIMA"*. Los tres carriles, en ESTE orden, y por que:

```text
   E  LA CPU EXPRIME     primero: es la base. Lo que la CPU sabe y la 3060
                         no, y lo que la 3060 no dice y hay que medir
                         (PERFIL/GPU.txt). Sin esto, lo demas va a ciegas
   M  EL MOTOR           segundo: lo que falta para JUGAR -- que la 3060
                         haga el trabajo de un motor, no solo pinte
   P  PRESENTAR          ultimo: sin cortes en pantalla, y la carrera justa
                         contra D3D12. Lo mas caro, y lo que menos cambia
```

**Que es VERRANO, en una linea (y la respuesta a "un BSF con cubo como
Windows pero para jugar"):** SI. VERRANO es la API (lo que en Windows es
D3D12), el BSF el sobre con los programas ya traducidos para ESTA tarjeta
(lo que alli es el DXIL y la cache de sombreadores del driver), y el juego
tiene TODO el control: ni DWM, ni cadena de intercambio, ni driver ajeno
entre medias. El cubo es el primer juego; lo que sigue le da lo que un juego
pide.

**Y INTI, donde entra:** INTI emite coma flotante y SIMD DE VERDAD (`xmm`,
`emisor-x86_64/src/pruebas/simd.rs`); el escritorio de Rust no (su destino,
`x86_64-unknown-none`, la hace por software: las tandas del banco son
~33 us por fotograma asi). INTI no habla con la 3060 --eso lo hace el
BSF--, pero es el lenguaje natural de lo que la CPU calcula por un juego:
la fisica, las matrices, la logica. Entra en E6 y en M6.

### E -- LA CPU EXPRIME

- [x] **E0 -- lo hecho.** El anillo (V1b), `coopera`, `exige` y las
      marcas del reloj (V1c): `maximo` 28596 fps, la 3060 18 us, la CPU
      espera 4. Y el PERFIL de la 3060 escrito (`PERFIL/GPU.txt`, "lo que
      la 3060 no dice").
- [ ] **E1 -- el GOBERNADOR: exigir solo, y a tiempo.** Hoy `exige` es una
      palabra. El orquestador lo decide solo por lo que mide: si la ESPERA
      DE LA CPU sube (el cuello es la 3060), pide PERF_BOOST; y lo renueva
      antes de los 60 s mientras haya trabajo seguido, y lo suelta al acabar
      (`gpu relojes off`). **Como se sabe:** `gpu verrano banco coopera`
      con N grande (mas de 60 s) no se cae a P8 a la mitad.
- [ ] **E2 -- medir el escritorio.** ~35 us de pared: 15 de preparar (el
      kernel), 18 de la 3060 en paralelo, y ~20 que no se ven. Un
      cronometro por fases en el banco: armar el paquete, `Frame::cover`,
      la llamada, el tablero. **Como se sabe:** las fases suman la pared.
- [ ] **E3 -- el paquete SIN programas.** Con el anillo armado, cada
      fotograma lleva de nuevo los ~700 B del BSF y el kernel los huele
      (FNV) para saber que son los mismos. Un paquete "solo vertices" que
      nombra la huella del armado. **Como se sabe:** preparar baja.
- [ ] **E4 -- una escritura y el timbre.** La cola de cada ranura en RAM
      prestada (como los vertices) y las 512 entradas del GPFIFO escritas
      al armar: por fotograma, GP_PUT y el timbre, sin ninguna lectura por
      PCIe. **Como se sabe:** preparar por debajo de 5 us.
- [ ] **E5 -- mas ranuras si hace falta.** 8 en vez de 4, si con la 3060
      en P0 la espera salta a rafagas. **Como se sabe:** la grafica de la
      pared, lisa.
- [ ] **E6 -- la cuenta de la CPU con coma flotante de verdad.** Las
      tandas y `Frame::cover` hoy van por software. O el escritorio aprende
      SSE (el kernel ya guarda x87+SSE de Ring 3: `xcr0 0x3` en la ficha
      BEF2 de `d.bex`), o esa cuenta la hace INTI. Con M2 (la matriz en la
      3060) la mayor parte deja de ser de la CPU. **Como se sabe:** las
      tandas de los 360 angulos, de ~12 ms a menos de 1.

### M -- EL MOTOR

- [ ] **M1 = V2 -- la profundidad.** Un z-buffer en la VRAM y dos cubos que
      se tapan. **Como se sabe:** contra el juez de la CPU con z-buffer.
- [ ] **M2 = V3 -- la matriz en la 3060.** Los 8 vertices del cubo FIJOS en
      la VRAM, la matriz por `LOAD_CONSTANT_BUFFER` (0x238c, `clc797.h`)
      dentro de las ordenes -- 16 numeros por fotograma --, y el programa
      de vertice la aplica: lo que hace D3D12. Programas nuevos en el BSF,
      con su juez. **Como se sabe:** la huella del 30 contra D3D12, igual.
- [ ] **M3 -- las texturas.** Una textura en la VRAM y su muestreador (los
      descriptores TIC/TSC de Ampere). **Como se sabe:** contra el juez.
- [ ] **M4 = V4 -- el emisor SPIR-V a SM86.** Los programas del BSF salen
      de su GLSL, no escritos a mano. **Como se sabe:** el BSF refabricado
      da los mismos bytes que el SASS a mano en V0.
- [ ] **M5 = V5 -- Vulkan a VERRANO.** Las 67 funciones de vkQuake 0.50.
- [ ] **M6 -- un juego de OTRO proceso.** Hoy solo el escritorio habla con
      la 3060 (autoridad MAQUINA). Un juego --en C o en INTI-- le da sus
      fotogramas por el PASE (el lienzo prestado y el buzon, P1 de la GPU)
      y el escritorio orquesta. **Como se sabe:** el cubo girando desde un
      `.ibx`.

### P -- PRESENTAR

- [ ] **P1 -- dos superficies y el cambio en el VBLANK** (M2 de
      [`PLAN_LA_3060.md`](PLAN_LA_3060.md), C5 de
      [`PLAN_LA_3060_AFINADA.md`](PLAN_LA_3060_AFINADA.md)): dibujar en una
      mientras el monitor escanea la otra, y cambiarlas en el borrado. Sin
      cortes. **Como se sabe:** el banco con presentar, y ni un corte.
- [ ] **P2 -- la carrera justa contra D3D12.** Con P1, limpiar la ventana
      ENTERA y presentar cada fotograma, como hace `estudio-d3d`: esos fps
      SI se ponen al lado de los ~3780. **Como se sabe:** la tabla, con las
      dos columnas medidas igual.

## 2b. Los idiomas de las GPU, y donde se aisla cada uno (26-09)

Una GPU solo ejecuta SU codigo maquina; SPIR-V es el idioma de paso. Y los
codigos maquina NO son iguales:

```text
   NVIDIA   SASS, uno por generacion: SM75 Turing, SM86 Ampere (la 3060),
            SM89 Ada, SM120 Blackwell. Familia parecida desde Volta
            (instrucciones de 128 bits con los bits de control dentro), pero
            opcodes, latencias y cabecera cambian: NVIDIA NO garantiza el SASS
            entre generaciones (para eso existe PTX)
   AMD      el ISA de RDNA (GFX10, 11, 12): publicado por AMD, otro mundo --
            unidades escalar y vectorial, y esperas por contador (s_waitcnt)
   Intel    el ISA de Xe: registros GRF y marcas de dependencia por software
            (SWSB), otro mas
```

Lo que eso decide en BMO-X, con la ley de la casa (hardware PERFILADO):

```text
   NEUTRO, una vez     el lector de SPIR-V, su juez y el oraculo (PLAN_EL_SOMBREADOR)
   POR GPU, aislado    el emisor, su juez del SASS (V3b) y su ABI, en el crate
                       de ESA tarjeta: ga10x (SM86), rdna4 (GFX12), ...
   el sobre (BSF)      viaja el SPIR-V de origen y un objetivo por GPU (`kind`),
                       cada uno con su hash: la 3060 recibe solo el suyo
```

Una NVIDIA nueva no es un compilador nuevo: es un DIALECTO -- otra tabla de
opcodes y latencias y otra version de la cabecera, como un perfil de CPU. Una
AMD o una Intel si son otro emisor, pero en su propio carril: no toca ni una
linea del de la 3060 ni del lector neutro. El BSF no "entiende" el idioma --
lo entiende el compilador --; el BSF GARANTIZA que lo que llega a la GPU es
exactamente lo que el juez aprobo, y para que GPU se hizo.

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
