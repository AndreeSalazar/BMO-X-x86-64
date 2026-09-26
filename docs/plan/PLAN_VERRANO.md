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
      TPC 1, SM 0): Out Of Range Register"**. No era el LDG (sin LDG tambien):
      era un REGISTRO. En Volta y despues, de los `REGISTROS = 16` que se le
      dan al programa, DOS se gastan en el contador de programa (NAK `sm70.rs`,
      `hw_reserved_gprs`): quedan R0..R13, y el de vertice usaba **R14** para
      `vertice * 32`. Arreglo: R1 (`tuberia::DESPLAZAMIENTO`), `cubo.bsf`
      refabricado, y el juez del SASS aprende la regla -- con ella, el de V0
      sale `TOMA TU BODRIO: R5 ... instruccion 6 (14)`, lo mismo que dijo la
      3060. Como se sabe: `gpu verrano` dice IGUAL.
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
