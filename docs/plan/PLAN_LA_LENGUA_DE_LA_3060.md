# PLAN LA LENGUA DE LA 3060 -- SPIR-V a SM86, con un juez que no calla

> Escrito el **2026-09-26**, con VERRANO V0 colgado en los VERTICES sin una
> sola excepcion de la 3060 (INTR, EXCEPTION y STATUS a 0). El propietario:
> *"darle un compilador a la GPU ... frontend y backend maestro, el que habla
> idioma, luego el juez vigila y luego que diga PERFECTO Y PRECISO y luego BSF
> lo entrega ... si mi juez dice que no le dice TOMA TU BODRIO"*.
> Mismo formato que [`PLAN_EL_SOMBREADOR.md`](PLAN_EL_SOMBREADOR.md): casillas
> en orden, cada una con **que la bloquea** y **como se sabe que quedo hecha**.

Es la casilla V4 de [`PLAN_VERRANO.md`](PLAN_VERRANO.md) hecha plan, con su
juez (V3b) delante. Los idiomas de las GPU y donde se aisla cada uno: seccion
2b de ese plan.

---

# 0. LA IDEA EN UN DIBUJO

```text
   cubo.vert (GLSL)                                     en el ANFITRION, una vez
        |  glslang de Khronos (como hoy)
        v
   SPIR-V --> LECTOR --> JUEZ DE SPIR-V --> ORACULO       NEUTRO (ya existe:
        |     (S1)        (S2)               (S3)          toolchain/lang/spirv)
        v
   EL EMISOR SM86  -- el que habla el idioma de la 3060   POR GPU (nuevo)
        |  instrucciones + registros + BITS DE CONTROL
        v
   EL JUEZ DEL SASS -- lee lo que salio, instruccion a instruccion
        |
        +--> "TOMA TU BODRIO: <el motivo, la instruccion, el registro>"
        |        y NO hay BSF: el build se para, como `la-3060`
        |
        +--> "PERFECTO Y PRECISO"
                 v
   EL BSF  -- el SPIR-V de origen + el objetivo SM86 + sus hashes
        v
   el kernel lo sube TAL CUAL ... y la 3060 no compila nada, nunca
```

**Por que el juez es la pieza central y no un extra.** En una CPU, un
programa mal hecho explota con una excepcion que dice donde. En la 3060 NO:
cada instruccion lleva sus bits de control -- cuantos ciclos esperar, que
barrera encender al acabar una carga, que barreras esperar antes de usar un
dato -- y el hardware **no comprueba nada: obedece**. Un numero mal puesto se
cuelga o da basura en silencio. Si la GPU calla, alguien tiene que hablar
antes: el juez. Y habla en el anfitrion, donde un NO cuesta un segundo, no un
arranque.

---

# 1. LOS DATOS PARA ESTUDIAR (ya estan, y de donde)

No se adivina nada: todo lo de abajo esta descargado o ya corrio en el metal.

| fuente | que da | para que pieza |
|---|---|---|
| **NAK** (Mesa 26.2.3, `src/nouveau/compiler/nak/`, MIT) -- el compilador de sombreadores de NVK, en Rust | `sm70_encode.rs` (4.596 lineas): el codificador de SM70 en adelante, SM86 incluido | el emisor (E2) |
| | `sm80_instr_latencies.rs` (1.639): las latencias de **Ampere y Ada** -- el propio fichero dice que son *"la informacion de planificacion de registros que dio NVIDIA"*. Instrucciones **acopladas** (latencia fija: necesitan ESPERA) y **desacopladas** (latencia variable: necesitan BARRERA) | el juez (J1) y el planificador (E4) |
| | `calc_instr_deps.rs` (1.201): como se reparten las 6 barreras y como se calculan las esperas | el juez y el planificador |
| | `sph.rs` (609): la cabecera de 128 bytes, bit a bit (de ahi salio el bit 26) | el emisor |
| | `assign_regs.rs`, `legalize.rs`, `from_nir.rs` | registros y forma legal de cada instruccion |
| | `nvdisasm_tests.rs` (1.103): cada codificacion comparada con el desensamblador de NVIDIA | como se prueba el emisor |
| **CUDA de NVIDIA** (PyPI: `nvcc` con `ptxas`, `nvdisasm`, `cuobjdump`) | `ptxas` da SASS de SM86 real; `nvdisasm` lee cualquier palabra de 128 bits | el oraculo de la codificacion (como hoy: `giro`, `blur`, `fractal`) |
| **el metal de BMO-X** | los programas que YA corrieron en la 3060: T1c, T2a, X5 (el cubo), `giro`, `blur`, `fractal`, `lienzo` | el corpus de oro del juez: tienen que pasar |
| **el juez neutro del cubo** (`bmo-cubo`, la CPU) | la imagen exacta, pixel a pixel | el que dice si lo que la 3060 dibujo es lo que el programa decia |

**Lo que se toma de NAK, y como:** se ESTUDIA y se cita (fichero y funcion,
como ya se hizo con la cabecera y el `AST`). Las tablas de latencias son
DATOS, no codigo: se copian como tabla con su procedencia (MIT, Collabora y
Red Hat) en un `PROCEDENCIA.md` al lado, igual que `bmo-cubo`. El codigo del
emisor es de BMO-X: NAK sale de NIR (el IR de Mesa); aqui se sale del modulo
del lector de SPIR-V que ya existe.

---

# 2. DONDE VIVE CADA PIEZA (el aislamiento)

```text
   toolchain/lang/spirv/            NEUTRO: lector, juez de SPIR-V, oraculo (ya)
   toolchain/lang/spirv/emisor-sm86 NUEVO: SPIR-V -> SASS de SM86 (host)
   platform/drivers/gpu/ga10x/src/sass/
        codifica.rs                 las instrucciones de SM86, bit a bit
        latencias.rs                la tabla de Ampere (datos, con procedencia)
        juez.rs                     EL JUEZ DEL SASS -- no_std, sin alloc
```

**El juez vive en el crate de la tarjeta, y es `no_std`:** asi lo puede
llamar el build (antes de fabricar el BSF) **y el kernel** (antes de subir un
programa a la 3060). Hoy el kernel sube el codigo "TAL CUAL"; con el juez, lo
sube tal cual **y juzgado**. Dos puertas, un solo juez: un programa que no
salio del emisor -- escrito a mano, como el de VERRANO V0 -- tambien pasa por el.

Una AMD tendria su `rdna4/src/isa/juez.rs` con SUS reglas (`s_waitcnt`), y una
NVIDIA nueva otra tabla de latencias. El lector de SPIR-V no se entera.

---

# 3. LAS CASILLAS

## [x] J0 -- EL CORPUS DE ORO

Los programas que ya corrieron en el metal, sacados a una lista con su nombre
y su origen: `raster` (T1c), `color3d` (T2a), `cubo` (X5), `giro`, `blur`,
`fractal`, `lienzo`, `triangulo`, `escena`, `pantalla`. Y el de vertice de
VERRANO V0 aparte, marcado **sospechoso**.

- **Bloquea:** nada.
- **Como se sabe:** una prueba recorre la lista y cada programa se decodifica
  entero (ninguna instruccion desconocida para el decodificador de J1).
- **Hecho (26-09):** `ga10x/src/sass/corpus.rs` -- `ORO`, 15 programas
  (`sombreador`, `lienzo`, `blur`, `fractal`, `triangulo`, `escena`, `giro`,
  `pantalla`, `video`, y el de vertice y el de pixel de T1c, T2a y X5), y
  `SOSPECHOSOS`, 3 (los de VERRANO V0 y la variante sin LDG).

## [x] J1 -- EL JUEZ DEL SASS: las reglas, cada una con su programa roto

Un decodificador de lo que BMO-X ya emite (LDG, STG, ALD, AST, IPA, MOV, IMAD,
IADD3, LOP3, FFMA, ISETP, BRA, EXIT, NOP, S2R...) y seis reglas. Cada NO
empieza por `TOMA TU BODRIO:` y dice la instruccion, el registro y la regla:

```text
   R1  dato leido antes de llegar     un registro que escribe una instruccion
                                      DESACOPLADA (LDG, ALD, IPA, S2R...) se lee
                                      sin esperar SU barrera
   R2  espera corta                   una instruccion ACOPLADA (IMAD, IADD3...)
                                      se lee antes de su latencia (tabla de
                                      Ampere): la suma de esperas no llega
   R3  barrera fantasma               se espera una barrera que nadie enciende,
                                      o se enciende una que nadie espera
   R4  fuente pisada                  se escribe un registro que una carga en
                                      vuelo aun esta LEYENDO (falta la barrera
                                      de lectura)
   R5  la cabecera miente             LDG/STG sin `DoesLoadOrStore` (bit 26);
                                      un AST a un atributo que la SPH no
                                      declara; un registro >= REGISTROS
   R6  final sucio                    EXIT con un AST o un STG pendiente
```

- **Bloquea:** J0.
- **Como se sabe:** (a) TODO el corpus de oro dice `PERFECTO Y PRECISO`; (b)
  cada regla tiene un programa roto a proposito que el juez rechaza con ESA
  regla y no con otra; (c) se le pasa el de vertice de VERRANO V0 y se apunta
  lo que dice -- si dice NO, su motivo es la pista del cuelgue, sin gastar un
  arranque; si dice que si, el cuelgue no es de los bits de control y se dice.
- **Hecho (26-09):** `ga10x/src/sass/juez.rs`, `no_std` y sin memoria
  dinamica. Lo que mostraron los 15 de oro al calibrarlo, cada cosa ya en el
  codigo con su fuente:
  - una fuente AUSENTE deja su campo a 0, que es R0: cada opcode dice
    cuantas fuentes tiene (NAK `encode_alu_src2` no escribe nada si no hay);
  - una desacoplada lee sus fuentes AL EMITIRSE, salvo que encienda barrera
    de LECTURA (la tabla de Ampere da 1 ciclo de WAR, y `ptxas` cuenta con
    ello); y esperar su barrera de ESCRITURA libera tambien sus fuentes;
  - el `BRA .` de relleno detras del EXIT no es un bucle.
  **El veredicto sobre VERRANO V0: PERFECTO Y PRECISO.** Esperas, barreras,
  fuentes, registros y cabecera estan bien: el cuelgue en los VERTICES NO es
  de nada de eso. Lo que queda es lo que v1 no mira -- la direccion que lee el
  LDG y si esta mapeada para ese canal --, y eso lo dira `gsp aviso` (un Xid
  31 es un fallo de pagina). La variante `sinldg` tenia DOS esperas a
  barreras que ya nadie encendia (R3): se limpiaron, para que la prueba de
  una variable cambie solo las cargas.

  **[!] Y ese veredicto estaba MAL (26-09 06:33).** La 3060 dijo `Xid 13, Out
  Of Range Register`: el juez v1 contaba los registros contra `REGISTROS`,
  pero en Volta y despues DOS se gastan en el contador de programa (NAK
  `hw_reserved_gprs`) y el de vertice usaba R14 con 16. El juez aprendio la
  regla (`juez::RESERVADOS`) y ahora dice `TOMA TU BODRIO: R5 ... instruccion
  6 (14)` con el de V0; con R1, VERRANO dibujo IGUAL a D3D12 (06:46). Y la
  prueba `con_lo_que_se_le_envia` juzga cada programa con los registros que
  de verdad van en las ordenes y los QMD.

## [x] J2 -- EL JUEZ EN LAS DOS PUERTAS

> **La del build, hecha (26-09):** `tests/bsf_sm86.rs` juzga los dos
> programas antes de fabricar `cubo.bsf`; con un bodrio, no hay sobre. Y lo
> dice: `cubo.bsf cubo_vertice: PERFECTO Y PRECISO: 20 instrucciones, 32
> lecturas y 4 esperas comprobadas`. Falta la del kernel.
>
> **Y la del escritorio (26-09, tarde):** `juez::juzgar_programa` juzga un
> programa TAL COMO VIAJA (la SPH y las instrucciones en bytes, lo que guarda
> el BSF). `gpu verrano` lo llama sobre los dos programas del sobre antes de
> armar el paquete: con un bodrio no se manda nada y lo dice (`NO  el
> programa de vertice del BSF: TOMA TU BODRIO: ...`); con los dos buenos, el
> veredicto sale en la pantalla y en el tablero del banco. Queda la puerta
> del kernel, que es la que no se puede saltar.
>
> **Y la del kernel, hecha (26-09):** `CUBO_VERRANO` juzga antes de subir y
> un bodrio vuelve con `IOMMU_NO_BODRIO` (87). Ver A3 de
> [`PLAN_EL_AISLAMIENTO.md`](PLAN_EL_AISLAMIENTO.md). Falta verlo en el metal
> con un sobre roto a proposito.

`bsf_sm86` (la prueba que fabrica `cubo.bsf`) llama al juez y no escribe el
sobre si dice NO. El kernel llama al mismo juez en `CUBO_VERRANO` antes de
subir los programas, y un NO es un motivo nuevo en la ABI (con su guardian
`la-3060` contando motivos en los tres sitios).

- **Bloquea:** J1.
- **Como se sabe:** un `cubo.bsf` con un bit de control cambiado no se
  fabrica; el mismo programa enviado a mano al kernel vuelve con el motivo y
  la 3060 no llega a verlo.

## [x] E1 -- EL SUBCONJUNTO DE SPIR-V PARA LA 3060

El de vertice y el de pixel del cubo (`cubo.vert`, `cubo.frag`), y el de
computo que ya entiende el lector (`GLCompute`): entradas por ubicacion,
salidas a atributos, buffers por su ranura del ABI `SM86_V1`, aritmetica
entera y de coma flotante sin FMA fusionada salvo que el SPIR-V la pida. Lo
que no entra se rechaza CON MOTIVO, como en S2.

- **Bloquea:** nada (el lector y el juez de SPIR-V ya existen).
- **Como se sabe:** el juez de SPIR-V acepta `cubo.vert.spv` y
  `cubo.frag.spv` con la etapa correcta, y rechaza con su nombre lo que falta.
- **Hecho (27-09), en dos pisos:**
  - el juez NEUTRO aprendio la etapa: `validate_stage(&Module, Stage)` en
    `toolchain/lang/spirv/src/validator.rs`. `Vertex` y `Fragment` (con
    `OriginUpperLeft`) ademas de `GLCompute`; `Output`; cada entrada o
    salida con su `Location` o un `BuiltIn` de SU etapa (`gl_PerVertex`, por
    miembro); otra etapa es `WrongStage` (`se pidio Fragment, es Vertex`), y
    sin nada es `NoLocation`. `validate` sigue siendo el de S2, solo computo.
  - el subconjunto de la 3060 en su crate, `emisor-sm86/` (`bmo-spirv-sm86`,
    `no_std` sin `alloc`): `check(&Module, Stage) -> Fit` pasa primero por el
    juez neutro y despues por el ABI `SM86_V1`: buffer en `DescriptorSet 0,
    Binding k < 16` (`BufferOffSlot`), el vertice entra por `VertexIndex` y
    sale por `Position` y el generico 0, el pixel lee el generico 0 y deja el
    color 0 (`BuiltInOffAbi`, `VertexInputByLocation`, `LocationOffAbi`,
    `AttributeNotFloat`), computo sin `NumWorkgroups`, un solo punto de
    entrada, y **sin trascendentes** (`Transcendental`: la 3060 las da con
    `MUFU` y el oraculo las define a un ULP; dos respuestas no). La FMA no se
    RECHAZA -- en SPIR-V no se puede escribir una que nadie pidio -- sino que
    se CUENTA: `Fit::fused` es el numero de `FFMA` que E3 puede poner.
  - `cargo test -p bmo-spirv-sm86`: `cubo.vert.spv` cabe como `Vertex`
    (ranura 0, generico 0), `cubo.frag.spv` como `Fragment`, y siete de
    computo del banco del lector como `GLCompute`; con la etapa cambiada,
    `WrongStage`; y una fila por regla sobre el MISMO modulo de verdad con
    una palabra cambiada, que cae con ESA regla (y `trascendentes.spv`, de
    verdad, cae en `Sin`).
  - **Lo que NO prueba:** que el emisor cumpla (E3 y J1); que el computo con
    `SM86_V1` corra en el metal -- el ABI se escribio para el cubo --; y `flat`
    contra lo interpolado sigue como lo deja `tuberia.rs` (el cubo lo juzga).
    Solo la Location 0 cabe porque el ABI solo nombra el generico 0 y el
    color 0: un segundo atributo es una version nueva del ABI, no esta casilla.

## [x] E2 -- EL CODIFICADOR, bit a bit contra NVIDIA

`sass/codifica.rs`: cada instruccion que el emisor use, con una prueba que
compara la palabra de 128 bits con la que da `ptxas` y lee `nvdisasm` -- la
misma disciplina que `codifican_lo_que_ya_corrio` de hoy.

- **Bloquea:** J1 (comparte el decodificador).
- **Como se sabe:** cada codificacion tiene su palabra de oro; `nvdisasm`
  lee de vuelta el texto esperado.
- **Hecho (28-09):** `ga10x/src/sass/codifica.rs`: FADD, FMUL, FFMA (con
  registro, inmediato o constante; `-`, `|x|` y `.SAT`), FMNMX (menor y
  mayor), MUFU (RCP, RSQ, SQRT), MOV (registro, inmediato, constante), EXIT y
  NOP -- lo que pide el emisor para el cubo de VERRANO y para BMOX-12 (dp4,
  dp3, mad, rsq, saturar). Las 24 PALABRAS DE ORO son de `ptxas -arch=sm_86`
  (CUDA 12.9, de PyPI) sobre `sombreadores/oro_codifica.ptx`, leidas con
  `nvdisasm -hex` (13.4): cada codificador da los 128 bits, control
  incluido. Y 10 combinaciones que ptxas NO dio (FFMA con inmediato,
  `FFMA.SAT R5, -R6, R7, -R8`, `FMNMX R1, |R2|, c[0x3][0x40], !PT`, MOV entre
  registros...) fabricadas por el codificador: `nvdisasm -b SM86` las lee
  como dice su texto. El juez (J1) conoce las 34 (`juez::conoce`).
  Saboteado: el `.SAT` un bit mas arriba, cae. Lo que NO: la raiz de
  `MUFU.RSQ` es la APROXIMADA de la 3060, no la exacta de la casa -- cuanto
  se separa lo mide E3 --; y el control se recibe, no se calcula (E4).

## [x] E3 -- EL EMISOR: SPIR-V a SASS, en linea recta

Traducir el modulo del lector a instrucciones de SM86 con registros
asignados, sin optimizar: primero correcto. Los bits de control, aun
CONSERVADORES (esperar siempre lo maximo).

- **Bloquea:** E1, E2.
- **Como se sabe:** el oraculo (S3) ejecuta el SPIR-V y un simulador de SASS
  en el anfitrion ejecuta lo emitido: mismos resultados en los vertices del
  cubo; y el juez dice PERFECTO Y PRECISO.
- **Hecho (28-09), con una decision del propietario:** la entrada NO es
  SPIR-V sino el `Programa` de la casa de PROTON-X -- el que ya sale de los
  DXIL de dxc y de los SM5 de FXC y que la CPU ya corre --: un emisor para
  BMOX-12 y lo que venga, con el interprete de la casa de oraculo. Para que
  Ring 3 lo pueda usar sin enlazar un driver (L8), el codificador de E2 salio
  a `platform/shared/bmo-sm86` (puro, sin dependencias) y el emisor vive en
  `platform/shared/proton-x-sm86` (regla S de `la-3060`: lo de NVIDIA en lo
  suyo). `emitir(&Programa, registros)`: FMUL/FADD sin fundir (Mad, Dot),
  FADD.SAT y FADD |x| con -0, FMNMX, MUFU.RSQ/SQRT, MOV desde constantes e
  inmediatos, EXIT; registros asignados y devueltos tras su ultimo uso;
  control CONSERVADOR (ALU; un MUFU enciende la barrera 0 y la siguiente la
  espera). ABI del banco (`V0`): entradas en c[1], cbuffer b0 en c[3],
  salidas en R(4e+k). `simula` corre ese SASS leyendo los mismos bits.
  **Como se sabe:** los 4 sombreadores del cubo (DXIL y SM5) dan en el
  simulador los BITS de `Programa::correr` (24 vertices x 4 fotogramas, 24
  pixeles x 3); un programa a mano con CADA operacion y los valores que
  muerden (NaN, infinitos, -0, subnormales) da los bits de la casa en 2873
  comparaciones; el juez (J1) dice PERFECTO Y PRECISO de los cuatro (en el
  banco del driver). Saboteado: la resta sin negar, sin `.SAT`, el simulador
  leyendo mal el destino, min por max, `|x|` sin abs, el inmediato sin
  negar: caen; un MUFU sin su barrera: el juez dice R3. Cifras: el de
  vertice 72 instrucciones y 22 registros, el de pixel 33 y 10 (1 MUFU).
  **Lo que NO:** `Div` se rechaza (la de la 3060 no es la exacta); MUFU esta
  MODELADO como la casa, la 3060 aproxima (lo mide E5); 22 registros pasan
  de los 16 de la tuberia de hoy (E4/E5 suben la cuenta o la bajan); el ABI
  es el del banco, no el de la tuberia (ALD/AST/IPA, E5).
  **09-10 (DL10 de PLAN_LAS_LIBRERIAS, LB6b):** `Div` ya se emite, EXACTA:
  la cuenta de `ptxas` para `div.rn.f32` rehecha con la lista blanca de R7 y
  con la prueba de Tuckerman en el redondeo (`proton-x-sm86/src/cociente.rs`),
  los bits de IEEE aunque el MUFU.RCP se equivoque -- el simulador lo mueve
  hasta 4 ULP a cada lado --. Cuesta 105 instrucciones. La raiz sigue como
  MUFU, modelada como la casa.
  **09-10 (LB7a de PLAN_LAS_LIBRERIAS):** los registros, FRUGALES cuando no
  caben. Si un cuerpo no cabe en los que le dan (64 en VERRANO), el emisor lo
  intenta otra vez: lo que sube a un registro (una constante, una entrada del
  banco) no se queda en el, se vuelve a subir cada vez que se lee; cada
  variable recibe su registro justo antes de lo primero que la toca, fuera de
  todo `si` y bucle; y lo que nadie lee se suelta en cuanto se escribe
  (`emitir_libreta`, y `nace` y `sobra` en `saltos.rs`). Mas MOV y menos
  registros: la matriz de un fotograma del cubo (`cubo.wvp` de
  `nivel11/cubo_gira`) pasa de 78 a 44. Lo que ya cabia sale igual, byte a
  byte. Lo prueba la bateria de E6 con el modo forzado: 400 programas al
  azar, en los dos ABI, con los bits de la casa y el si del juez.

## [x] E4 -- LOS BITS DE CONTROL POR REGLA

Las esperas y las 6 barreras calculadas con la tabla de Ampere (J1 las
juzga con la MISMA tabla), como `calc_instr_deps.rs`. Aqui deja de haber bits
de control escritos a mano en todo BMO-X.

- **Bloquea:** E3.
- **Como se sabe:** el juez sigue diciendo PERFECTO Y PRECISO, y el programa
  es mas corto en ciclos que el conservador de E3 (medido en el simulador).
- **Hecho (28-09):** `proton-x-sm86/src/planifica.rs`: el emisor pone cada
  instruccion con lo que lee y escribe (su clase: ALU, FMA, MUFU) y el
  planificador calcula la espera JUSTA con la tabla de Ampere del juez
  (`latencia`), y a cada MUFU una de las 6 barreras, que espera el primero
  que lee (o pisa) su resultado. Y para caber en la puerta del kernel (64
  instrucciones): un valor que se sube a un registro se queda en el hasta su
  ultimo uso (la entrada que se leia 4 veces se carga UNA), los operandos de
  lo conmutativo al reves si ahorra un MOV (y si ninguno esta en registro,
  entra el que vive mas), el resultado que es una salida se calcula YA en su
  registro de salida, y el producto escalar se acumula en su destino.
  **Como se sabe:** vertice 72 -> 50 instrucciones y 385 -> 116-128 ciclos;
  pixel 33 -> 28 y 163 -> 72-75 (la prueba exige <= 64 y menos de la MITAD
  que E3); los bits siguen siendo los de la casa (las 4 pruebas de E3); el
  juez dice PERFECTO Y PRECISO de los cuatro; `nvdisasm -b SM86` lee las 156
  instrucciones sin un error. Saboteado: la latencia FMA->FMA a 2, el juez
  dice R2; sin esperar la barrera del MUFU, R1. Lo que NO: 18-19 registros
  en el de vertice (la tuberia de hoy da 16: E5 sube REGISTROS, que la SPH
  permite); y la tabla es la del juez para estas clases, no la de NVIDIA
  entera.

## [x] E5 -- EN EL METAL: el cubo con programas EMITIDOS

`cubo.bsf` deja de llevar SASS a mano: lo fabrica el emisor. `gpu verrano`
dibuja el cubo con ese sobre.

- **Bloquea:** E4, J2 -- y que VERRANO V0 deje de colgarse, o que el juez
  diga por que se colgaba.
- **Como se sabe:** `IGUAL: VERRANO en la 3060 = VERRANO en la CPU = D3D12`
  en el 0, el 30 y el 60.
- **Hecho (28-09), lo que no es el metal:**
  - El emisor recibe entradas y cbuffer YA en registros (`Abi::Registros`,
    los cargara el pegamento con LDG) y dice cuales y donde (`precargas`).
    Los 4 sombreadores dan los bits de la casa con todo lo demas en NaN y
    el banco vacio; el juez juzga los dos ABI. Vertice 44 instrucciones.
  - La puerta del kernel (Ring 0, dicho que si): `MAX_INSTRUCCIONES` 64 ->
    128, y cada programa de VERRANO de 512 / 256 B a `HUECO` = SPH + 2 KiB
    (cada uno tiene su pagina entera). El juez ya no copia el programa a la
    pila del kernel: lo lee donde esta. La caja del escritorio mide el
    paquete mas grande (`MAX_PAQUETE`).
  - El paquete lleva DATOS (el cbuffer y los vertices sin transformar,
    palabra 28 de la cabecera) y VERRANO da 64 registros a sus programas.
  - El pegamento (`ga10x/src/trabajos/pegamento.rs`): ALD, la tabla, LDG de
    cada entrada y cada fila, el cuerpo, AST de la posicion y de los dos
    genericos; el de pixel, IPA y LDG de la luz. Juez: PERFECTO.
  - Los programas de BMOX-12 (los `.cso` de FXC) viajan en
    `ga10x/sombreadores/bmox12_{vs,ps}.sm86` (vertice 97 instrucciones,
    pixel 39), fabricados y juzgados por `tests/bmox12_sm86.rs`, que prueba
    con la casa que dan en 0, 30, 60 y 123 la MISMA posicion de recorte
    (bit a bit) que la tanda de V0 y el mismo color en 8 bits.
  - `gpu verrano bmox12 [N]`: la 3060 transforma e ilumina ella; la CPU
    dibuja lo mismo por V0 y se comparan pixel a pixel y con D3D12.
- **HECHO EN EL METAL (28-09 11:37):** `gpu verrano bmox12 0`, `30` y `60`:
  `IGUAL: VERRANO en el aparato = VERRANO en la CPU = D3D12 en la 3060 bajo
  Windows` en los tres (huellas 0xab7afc663a345885, 0x2b3985e93e1a6574,
  0x8dc7ef10f691548e), con los programas de BMOX-12 traducidos por PROTON-X
  (1696 + 768 B). La 3060 transforma e ilumina; la CPU solo juzga.
- **Lo que el metal mostro (y el juez ya sabe):** dos carreras que el juez
  no miraba. (1) El EXIT de un programa de pixel LEE el color (R0..R3): lo
  que lo escribe tiene que haber llegado. (2) Una barrera de escritura
  tarda UN ciclo en encenderse: quien la enciende espera 2 si el siguiente
  la espera (`ptxas` lo hace). Las dos salian como puntos al azar por
  bloques, solo con pocos warps.
- **Queda (no bloquea):** `cubo.bsf` sin SASS a mano; la interpolacion con
  perspectiva (hoy ScreenLinear, exacta en caras de un valor).
  - **08-10, LB6 de [`PLAN_LAS_LIBRERIAS.md`](PLAN_LAS_LIBRERIAS.md):** en el
    anfitrion, ya sin SASS a mano. El cubo de V0 escrito en TITAN++
    (`toolchain/lang/titan/ejemplos/nivel11/cubo`) sale por este emisor, el
    pegamento de E5 y el juez; `titan sm86` deja su `cubo.bsf` con el MAPA de
    la casa de fuente. Da los bits de la tanda, y cumple el contrato que el
    escritorio exige al abrir. El metal, del propietario: `gpu verrano` con
    ese sobre.

## [ ] E6 -- SALTOS, BUCLES, `switch` Y ENTEROS, del SM5 y del DXIL (02-10; el anfitrion, hecho)

Del propietario (02-10): "haz los saltos y bucles del emisor", en orden y
DIRECTO al emisor -- DXIL/SM5 a SASS sin SPIR-V ni el compilador de NVIDIA
por medio --. Hecho en el anfitrion, de la punta del Programa a la del juez:

- **La lengua** (`bmo-sm86`): FSETP, ISETP, SEL, IADD3 y BRA, con las 18
  palabras de oro de `ptxas` 12.9 (`ga10x/sombreadores/oro_saltos.ptx`,
  128 bits con su control) y 21 fabricadas que `nvdisasm` 13.4 lee como
  dice su texto (`LEIDAS_E6`). Lo que se vio en `ptxas`: para `if` y bucles
  pone `FSETP` y `@P0 BRA`, SIN `BSSY`/`BSYNC` (la reconvergencia es
  rendimiento; con `TEX.LZ` no hay derivadas que la pidan); y un predicado
  tarda 13 ciclos en poder ser GUARDA, 4 en ser operando de SEL.
- **El Programa** (`proton-x`): `Compara`, `Elige`, `Copia`, `SumaEntera`,
  `Si`/`SiNo`/`FinSi`, `Bucle`/`RomperSi`/`Romper`/`FinBucle`; un registro
  guarda BITS (float, entero o el booleano de D3D, 0xFFFFFFFF); su forma se
  comprueba (`Programa::forma`). El interprete los corre y es el juez de
  todo lo demas; `nativo` (x86-64) se aparta de lo que salta.
- **El lector SM5**: `if_nz`/`if_z`, `else`, `endif`, `loop`, `endloop`,
  `break`, `breakc_nz`/`breakc_z`, `lt` `ge` `eq` `ne`, `ilt` `ige` `ieq`
  `ine`, `iadd`, `movc`. Un programa que salta no renombra: `r#` y `o#` son
  VARIABLES, y lo que lee de `v#` y `cb0` va al principio.
- **El emisor** (`proton-x-sm86`, `saltos.rs`): las variables (un registro
  de la 3060 todo el programa, puesto a su valor inicial), lo vivo en la
  cabeza de un bucle hasta su fin, nada de constantes guardadas dentro de
  una rama, y la Compara de un solo lector FUNDIDA a P0 (sin SEL).
- **El planificador**: un predicado, 13 ciclos a su guarda y 5 a SEL; un
  BRA DRENA (todo lo escrito llego, todas las barreras esperadas) y deja 5
  ciclos detras. Asi la cuenta en linea recta vale para cualquier camino.
- **El simulador**: P0..P6, el guarda de cada instruccion, las cinco nuevas,
  y un tope de pasos (un bucle que no sale es `SinFin`, no un cuelgue).
- **El juez** (J1): R9 (predicado antes de llegar: 13 guarda, 4 operando --
  todo el oro de `ptxas` lo cumple); R8 (salto sucio: un BRA con algo en
  vuelo) en lo que fabrica BMO-X -- los programas con SPH, los que sube la
  puerta del kernel -- y en `juzgar_drenado`; el computo de `ptxas` (`giro`
  salta con cargas en vuelo y las espera en el destino) se sigue leyendo
  como en v1. R7: un cuerpo de app puede comparar, elegir, sumar enteros y
  SALTAR, pero solo dentro de su cuerpo, y el guarda solo en su BRA.

- **Como se sabe:** los cuatro programas de `dxil::ejemplos` y los dos SM5
  armados palabra a palabra, con los dos ABI, dan los MISMOS bits en el
  simulador que en la casa (un NaN, NaN: su signo y su carga no se modelan),
  y el juez dice PERFECTO con los saltos drenados; y 400 programas al azar
  (si dentro de bucles dentro de si, romper desde un si, variables que
  cruzan ramas, contadores enteros) en el banco -- 4000 una vez: 3977
  emitidos, 23 sin registros, 34 562 saltos, todos iguales y PERFECTOS. La
  prueba al azar encontro un fallo VIEJO de la linea recta: `Min`/`Max` se
  ponian al reves para ahorrar un MOV, y con un cero de cada signo dan el
  primero (`max(-0, +0)` es -0). Ya no se ponen al reves.
- **Queda (el metal):** un sombreador que salta en `gpu verrano`, contra
  D3D12 en la 3060 bajo Windows, como E5. Y [!] un bucle que no sale CUELGA
  la 3060: el juez no puede saber si acaba, y el kernel no tenia un
  vigilante que corte un trabajo que no vuelve. Antes de dejar que una app
  mande cuerpos con bucles al metal, ese vigilante (E7: hecho en el
  anfitrion el 02-10; falta verlo en el metal con `gpu eterno ya`).
- **E6b, el DXIL que salta (02-10, hecho en el anfitrion):** el de `dxc`,
  el de Cyberpunk. El lector sabe `br`, `phi` (sus valores van con signo y
  pueden ser de mas adelante: se resuelven al acabar), `fcmp`/`icmp`
  (CMP2; las desordenadas, como la negacion de una ordenada), `select` y
  `add`/`sub` de enteros. `dxil/estructura.rs` vuelve el grafo `si` y
  bucles: dominadores y post-dominadores (Cooper, Harvey y Kennedy), bucles
  naturales de UNA salida, la union de un `si` dentro de un bucle con los
  post-dominadores de SU grafo (sin las aristas de `break` ni de `continue`:
  si no, el bloque de detras salia repetido), y cada `phi` una copia en su
  arista, en paralelo. Y `Continuar` (el `continue`) en el Programa, el
  interprete, el emisor y el SM5 (`continue`, `continuec`).
  **Como se sabe:** tres sombreadores de `dxc` (`proton-x/prueba/saltos`,
  `anidado`, `mientras`: if/else con phi, bucles rotados, continue y break
  anidados, select, un while) dan en la casa los MISMOS bits que su HLSL
  escrito en Rust, y en el simulador los mismos que en la casa, con los dos
  ABI; el juez, PERFECTO. Encontro un fallo del emisor: `dxc` repite el
  `cbufferLoadLegacy` DENTRO del bucle, y con el ABI de registros (una
  precarga, hecha una vez) su registro se soltaba tras su ultima lectura de
  la PRIMERA vuelta. Ahora lo que lee una Entrada o el cbuffer vive hasta el
  fin del bucle; la prueba al azar lee filas dentro de bucles y lo habria
  visto (comprobado deshaciendo el arreglo).
- **E6c, los enteros y el `switch` (02-10, hecho en el anfitrion):**
  - La lengua (`bmo-sm86`): IMAD, LOP3, SHF, IMNMX, F2I e I2F, con 17
    palabras de oro de `ptxas` (`oro_enteros.ptx`) y 9 leidas por
    `nvdisasm`. `ptxas` pone en la 3060 `I2FP` (acoplada, opcode 0x45) para
    pasar de entero a float; aqui va I2F (desacoplada, con barrera): de
    `I2FP` no se tiene su latencia, y una espera mal puesta es un pixel al
    azar.
  - El Programa: `Entera` (resta, mul, shl, shr logico y aritmetico, and,
    or, xor, min y max con y sin signo; los desplazamientos con `& 31`,
    como D3D), `Convierte` (de float a entero hacia cero y en su limite, un
    NaN 0; de entero a float al mas cercano) y las comparaciones sin signo.
    El simulador las sabe (y el `-` de IADD3 es ya el de los enteros, no el
    bit de signo de un float); el juez, en R7, las deja a una app.
  - El DXIL: `sub mul shl lshr ashr and or xor`, `trunc`/`zext`/`sext` de
    `i1` (un `i1` cierto es 0xFFFFFFFF), `fptosi fptoui sitofp uitofp`,
    `bitcast`, las sobrecargas `.i32` y `dx.op.binary.i32`; y el `switch`,
    que se vuelve una cadena de `si` antes de estructurar.
  - El SM5: `and or xor not ineg imul umul imad umad ishl ishr ushr imin
    imax umin umax itof utof ftoi ftou ult uge`, y `switch`/`case`/
    `default`/`endswitch` como un bucle de UNA vuelta (su `break` es el
    Romper) con un `si` por tramo y una marca de "ya se entro" (asi un caso
    sin `break` cae al siguiente).
  - **Como se sabe:** `enteros.hlsl` de `dxc` y dos SM5 armados palabra a
    palabra (un `switch` con un caso que cae y su `default`; y los de bits)
    dan los mismos bits que su referencia en Rust, y en el simulador los
    de la casa con los dos ABI; el juez, PERFECTO. Cada operacion sola, con
    17 x 17 entradas raras (shifts de 32 o mas, NaN, infinitos), igual. Y
    4000 programas al azar con enteros: iguales. Lo que se vio con el azar: un
    NaN visto como ENTERO cambia el camino segun su signo, que no se modela
    (`a - b` de la CPU y `FADD a, -b` lo dan distinto); un sombreador de
    verdad no lo hace -- sus tipos lo impiden -- y la prueba tampoco.
- **E6d, la division y el resto de enteros (02-10, hecho en el anfitrion):**
  - La 3060 no divide enteros. `ptxas` (`oro_division.ptx`: div y rem, u32
    y s32) saca un inverso de `b` por abajo (I2F.U32.RP, MUFU.RCP, menos 2
    ULP, F2I.FTZ.U32.TRUNC), lo afina con una vuelta de Newton con
    IMAD.HI.U32 (la mitad alta de un producto) y corrige el cociente dos
    veces; con signo, sobre IABS. 9 palabras de oro nuevas.
  - El emisor (`dividir`) hace esa cuenta SIN guardas -- en el cuerpo de una
    app solo un BRA lleva guarda (R7) --: cada `@P0` es un SEL. Y la vuelta
    de Newton sin el par de IMAD.HI (`e + hi(e * t)` con un IADD3). Con el
    acarreo (abajo), de 17 a 20 instrucciones sin signo y de 22 a 26 con
    signo, segun el ABI y lo que se pida; IMAD.HI es una clase nueva
    del planificador, `Ancha` (la de NAK: 6 ciclos entre dos de ellas).
  - Entre 0 da todo unos, cociente y resto (lo de D3D en `udiv`, y la 3060
    pone `~b`); con signo, hacia cero, el resto con el signo de `a`, y
    `i32::MIN / -1` es `i32::MIN` (y su resto 0).
  - El DXIL: `udiv sdiv urem srem`. El SM5: `udiv` con sus dos destinos,
    cualquiera null, y leyendo todo antes de escribir.
  - **Como se sabe:** cada una sola con 23 x 23 entradas raras y con 211 x
    211 pares (los bordes de cada potencia de 2 y al azar), igual que la
    casa en el simulador; `division.hlsl` de `dxc` (con un bucle que divide)
    y un SM5 de `udiv` armado a mano, igual que su HLSL en Rust y emitidos
    con los dos ABI, PERFECTO ante el juez; y el azar con division. El
    simulador hace el MUFU.RCP exacto, la 3060 no: la misma cuenta en Rust
    con el inverso movido de -64 a +1 ULP da el mismo cociente (con +2 ya
    no: ese es el margen de las -2 ULP), en 50000 pares.
  - **Lo que se ahorra (02-10, segunda parte, `division.rs`):**
    `division.hlsl` salia de 200 instrucciones, mas que la puerta de 128
    del kernel. Ahora 113 (123 con el pegamento de pixel; la prueba lo
    PEGA y lo juzga con la puerta):
    - LA PAREJA: `a / b` y `a % b` del mismo par en el mismo tramo recto
      (lo que escribe `dxc` para `x / n` y `x % n`, tambien dentro de un
      bucle) son UNA cuenta con dos salidas. No se funden si algo escribe
      `a` o `b` entre medias, o lee el destino de la segunda, o hay un `si`
      o un bucle por medio.
    - UNA CONSTANTE: ni inverso ni comprobar el 0. Potencia de 2, un
      desplazamiento; si no, la multiplicacion "magica" de Granlund y
      Montgomery (la de 32 bits si existe, si no la de 33 con su suma), lo
      que hace NVVM antes del PTX (`ptxas` no: usa el inverso tambien con
      una constante, se vio en su SASS). Con signo, sobre `|a|`.
    - EL ACARREO: cada correccion era ISETP, IADD3, SEL, IADD3, SEL; ahora
      `IADD3 k, P0, r, -b` (su acarreo es `r >= b`), SEL y `IADD3.X q, q,
      RZ, RZ, P0`. 5 palabras de oro mas (IMAD.HI con inmediato, IADD3 con
      acarreo e IADD3.X); el juez sabe que IADD3 escribe su acarreo y que
      .X lo lee (R9: 4 ciclos, como `ptxas`), y el simulador lo suma como
      la 3060 (`a - 0` acarrea: es `a + ~0 + 1`).
    - **Como se sabe:** cada constante rara (0, 1, -1, potencias de 2,
      `i32::MIN`, las dos magias, negativas) con 84 dividendos, sin un I2F;
      la pareja en los dos ordenes, con algo por medio, con constante y con
      `x = x % b`, UNA cuenta; con `a` escrita en medio o un `si`, dos; la
      magia contra Rust con unos 6000 divisores; y el azar con parejas. Romper la
      magia, el resto o la condicion de la pareja lo detectan las pruebas
      (comprobado).
- **Despues (no es esta casilla):** bucles de varias salidas y `break` de
  dos bucles, `continue` dentro de un `switch`, y el grafo no reducible
  (`dxc` no lo escribe).

## [ ] E7 -- EL VIGILANTE: un trabajo de la 3060 que no vuelve (el TDR de BMO-X)

Anotado el 02-10, del propietario (la tarjeta de tarea que salio de E6):
desde E6 el emisor de PROTON-X pone bucles (`BRA` hacia atras) y R7 deja a
un cuerpo de app saltar dentro de si mismo. El juez NO puede saber si un
bucle acaba. Un sombreador con un bucle que no sale cuelga la 3060 PARA
SIEMPRE: en Windows lo para el TDR (Timeout Detection and Recovery, que
reinicia la GPU); en BMO-X no hay nada igual -- ni en
`Ultra_kernel_x86-64/kernel/src/ring0/dev/gpu_trabajo/` ni en
`platform/drivers/gpu/ga10x/src/trabajos/` --.

- **Que hace falta**, del lado del kernel, para los trabajos que entran por
  la puerta de VERRANO/PROTON-X (`gpu_trabajo/cubo.rs`, donde se juzgan con
  `juzgar_programa` antes de subir):
  1. darse cuenta de que un trabajo mandado no aviso de que acabo
     dentro de un plazo;
  2. parar o reiniciar el canal o el motor SIN romper lo demas -- estudiar
     como se le quita el canal a la 3060 o se le mata por el GSP-RM: el
     punto de partida es `platform/drivers/gpu/ga10x/COMO_LE_HABLA_NVIDIA.md`
     y los controles del GSP-RM (`control.rs`, `Control::TODOS`; nova-core y
     open-gpu-kernel-modules tienen la preempcion de canales y el
     recuperar de un canal en `RC`, "robust channel");
  3. decirlo claro en la cabina (`warn`), como el juez dice un BODRIO.
- **Mientras no exista:** un cuerpo de app con bucles NO va al metal (lo
  dice E6). Los programas de la casa con bucles, solo los que se sabe que
  acaban.
- **Como se sabra:** en el metal, un sombreador hecho a proposito con un
  bucle eterno: la cabina dice el corte, el canal se recupera y el
  siguiente dibujo sale.
- **E7a, el vigilante (02-10, hecho en el anfitrion; falta el metal):**
  - Lo estudiado, de open-gpu-kernel-modules 570.144 (bajado de
    raw.githubusercontent): el RM de la CPU de NVIDIA tiene su vigilante
    (`kernel_rc_watchdog_callback.c`) y, cuando ve la GPU colgada, NO toca
    registros: le manda al GSP-RM `INTERNAL_RC_WATCHDOG_TIMEOUT`
    (0x20800A6A, sin parametros; `krcWatchdogRecovery_KERNEL`), y el GSP
    recupera. En un sistema con GSP, el RM de la CPU es BMO-X: ese vigilante
    es nuestro. Sus banderas (`g_subdevice_nvoc.c`) son 0xC0,
    `ROUTE_TO_PHYSICAL | INTERNAL`: va sobre las asas INTERNAS del RM, las
    de `GET_GSP_STATIC_INFO` con las que G0 ya hablo en el metal. Y para un
    solo canal, `STOP_CHANNEL` (0xA06F0112, `bImmediate`, banderas 0x8: lo
    puede pedir quien tiene el canal): lo saca del motor y de su lista, y si
    no se deja, le hace RC.
  - El corte (`bmo_gpu_ga10x::vigilante`, puro y probado; el kernel en
    `gpu_trabajo/vigilante.rs`): tras CADA espera de un trabajo del GR (17
    sitios: los del kernel, el cubo, el anillo, la pantalla, el video y la
    imagen), si VENCIO -- lanzado, sin pagar y pasado su plazo (1 s; los
    cortos del kernel, 100 ms) -- y el canal no estaba ya muerto: 1 PARAR
    (`STOP_CHANNEL` sobre nuestro canal de GR); si a los 250 ms el GR sigue
    ocupado (`NV_PGRAPH_STATUS` 0x400700, bit 0) y sin RC, 2 ESCALAR
    (`RC_WATCHDOG_TIMEOUT`). Acaba con el RC_TRIGGERED del canal (su Xid),
    con el GR quieto, o "sigue girando: reiniciar". La cabina lo dice paso a
    paso (`E7:`), y el canal de GR queda MUERTO (P3b4c): todo trabajo del GR
    dice NO al instante. La regla vieja (dos dibujos seguidos sin pagar) se
    va: el primero que vence ya se corta, y la 3060 deja de girar.
  - Las respuestas del GSP no se pueden emparejar con su pregunta (salen
    todas con `sequence` 0): por eso el corte mira el MOTOR y la cola de
    avisos, no la respuesta.
  - El contrato deja salir los dos pasos y nada parecido: `PararGr` con su
    `bImmediate` exacto (con 0, NO), y el escalon solo sin parametros y con
    asas.
  - La prueba en el metal: `gpu eterno ya` (`IOMMU_OP_GPU_ETERNO`, 0x4E):
    un computo de un warp cuyo programa EMPIEZA por el `BRA` a si mismo que
    `ptxas` pone detras de cada EXIT (palabra ya corrida en el metal). Dice
    si se corto, en cuanto y como acabo. Deja el GR fuera: la ultima prueba
    de una sesion.
- **Falta:**
  - **El metal**: `gpu eterno ya` y que la cabina diga el corte, y que el
    resto (pantalla, copia, escritorio) siga. Lo que conteste el GSP a cada
    paso (RC con que Xid, o quieto) se aprende ahi.
  - **E7b, levantar el GR otra vez sin reiniciar**: pedir otro canal al RM y
    rehacer su contexto (G1..G4, S1); hoy el canal de GR queda fuera hasta
    reiniciar, como tras un Xid.
  - Con E7a visto en el metal, los cuerpos de app con bucles pueden ir a la
    3060: un bucle eterno ya no la cuelga para siempre (la deja sin GR hasta
    reiniciar, y lo dice).
- Las convenciones: comentarios en ASCII y en castellano sin enes caidas,
  ambitos de `toolchain/tools/ambitos/AMBITOS.txt`, los guardianes de
  `toolchain/tools/*` con `--check`.

## [ ] E8 -- TODA LA LISTA DE LA CASA EN LA 3060 (09-10)

Del propietario, el 09-10: *"hasta el final que tenga todo el ASM de GPU
rtx 3060 12G"*. La casa (el `Programa` de PROTON-X y de TITAN++) sabe
operaciones que el emisor de la 3060 todavia dice que NO -- `NoEmite::Operacion`
en su sitio --, y entonces ese sombreador va por la CPU. E8 las trae, una a
una, con la misma vara de siempre: los BITS de la casa en el simulador, el
juez diciendo que si (R0..R6, y R7 para el cuerpo de una app), y el metal
despues.

- **Lo que decia que NO el 09-10**, y lo que pide cada cosa:

```text
   Mate exactas    redondeos, frac, es_nan..., los bits,   E8a: HECHO (abajo),
                   los medios floats                        con lo que R7 deja
   arrays          LeeIndexado, EscribeIndexado,            E8b: con SEL en
                   ConstantesEn                             cadena (R7 lo deja)
   discard         Descarta (el KILL de la 3060)            R7: un EXIT con
                                                            guarda, o KILL
   olas            vote, shfl (Ola)                         R7, y el simulador
                                                            por warp
   texturas        arrays, cubos, 3D, mips, Load,           TEX/TLD con su asa:
                   GetDimensions, EligeTextura              el kernel la pone
   Mate de series  sin, cos, tan, exp2, log2, los arcos,    sus bits salen de
                   los hiperbolicos (la casa, f64)          f64: DADD/DMUL/DFMA
                                                            no estan en R7
   computo         IdHilo, Barrera, la memoria compartida,  LB8 de
                   UAV, atomicos, el contador               PLAN_LAS_LIBRERIAS
   geometria       EntradaDe, Emite, Corta                  VERRANO no tiene
                                                            esa etapa
```

- **Lo que es del propietario** (R7 es la puerta de las apps, Ring 0): que
  instrucciones nuevas deja R7 a un cuerpo de app (KILL o el EXIT con
  guarda, VOTE, SHFL, las de f64), y por que camino van los bits EXACTOS de
  las de series (DL10 dijo exactos: en f64 en la 3060 como la casa, u otras
  cuentas de f32 que den los mismos bits -- que la casa cambie las suyas es
  tambien una salida --).
- **E8a, las Mate EXACTAS (09-10, hecho en el anfitrion):**
  - `proton-x-sm86/src/mates.rs`: `RedondoPar`, `Suelo`, `Techo`, `Trunca`,
    `Frac`; `EsNan`, `EsInf`, `EsFinito`, `EsNormal`; `CuentaBits`,
    `InvierteBits`, `PrimerBitBajo`, `PrimerBitAlto`,
    `PrimerBitAltoConSigno`; `F16aF32` y `F32aF16`. Las dieciseis con lo que
    R7 YA deja (FADD, FMUL, FSETP, ISETP, SEL, IADD3, IMAD, LOP3, SHF, I2F,
    F2I): el juez no cambia ni una linea.
  - Como: truncar es F2I.TRUNC e I2F con |x| < 2^23, con el SIGNO de x (-0.5
    da -0, como la casa); suelo, truncar menos uno si se paso; techo,
    -suelo(-x) por los bits; al par, (|x| + 2^23) - 2^23 -- la suma redondea
    al par --; frac, la misma resta que la casa. La clase, por FSETP
    (desordenada para el NaN) y la normal por su exponente. Los bits, por
    SWAR (contar, invertir; el primero por arriba, derramando). Los medios
    floats por sus campos, con los bordes de la casa: su NaN (0x7E00), el
    infinito desde 65520 y los subnormales al par. Cada cuenta escribe su
    destino en la ULTIMA instruccion (`x = f(x)` no pisa lo que lee), con
    los predicados P1..P3 (el P0 es de las comparaciones fundidas).
  - Como se sabe (`pruebas_mates.rs`): cada una, en los dos ABI, juzgada
    (R0..R6 y R7), sobre los bordes de cada exponente, 20 000 al azar y, la
    de medio a f32, los 65 536 medios (tambien con basura arriba): los
    MISMOS bits que `Mate::aplicar` -- exactos los enteros y los si/no, y en
    los f32 un NaN por otro --. Saboteada (sin el signo de truncar), cae en
    el primer -0. Y `x = f(x)` tres vueltas en un bucle da lo de la casa.
  - Lo que cuestan, con su entrada y su EXIT (`lo_que_cuesta_cada_mate`,
    el ABI de registros): la clase de un numero, 3 instrucciones (la
    normal, 6); truncar y al par, 7; suelo, 10; frac, 11; techo, 12; contar
    bits, 13; el primer bit por abajo, 18; de medio a f32, 23; invertir y el
    primero por arriba, 26; con signo, y de f32 a medio, 29. De 5 a 11
    registros. Caben de sobra en un hueco de la tuberia (128).
  - Las de SERIES siguen diciendo que NO en su sitio, y van por la CPU.
- **Como se sabra E8 entera:** cada fila de la tabla, con su prueba en el
  anfitrion; las que piden R7 o el kernel, cuando el propietario las abra; y
  en el metal, un sombreador de cada una.

---

# 4. LO QUE ESTE PLAN NO ES

- **No es un compilador de CUDA ni de PTX.** La entrada es SPIR-V y nada mas.
- **No optimiza** hasta que lo correcto este en el metal (E5). Un emisor
  rapido que cuelga la 3060 es peor que uno lento que dibuja.
- **No es para toda NVIDIA.** SM86 y su tabla. Otra generacion es otra tabla
  (y otra lista del corpus), no otro compilador.
- **No compila en el Ryzen todavia.** Todo en el anfitrion; el juez si corre
  en el kernel (J2) porque es `no_std` y chico.
