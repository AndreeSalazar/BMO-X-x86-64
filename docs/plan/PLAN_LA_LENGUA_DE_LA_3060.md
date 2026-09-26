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

## [ ] J0 -- EL CORPUS DE ORO

Los programas que ya corrieron en el metal, sacados a una lista con su nombre
y su origen: `raster` (T1c), `color3d` (T2a), `cubo` (X5), `giro`, `blur`,
`fractal`, `lienzo`, `triangulo`, `escena`, `pantalla`. Y el de vertice de
VERRANO V0 aparte, marcado **sospechoso**.

- **Bloquea:** nada.
- **Como se sabe:** una prueba recorre la lista y cada programa se decodifica
  entero (ninguna instruccion desconocida para el decodificador de J1).

## [ ] J1 -- EL JUEZ DEL SASS: las reglas, cada una con su programa roto

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

## [ ] J2 -- EL JUEZ EN LAS DOS PUERTAS

`bsf_sm86` (la prueba que fabrica `cubo.bsf`) llama al juez y no escribe el
sobre si dice NO. El kernel llama al mismo juez en `CUBO_VERRANO` antes de
subir los programas, y un NO es un motivo nuevo en la ABI (con su guardian
`la-3060` contando motivos en los tres sitios).

- **Bloquea:** J1.
- **Como se sabe:** un `cubo.bsf` con un bit de control cambiado no se
  fabrica; el mismo programa enviado a mano al kernel vuelve con el motivo y
  la 3060 no llega a verlo.

## [ ] E1 -- EL SUBCONJUNTO DE SPIR-V PARA LA 3060

El de vertice y el de pixel del cubo (`cubo.vert`, `cubo.frag`), y el de
computo que ya entiende el lector (`GLCompute`): entradas por ubicacion,
salidas a atributos, buffers por su ranura del ABI `SM86_V1`, aritmetica
entera y de coma flotante sin FMA fusionada salvo que el SPIR-V la pida. Lo
que no entra se rechaza CON MOTIVO, como en S2.

- **Bloquea:** nada (el lector y el juez de SPIR-V ya existen).
- **Como se sabe:** el juez de SPIR-V acepta `cubo.vert.spv` y
  `cubo.frag.spv` con la etapa correcta, y rechaza con su nombre lo que falta.

## [ ] E2 -- EL CODIFICADOR, bit a bit contra NVIDIA

`sass/codifica.rs`: cada instruccion que el emisor use, con una prueba que
compara la palabra de 128 bits con la que da `ptxas` y lee `nvdisasm` -- la
misma disciplina que `codifican_lo_que_ya_corrio` de hoy.

- **Bloquea:** J1 (comparte el decodificador).
- **Como se sabe:** cada codificacion tiene su palabra de oro; `nvdisasm`
  lee de vuelta el texto esperado.

## [ ] E3 -- EL EMISOR: SPIR-V a SASS, en linea recta

Traducir el modulo del lector a instrucciones de SM86 con registros
asignados, sin optimizar: primero correcto. Los bits de control, aun
CONSERVADORES (esperar siempre lo maximo).

- **Bloquea:** E1, E2.
- **Como se sabe:** el oraculo (S3) ejecuta el SPIR-V y un simulador de SASS
  en el anfitrion ejecuta lo emitido: mismos resultados en los vertices del
  cubo; y el juez dice PERFECTO Y PRECISO.

## [ ] E4 -- LOS BITS DE CONTROL POR REGLA

Las esperas y las 6 barreras calculadas con la tabla de Ampere (J1 las
juzga con la MISMA tabla), como `calc_instr_deps.rs`. Aqui deja de haber bits
de control escritos a mano en todo BMO-X.

- **Bloquea:** E3.
- **Como se sabe:** el juez sigue diciendo PERFECTO Y PRECISO, y el programa
  es mas corto en ciclos que el conservador de E3 (medido en el simulador).

## [ ] E5 -- EN EL METAL: el cubo con programas EMITIDOS

`cubo.bsf` deja de llevar SASS a mano: lo fabrica el emisor. `gpu verrano`
dibuja el cubo con ese sobre.

- **Bloquea:** E4, J2 -- y que VERRANO V0 deje de colgarse, o que el juez
  diga por que se colgaba.
- **Como se sabe:** `IGUAL: VERRANO en la 3060 = VERRANO en la CPU = D3D12`
  en el 0, el 30 y el 60.

---

# 4. LO QUE ESTE PLAN NO ES

- **No es un compilador de CUDA ni de PTX.** La entrada es SPIR-V y nada mas.
- **No optimiza** hasta que lo correcto este en el metal (E5). Un emisor
  rapido que cuelga la 3060 es peor que uno lento que dibuja.
- **No es para toda NVIDIA.** SM86 y su tabla. Otra generacion es otra tabla
  (y otra lista del corpus), no otro compilador.
- **No compila en el Ryzen todavia.** Todo en el anfitrion; el juez si corre
  en el kernel (J2) porque es `no_std` y chico.
