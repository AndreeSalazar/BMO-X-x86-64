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
- **Lo que el metal enseno (y el juez ya sabe):** dos carreras que el juez
  no miraba. (1) El EXIT de un programa de pixel LEE el color (R0..R3): lo
  que lo escribe tiene que haber llegado. (2) Una barrera de escritura
  tarda UN ciclo en encenderse: quien la enciende espera 2 si el siguiente
  la espera (`ptxas` lo hace). Las dos salian como puntos al azar por
  bloques, solo con pocos warps.
- **Queda (no bloquea):** `cubo.bsf` sin SASS a mano; la interpolacion con
  perspectiva (hoy ScreenLinear, exacta en caras de un valor).

---

# 4. LO QUE ESTE PLAN NO ES

- **No es un compilador de CUDA ni de PTX.** La entrada es SPIR-V y nada mas.
- **No optimiza** hasta que lo correcto este en el metal (E5). Un emisor
  rapido que cuelga la 3060 es peor que uno lento que dibuja.
- **No es para toda NVIDIA.** SM86 y su tabla. Otra generacion es otra tabla
  (y otra lista del corpus), no otro compilador.
- **No compila en el Ryzen todavia.** Todo en el anfitrion; el juez si corre
  en el kernel (J2) porque es `no_std` y chico.
