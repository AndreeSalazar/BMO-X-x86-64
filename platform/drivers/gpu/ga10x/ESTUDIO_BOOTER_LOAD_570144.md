# Booter Load 570.144: estudio estatico del fichero intacto

17-10-2026. Alcance: leer el booter original para interoperabilidad. No se
modifica firmware ni la secuencia del kernel; no se ejecuta ucode en la GPU.

## 1. Identidad y regiones verificadas

**[CITADO]** HEAD estudiado `738ab655f` contiene
`9c420bc58`. Se contrastaron `arranque/booter.rs`, `falcon.rs`,
`boot_probe.rs`, `gpu_despertar.rs` y la bitacora de arranques.

**[MEDIDO en el fichero]** Original leido de
`BMO-externo/firmware/nvidia/ga102/gsp/booter_load-570.144.bin`, 61304 bytes
(0xEF78). SHA-256 ANTES y DESPUES, identico:
`4497e3eff7e95c774b8a569d17b27c08c9650158d10b229d2be81cdcad9a085b`.

**[MEDIDO en el fichero]** BinHdr: magic=0x10DE, version=1,
medida declarada=0xF000, cabecera=0x18, ucode=0x378, bytes ucode=0xEC00.
La medida efectiva es 0xEF78; no se presupone que el campo declarado sea
una validacion criptografica ni que su diferencia demuestre corrupcion.

**[MEDIDO en el fichero]** HsHeaderV2 (nueve u32):
`3C 300 33C 340 344 C 350 354 24`. Son offsets/medidas, no instrucciones.
En 0x344: fuse_ver=1, engine_id_mask=1, ucode_id=3. Numero de firmas=2;
punto de insercion=ucode+0x8A10. Cabecera de carga:
`0 100 8A00 6200 1 100 8900 100 0`.

| Region | Rango del fichero, fin exclusivo | Bytes | Entropia Shannon, bits/byte | Bytes cero |
| --- | --- | ---: | ---: | ---: |
| [MEDIDO en el fichero] NS | 0x378..0x478 | 256 | 0 | 256 |
| [MEDIDO en el fichero] HS | 0x478..0x8D78 | 35072 | 7,993195 | 115 |
| [MEDIDO en el fichero] Datos | 0x8D78..0xEF78 | 25088 | 0 | 25088 |

**[MEDIDO en el fichero]** El comienzo HS es
`cd820c12377699283e4227a2d8776d69`. Sus 2192 bloques alineados de 16 bytes
contienen 2153 valores distintos; la mayor repeticion de un bloque es cuatro.

**[INFERIDO]** El tramo HS es fuertemente compatible con codigo cifrado y
no se acepta como instrucciones Falcon en claro. La entropia no distingue
por si sola cifrado de empaquetado, compresion u otra codificacion.
**[NO LO SE]** No hay aqui verificacion criptografica que identifique
algoritmo, clave ni descifrado; tampoco se identifico en la cabecera un
campo que certifique por si solo "esta imagen esta cifrada".

**[MEDIDO en el fichero]** No hay un preludio NS en claro en 0x378..0x478:
esa region contiene exclusivamente ceros. No se aplica automaticamente la
secuencia de entrada HS de otro booter, Tegra o GA100.

## 2. Desensamblador ejecutado, controles y base

**[CITADO]** Envytools, revision fijada
`f102b82381f3f11cee113d16374c87091db039d9`:
[`envydis/falcon.c::falcon_prep`](https://github.com/envytools/envytools/blob/f102b82381f3f11cee113d16374c87091db039d9/envydis/falcon.c)
ofrece variantes fuc5 y fuc6 y la feature crypt. Su
[`docs/envydis/index.rst`](https://github.com/envytools/envytools/blob/f102b82381f3f11cee113d16374c87091db039d9/docs/envydis/index.rst)
situa fuc6 desde GP102 para motores seleccionados.
**[NO LO SE]** Esa lista no certifica la revision exacta de ISA del SEC2
GA106. El nombre de cabecera `dev_falcon_v4.h` tampoco es esa prueba.

**[MEDIDO en el fichero]** En Windows se compilaron 27 fuentes C y 17
cabeceras del proyecto, contrastadas byte por byte contra el tar de esa
revision. El adaptador local `driver.c` solo da entrada/salida a la API
`envydis`; el define `strdup=_strdup` adapta el runtime de Windows. No se
cambiaron las tablas de opcodes. Compilador: clang 23.1.2 con SDK/MSVC local.

**[MEDIDO en el fichero]** Dos controles sinteticos de cuatro
instrucciones (`mov`, `mov`, `iowr`, `exit`) se decodifican sin desconocidos
en ambas variantes, con y sin crypt. Son fixtures de ISA y NUNCA firmware
para cargar; sus operandos I/O no acreditan registros de GA106.

| Entrada | Variante | Lineas de instrucciones | Lineas con campos desconocidos | Instrucciones desconocidas | iowr/iowrs aparentes |
| --- | --- | ---: | ---: | ---: | ---: |
| [MEDIDO en el fichero] HS bruto | fuc5 | 11596 | 4556 (39,289 %) | 4138 | 92 |
| [MEDIDO en el fichero] HS bruto | fuc6 | 11596 | 4556 (39,289 %) | 4138 | 92 |
| [MEDIDO en el fichero] Aleatorio, semilla 570144 | fuc6 | 11597 | 4521 (38,984 %) | 4102 | 86 |

**[MEDIDO en el fichero]** Habilitar crypt no cambia los contadores del
HS; en el aleatorio reduce de 4521 a 4520 las lineas con campos desconocidos.
fuc5 y fuc6 no producen ficheros enteramente identicos, aunque sus contadores
coincidan. Una salida contiene tambien una instruccion final incompleta.

**[INFERIDO]** Un desensamblado lineal con un aspecto estadistico tan
parecido al control aleatorio no permite reconstruir funciones ni flujo.
Contar 92 iowr aparentes no demuestra 92 escrituras ejecutables del booter.
**[NO LO SE]** Cambiar solo fuc5 por fuc6 no permite recuperar una rama
validada para 0x15 en esta imagen.

**[CITADO]** `booter.rs` distingue origen IMEM=ucode+0x100,
destino fisico=0 y etiqueta/direccion de arranque=0x100. Por eso la base
de presentacion para el tramo HS extraido es **0x100**, no 0x478 ni 0x378.
La direccion virtual de ejecucion no debe confundirse con destino fisico.

## 3. Escrituras del error y espacio I/O

**[MEDIDO en el fichero]** Hay 143 bytes de valor 0x15 en HS; ninguna
secuencia LE u32 `15 00 00 00`. El desensamblado bruto imprime dos
`mov Rx 0x15`, en posiciones presentadas 0x10DD y 0x750B.
**[INFERIDO]** Son coincidencias no validadas, NO sitios del error ni
direcciones IMEM acreditadas de una rutina. El codigo tambien podria
calcular un error o recibirlo de otra funcion sin un inmediato 0x15.

**[CITADO]** OpenRM
[`kernel_gsp_booter_tu102.c::s_executeBooterUcode_TU102`](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/570.144/src/nvidia/src/kernel/gpu/gsp/arch/turing/kernel_gsp_booter_tu102.c)
acepta buzon cero y transforma un buzon no cero en `NV_ERR_GENERIC`.
No ofrece la tabla privada de errores ni las direcciones de sus ramas.

**[CITADO]** El offset de host del buzon es +0x40. No se identifica sin mas
con la representacion `I[]` de una ISA: la
[documentacion I/O de envytools](https://envytools.readthedocs.io/en/latest/hw/falcon/io.html)
muestra tanto el modelo desplazado (SCRATCH0 I[0x1000]) como excepciones
posteriores sin desplazamiento. Nouveau
[`pmu/fuc/macros.fuc::nv_iowr`](https://github.com/torvalds/linux/blob/91645a52ebf2e22f0bd37c142b79238617d38758/drivers/gpu/drm/nouveau/nvkm/subdev/pmu/fuc/macros.fuc)
tambien distingue ambos modelos.
**[NO LO SE]** No se ha acreditado con esta imagen el modelo exacto del
SEC2 GA106 ni una escritura a ese buzon. Buscar solo el literal 0x40 no
identifica el destino efectivo, que tambien puede venir de un registro.

## 4. Tabla solicitada: lo que realmente se puede afirmar

| Codigo | Direccion IMEM | Condicion interna | Registro leido por la rama | Valor esperado | Estado |
| --- | --- | --- | --- | --- | --- |
| 0x15 | No localizada | No reconstruida | No identificado | No identificado | [NO LO SE] |
| Otros errores privados | No localizados | No reconstruidas | No identificados | No identificados | [NO LO SE] |
| 0 | No localizada | El wrapper del HOST lo acepta como exito; no es una condicion interna identificada | MAILBOX0 leido por el host | 0 | [CITADO] wrapper OpenRM, no desensamblado |

**[NO LO SE]** No se puede dar una lista causal de lecturas WPR2, GSP, FB,
MMU, PTIMER, relojes o fusibles para la rama 0x15 a partir de esta salida.
Los registros publicos del driver sirven para fotografiar estados; no se
convierten por ello en dependencias demostradas de esa rama.

## 5. Datos, versiones y nombres

**[MEDIDO en el fichero]** 0x8D78..0xEF78 no contiene tablas de errores,
nombres ni cadenas: todos sus bytes son cero. Las firmas y parametros estan
fuera, en el contenedor. En HS hay cinco rachas ASCII de al menos ocho bytes,
sin nombres o versiones identificados; no son evidencia de texto util.

**[CITADO]** `booter.rs::firmar` inserta una firma del
contenedor en la COPIA ucode del cargador: DMEM+0x10, fichero+0x8D88 en
este formato. Ese es el protocolo existente de carga, no una alteracion
realizada por este estudio. El original siguio intacto.

## 6. Reproducibilidad y salida completa

**[MEDIDO en el fichero]** Todo lo generado y las fuentes ajenas estan
en `target/gsp-investigacion/570.144/booter-estudio/` (ignorado):
`mediciones.json`, `analizar.py`, `driver.c`, `build.cmd`, `build.rsp`,
`source-sha256.json`, las copias `ns.bin`, `hs.bin`, `data.bin`, controles y
las salidas `hs.fuc5.txt`, `hs.fuc6.txt`, con variantes `+crypt`.

**[CITADO]** Se puede repetir en este Windows, desde la raiz:

```powershell
$py = 'C:/Users/Salazar/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe'
& $py target/gsp-investigacion/570.144/booter-estudio/analizar.py --firmware ../BMO-externo/firmware/nvidia/ga102/gsp/booter_load-570.144.bin --salida target/gsp-investigacion/570.144/booter-estudio --decoder target/gsp-investigacion/570.144/booter-estudio/envydis-falcon.exe
```

**[CITADO]** Para usar la CLI original en una VM
Ubuntu/Debian, estas son las dependencias y opciones de entrada binaria,
base y variante documentadas. **[NO LO SE]** No se ejecuto esta instalacion
de VM en este PC; el resultado medido procede del adaptador nativo anterior.

```bash
sudo apt-get update
sudo apt-get install -y git build-essential cmake pkg-config flex bison libxml2-dev python3
git clone https://github.com/envytools/envytools.git
git -C envytools checkout f102b82381f3f11cee113d16374c87091db039d9
cmake -S envytools -B envytools-build -DCMAKE_BUILD_TYPE=Release
cmake --build envytools-build --target envydis -j2
sha256sum booter_load-570.144.bin
dd if=booter_load-570.144.bin of=hs.bin bs=1 skip=$((0x478)) count=$((0x8900)) status=none
dd if=booter_load-570.144.bin of=data.bin bs=1 skip=$((0x8D78)) count=$((0x6200)) status=none
envytools-build/envydis/envydis -m falcon -V fuc6 -i -n -b 0x100 hs.bin > hs.fuc6.txt
envytools-build/envydis/envydis -m falcon -V fuc5 -i -n -b 0x100 hs.bin > hs.fuc5.txt
envytools-build/envydis/envydis -m falcon -V fuc6 -F crypt -i -n -b 0x100 hs.bin > hs.fuc6-crypt.txt
strings -a -t x -n 8 data.bin
sha256sum booter_load-570.144.bin
```

**[CITADO]** `-i` selecciona entrada binaria; `-b` es la base de
presentacion y `-n` quita colores. Fuente:
[`envydis/envydis.c::main`](https://github.com/envytools/envytools/blob/f102b82381f3f11cee113d16374c87091db039d9/envydis/envydis.c).
**[INFERIDO]** Repetir esos comandos sobre el HS bruto no lo descifra.

**[NO LO SE]** Para cerrar la rama del error falta codigo en claro
verificado de esta misma build, un mapa/enum del fabricante, o evidencia
equivalente que vincule instruccion, condicion y buzon. No se asume que una
imagen debug tenga el mismo codigo.
