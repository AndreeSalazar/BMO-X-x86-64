# PLAN LA ESCALERA DE PROTON-X -- de HelloWindow a Cyberpunk, una capa por escalon

> Abierto el **2026-10-05**. El propietario, tras doce corridas de Cyberpunk
> 2077 en el metal: *"PROTON-X refleja perfecto, pero me fui directo al final
> boss sin darme cuenta. Como solucionamos eso? Eso es MAS FUERTE, no?"*. Y
> despues: *"si, escribelo como plan"*.
>
> Lo de antes, que este plan no repite: las doce corridas y sus muros en
> [`PLAN_LAS_TRES_GRANDES.md`](PLAN_LAS_TRES_GRANDES.md) seccion 6; el ABI
> de la casa y que leer de los que ya lo hicieron en
> [`D3D12_MAESTRO.md`](../maestro/D3D12_MAESTRO.md); la regla de solo
> x86-64 en [`PLAN_PROTON_X.md`](PLAN_PROTON_X.md) seccion 1.

---

# 0. LO QUE PASO, DICHO CLARO

**La base es buena, y esta medida.** Las DLL de la casa corren en el banco
del anfitrion con `.exe` de Windows de verdad (`proton-x-casa/tests/corre.rs`):
`hola`, `teb`, `ventana`, `hilos`, `ficheros`, `crt`, `ucrt`, `stdio`, `seh`,
`usadll`, las tandas 1 a 48 y `vueltas.exe`. El ABI de D3D12 tiene 465 huecos
en 28 interfaces y ninguno sale por `falta` (`proton-x-casa/tests/abi.rs`). El
cubo de D3D12 salio IGUAL, bit a bit, en Windows y en la 3060 (X4 y X5 de
[`PLAN_LA_LUDOTECA.md`](PLAN_LA_LUDOTECA.md)).

**Lo que falto fue el medio.** La escalera de juegos de `D3D12_MAESTRO.md`
(seccion 5, hasta el 05-10) decia:

```text
   1  los DirectX-Graphics-Samples de Microsoft
   2  Cyberpunk 2077
```

y del escalon 1 solo existia una parte. Medido el 05-10 en el arbol:

```text
   HelloTriangle    no hay .exe de Microsoft; el cubo es `cubo12.exe`, nuestro
   HelloTexture     su CAMINO llamado a mano desde Rust (tests/textura.rs);
                    el .exe de Microsoft nunca corrio en PROTON-X
   el resto         nada
```

Asi que de "la base" se paso a Cyberpunk sin escalones, y cada tanda nacio de
un muro de Cyberpunk, a la contra. Un solo puntero nulo (`ffxDispatch`) costo
diez corridas en el metal para encontrarlo, porque el codigo es cerrado y el
unico camino era la forense: la autopsia, el lector del nulo, los bytes del
`.exe` sacados de su copia en Windows.

**No fue tiempo perdido.** Cyberpunk llego, sin Wine, a su ventana, a su
sonido y a 1065 PSO: eso dejo 48 tandas en el banco y el mapa de lo que
falta. Lo que cambia desde hoy es el ORDEN.

# 1. POR QUE LA ESCALERA ES MAS FUERTE

Cyberpunk pide CINCO capas a la vez, y cuando cae no dice de cual es:

```text
   1  Win32           hilos, ficheros, el registro, COM, la red
   2  D3D12           lo que cada hueco HACE (la semantica, MAESTRO 3)
   3  DXIL            las operaciones de sus sombreadores (MAESTRO 4)
   4  DLL de otros    FSR, Streamline, Bink, PhysX, Galaxy, Aftermath
   5  rendimiento     un nucleo; los sombreadores en la CPU
```

Un escalon pide UNA capa nueva sobre lo que ya paso:

```text
                         CYBERPUNK                  UN ESCALON
   capas nuevas          cinco a la vez             una
   el codigo             cerrado: forense           abierto: se lee la fuente
   la referencia         ninguna ("llego mas        su HUELLA en Windows (R3)
                         lejos" no es un juez)
   donde se prueba       en el metal, una corrida   en el banco del anfitrion,
                         por viaje                  en segundos
   si luego se rompe     nadie se entera hasta      el banco se pone rojo
                         la proxima corrida
```

Y las casillas que le faltan a Cyberpunk son LAS MISMAS: el bindless, el
computo, ExecuteIndirect, el HDR... cada una tiene su muestra de Microsoft
que la prueba SOLA (seccion 4). No es trabajo de mas: es el mismo trabajo,
cada pieza aislada y con su imagen correcta delante.

# 2. LAS REGLAS

```text
   R1  UN ESCALON, UNA CAPA. Si un escalon pide dos casillas abiertas de
       LAS_TRES_GRANDES, se parte en dos, o se sube primero el que pide una
   R2  EL .EXE DE VERDAD. Compilado del fuente publico SIN cambios. Lo que
       solo acepta MSVC va en un puente (`prueba/muestras/antes.h`) que no
       cambia lo que hace la muestra. Nada de "su camino llamado a mano":
       eso es lo que ya habia
   R3  EL JUEZ. La huella de cada Present (FNV-1a de B, G, R, como
       `bmo_cubo::referencia::huella`) la toma el banco SIN tocar el .exe
       (`VISTAS` de `tests/corre.rs`, 05-10). La esperada sale de las reglas
       de D3D12 cuando las fijan (E1.1: un Clear), o de la misma muestra en
       Windows, con la 3060 y su driver. Las dos tienen que ser LA MISMA:
         bit a bit   donde D3D12 lo fija: copias, enteros, sin filtrado
         su margen   donde la especificacion lo deja (filtrado, mezcla en
                     float, las trascendentales): el de la especificacion,
                     escrito en la casilla con su numero. Nunca "parecido"
   R4  PRIMERO EL BANCO, LUEGO EL METAL. Cada escalon entra en
       `tests/corre.rs` (como las tandas) y despues se ve en el Ryzen. Un
       escalon cerrado no se puede caer sin que el banco se ponga rojo
   R5  CYBERPUNK ES LA VARA DE MEDIR. Una corrida al cerrar un escalon (para
       ver hasta donde llega ya), no una por muro. Un muro nuevo se apunta en
       LAS_TRES_GRANDES y se busca su escalon: si cae en uno de aqui, se
       arregla AHI, con su referencia; si no cae en ninguno, va a una tanda
       (como siempre) o a un escalon nuevo de este plan
   R6  SOLO x86-64. PE32+ AMD64, como dice PROTON-X (su seccion 1). Todos
       los escalones de aqui lo son; uno que no, no sube
   R7  DE DONDE SALE CADA BINARIO (cambiada el 05-10, con E1.0 hecho). En
       la NUBE, con `prueba/muestras/construir.sh`: mingw-w64 sobre el UCRT
       (el mismo CRT que Cyberpunk: api-ms-win-crt-*), los commits de
       origen fijados, reproducible (dos directorios, el mismo sha256) y sin
       rutas de la maquina (`procedencia --check`). Sin Windows ni Visual
       Studio para fabricarlo; Windows solo hace falta para MIRARLO. El sha256
       en `prueba/HACER.txt` (PX1), la licencia en `muestras/` y en el
       NOTICE de la raiz
   R8  EL PISO DE LA 3060. Cada escalon tiene dos mitades: la de la CPU (el
       banco, la huella del interprete) y la de la 3060 (la huella del
       metal). La de la 3060 pide que el GSP despierte SIEMPRE: G0 de
       PLAN_LA_3060. Mientras la 3060 despierte cuando quiera, una huella
       mala no dice si fallo el escalon o el arranque. La mitad de la CPU NO
       lo pide: E1 y E2 avanzan aunque la 3060 este terca
```

La unica excepcion a R5 es la que ya esta en marcha: la corrida 13, que
dice si el arreglo de `ffxDispatch` (`d16a29d`) funciono (E0.4).

# 3. LA ESCALERA

## E0 -- lo que ya hay

- [x] **E0.1 -- la base Win32 en el banco.** `hola` a `seh`, `usadll` con
  `saludo.dll`, las tandas 1 a 48 y `vueltas.exe` corren en el anfitrion con
  las DLL de la casa (`proton-x-casa/tests/corre.rs`).
- [x] **E0.2 -- el ABI de D3D12 contado.** 465 huecos, 0 faltan
  (`proton-x-casa/tests/abi.rs`, que escribe la seccion 2 del MAESTRO).
- [x] **E0.3 -- el cubo.** `cubo12.exe` (nuestro, por el camino de BMOX-12):
  su huella es la de la 3060 en los fotogramas 0, 30 y 60 (X4, X5, P3c4).
- [ ] **E0.4 -- cerrar el nulo de `ffxDispatch`.** La corrida 13 con
  `d16a29d`. **Como se sabe:** en `informe/diario.txt` la linea `# PROTON-X:
  en vivo: amd_fidelityfx_dx12.dll CARGADA en ...`, y el salto a 0 desde
  `Cyberpunk2077.exe+0x1d4c6cf` no vuelve. Es la ultima corrida de Cyberpunk
  "por muro"; las siguientes son de la vara (R5).

## E1 -- los Hello de Microsoft (`Samples/Desktop/D3D12HelloWorld`, MIT)

- [x] **E1.0 -- la cadena de fabricacion** (05-10). `prueba/muestras/
  construir.sh` trae DirectX-Graphics-Samples, DirectX-Headers y
  DirectXMath en sus commits y compila la muestra en la nube (R7);
  `antes.h` es el puente de MSVC a mingw-w64: `_uuidof`, el `FileHandle` de
  la WRL, y los metodos COM que devuelven una estructura (la llamada de
  maquina es la misma que la de MSVC). Lo que se aprendio al hacerlo: con
  msvcrt.dll el `.exe` pedia 23 funciones que la casa no tiene, casi todas
  del arranque viejo de mingw; sobre el UCRT, 12, y todas de Windows de
  verdad. Por eso el UCRT. Lo que queda para los siguientes: los
  sombreadores, con `sombras.exe` en Windows (el `D3DCompile` de la casa no
  compila HLSL, sirve el `.cso` de su huella: P3c2,
  `proton-x-casa/src/compilador.rs`). **Como se sabe:** `hwindow.exe` en el
  arbol, su sha256 en `prueba/HACER.txt`, `proton_x.py` y
  `procedencia --check` en verde.
- [x] **E1.1 -- HelloWindow** (05-10: en el banco, y el MISMO dia en el
  metal y en Windows, dicho por el propietario: *"ya cumplen las 2
  peticiones: lo que en Windows funciona, funciona normal en BMO-X"*).
  Solo la cadena de intercambio, un Clear, una valla y Present. La capa:
  la cadena de un `.exe` de Microsoft (D5.7 de LAS_TRES_GRANDES, a
  medias). Pidio 12 funciones que la casa no tenia, y
  ya las tiene: `GetThreadId`, `CommandLineToArgvW`, `mbrtowc`, `wcrtomb`,
  `_ismbblead`, `__p__environ`, `__p__wenviron`, `__p__acmdln`,
  `__p__fmode`, `signal` (guarda, no llama, y lo dice), `__daylight` y
  `rand_s`. **Como se sabe:** `tests/corre/muestras.rs`: 60 Present en
  1280x720 y en cada uno CADA pixel R 0x00 G 0x33 B 0x66, lo que fija D3D12
  para {0.0, 0.2, 0.4} en R8G8B8A8_UNORM; y las doce, una a una, con los
  casos que muerden. En el metal: `run sys/proton-x.bex window/hwindow.exe`,
  una ventana azul -- visto.
Lo que dijeron E1.2 a E1.6 al hacerlas (05-10): las muestras de hoy ya NO
compilan en marcha. Su proyecto de Visual Studio compila con DXC (SM 6.0)
al construir, y el `.exe` lee `shaders_VSMain.cso` y `shaders_PSMain.cso` de
su carpeta. DXC es de Microsoft y de codigo abierto: se compila en la nube
en el commit fijado (`muestras/construir.sh`, con `DXC=`), y sin Windows.
Cada muestra vive en su carpeta del volumen, `window/<muestra>/` (todas
llaman igual a sus `.cso`): `ejemplos.ps1` la copia asi, y el guardian PX2
lo sabe desde hoy. Las cinco pedian a la casa UNA sola funcion, la misma:
`CreateFile2` (con ella leen sus `.cso`).
**Lo que dijo el METAL (05-10, el propietario con `dynindex.exe`):** el
`.exe` arranca y aborta (`terminate called after throwing an instance of
'std::exception'`, sale con 3): no encuentra `shader_mesh_simple_vert.cso`.
No es la casa: el FAT32 de BMO-X busca por el nombre CORTO (8.3) y se salta
las entradas de nombre largo (`fat32/src/buscar.rs`), y los `.cso` de las
muestras tienen nombres largos -- tambien los de E1.2 a E1.6
(`shaders_VSMain.cso`). Dos salidas: la carpeta de la muestra en `D:`
(NTFS, solo lectura, que si lee nombres largos), o que el FAT32 los lea
(Ring 0: se decide con el propietario). Los `.exe` nuestros no lo sufren:
`computo.exe` lleva su sombreador dentro.

- [x] **E1.2 -- HelloTriangle, el `.exe` de verdad** (05-10, en el banco;
  falta verlo en el metal). Lo que el cubo no pide: `d3dx12.h`, ComPtr, el
  CRT de C++, y sus `.cso` de DXC. **Como se sabe:** `tests/corre/
  muestras.rs`: 30 Present iguales; fuera del triangulo (a mas de 1 pixel
  del borde) EXACTAMENTE el azul; dentro, rojo, verde y azul interpolados
  con las baricentricas, cada canal a 2 o menos de lo exacto (el margen de
  R3: la interpolacion y el paso a 8 bits); y 51200 pixeles dentro, +-1 %.
- [x] **E1.3 -- HelloTexture, el `.exe` de verdad** (05-10, en el banco;
  falta verlo en el metal). El tablero de 256x256 que el `.exe` hace en la
  CPU, subido con UpdateSubresources, y un muestreador de PUNTO. **Como se
  sabe:** bit a bit: cada pixel de dentro es EXACTAMENTE el negro o el
  blanco del cuadro de su UV (salvo a menos de 0.05 texeles de una raya).
  Un aviso, y es de velocidad: un PSO que muestrea va por el interprete.
- [x] **E1.4 -- HelloConstBuffers** (05-10, en el banco; falta verlo en el
  metal). Un cbuffer en un monton UPLOAD con `Map` PERSISTENTE que el `.exe`
  escribe antes de cada fotograma (D2.3). **Como se sabe:** cada fotograma
  distinto, y en el 0, el 15 y el 29 el triangulo de E1.2 corrido
  (n + 1) * 0.005, con el mismo juez.
- [x] **E1.5 -- HelloFrameBuffering** (05-10, en el banco; falta verlo en el
  metal). Dos fotogramas en vuelo, un allocator y una valla por fotograma
  (D5.2). **Como se sabe:** cada Present, bit a bit, el de E1.2.
- [x] **E1.6 -- HelloBundles** (05-10, en el banco; falta verlo en el
  metal). `ExecuteBundle`, que la casa se SALTABA (la mitad de N5.17): ya
  corre, con las reglas de herencia de D3D12 (render targets, viewport y
  tijera de la lista que lo llama; lo que el bundle deja puesto, de vuelta a
  ella). **Como se sabe:** cada Present, bit a bit, el de E1.2.

## E2 -- una casilla abierta, una muestra (`Samples/Desktop/`, MIT)

Medido el 05-10 en el commit fijado: las siete compilan sus sombreadores con
DXC al construir, como E1, asi que se fabrican en la nube igual. Lo nuevo
que piden al compilarlas: `pix3.h` (los marcadores de PIX; sin `USE_PIX` son
macros vacias, y un `pix3.h` minimo en el puente basta), y DynamicIndexing y
Multithreading, sus datos (`occcity.bin`, texturas) de la carpeta de la
muestra.

- [ ] **E2.1 -- D3D12Multithreading.** Listas de ordenes grabadas desde
  varios hilos (H2.7), con sus mapas de sombras (N5.12, hecho). **Como se
  sabe:** su huella, igual, con los hilos cooperativos de hoy; y otra vez
  cuando H1 lleve los hilos a varios nucleos.
- [x] **E2.2 -- D3D12DynamicIndexing** (05-10, en el banco; falta verlo en
  el metal). El indice dinamico de descriptores (N5.4, el bindless): una
  ciudad de 15 x 8 copias, y cada una lee SU material de 120 con el registro
  calculado de una constante de la raiz. Antes de hoy decia lo mismo que
  Cyberpunk, "createHandle con un registro CALCULADO", y no dibujaba nada;
  ahora la casa lo hace (ver N5.4). Pidio ademas `_wassert`. **Como se
  sabe:** `tests/corre/muestras.rs`: el tono medio de cada franja sube de
  las ciudades de cerca (rojo, material 0) a las del fondo (violeta, 119),
  con los seis tramos del arcoiris; probado que dice NO con el indice
  atascado. La imagen, a ojo, es la de la captura de Microsoft. Bit a bit
  no se puede (muestreo LINEAL): falta la de Windows a 1280x720 para
  compararla con su margen.
- [x] **E2.3a -- el COMPUTO, con un juez nuestro** (05-10, en el banco;
  falta verlo en el metal y en Windows). Antes de nBodyGravity, su capa
  sola (R1): `prueba/computo.exe` (`computo.cpp`, de consola) corre un CS
  de 64 hilos que se pasan sus datos por la memoria COMPARTIDA con una
  BARRERA en medio, dos veces: en la cola directa (constantes en la raiz) y
  en una cola de COMPUTO que la espera con una VALLA (un CBV en la raiz). La
  casa ya lo corre: N5.5 en el crate (el interprete se para en cada barrera
  y sigue grupo a grupo) y en la casa (`SetComputeRoot*`, `Dispatch`
  apuntado y corrido al ejecutar, los UAV de bufer de las tablas). **Como
  se sabe:** el `.exe` compara los 2 x 1024 floats con su cuenta, BIT A BIT
  (todo exacto en float: la 3060 tiene que dar lo mismo), y en Windows dice
  lo mismo; probado que dice NO sin la barrera.
- [x] **E2.3b -- D3D12nBodyGravity** (05-10, en el banco; falta verlo en
  el metal y en Windows). Confirmado con su fuente: dibuja las 10.000
  particulas con un GEOMETRY SHADER (un punto, un cuadro de cuatro
  vertices) y calcula en UN hilo suyo con su cola de computo y vallas entre
  colas. Lo que pidio, y ya tiene la casa: el GS (en el crate: `EntradaDe`,
  `Emite`, `Corta`, su PSV0, el enlace VS -> GS -> PS y las primitivas; en
  la casa, el PSO con GS y los puntos), `D3D12_OPTIONS12` contestado
  (`EnhancedBarriersSupported` NO: va por ResourceBarrier), `rand`/`srand`
  de MSVC, los `getelementptr` CONSTANTES de su CS (384 lecturas de la
  compartida), y que una espera cumplida CEDA el turno: con la cola
  sincrona su hilo de computo nunca esperaba de verdad y el de dibujo no
  corria. Y su CS, TRADUCIDO a x86-64 (`nativo_computo`): interpretado, un
  paso de la simulacion eran 73 s; traducido, uno o dos. **Como se sabe:**
  `tests/corre/muestras.rs`: tres Present distintos, las dos nubes iguales a
  izquierda y derecha y centradas, rojas en el 0 y amarillas en el 2 (ya
  aceleran); y `tests/nativo_computo.rs`: su CS traducido da los bits del
  interprete y, sobre 2048 particulas como las suyas, la FISICA en f64 con
  su margen (R3: floats sumados en otro orden; lo peor, un 3 % del margen),
  probado que dice NO con un paso una milesima distinto. Los bits de
  Windows, no (otro orden de suma). La captura de Microsoft es de mucho
  despues (las dos nubes ya fundidas en una galaxia): el estilo es el
  mismo (puntos rojos con su halo, amarillos donde aceleran, el fondo azul
  oscuro); el momento, no.
- [x] **E2.4 -- D3D12ExecuteIndirect** (05-10, en el banco; falta verlo en
  el metal). 1024 triangulos, cada uno su orden INDIRECTA (la direccion de
  su CBV y su Draw), y un CS que las CULLEA con `Append` en un UAV con
  CONTADOR. Lo que pidio: el contador de un UAV (la op `Contador`, en el
  interprete y traducido; su numero viaja en la ranura del descriptor, asi
  que sobrevive a las copias de descriptores), `ExecuteIndirect` (la otra
  mitad de N5.17: se apunta y se resuelve al EJECUTAR la lista, porque sus
  argumentos y su cuenta los escribe el computo de antes) y su firma.
  Nada mas: ni un aviso. **Como se sabe:** `tests/corre/muestras.rs`: dos
  corridas de 60 Present, con culling y con el ESPACIO pulsado (sin el):
  dentro de la franja del culling, los MISMOS pixeles bit a bit; fuera,
  con culling, solo el fondo; probado que dice NO con el contador atascado.
  Y su CS traducido da las mismas ordenes y el mismo contador que el
  interprete (`tests/nativo_computo.rs`).
- [x] **E2.5 -- las OLAS de verdad, con un juez nuestro** (`prueba/olas.exe`;
  05-10, en el banco; falta verlo en el metal y en Windows). Antes eran
  "un pixel por ola": cada hilo SOLO, y `WaveGetLaneCount` daba 1 mientras
  OPTIONS1 contestaba 32. Ahora una ola son 32 carriles
  (`dxil::olas::CARRILES`, el mismo numero que contesta OPTIONS1): en el
  computo, los 32 hilos SEGUIDOS de SV_GroupIndex (como un warp de la
  3060); en los pixeles, cuadros de 2x2 de un triangulo, de 8 en 8, con
  los pixeles de fuera de AYUDANTES (`src/cuadros.rs`); en vertices y GS,
  un carril activo de 32. El interprete PARA cada carril en su operacion de
  ola y `dxil/carriles.rs` la resuelve con los que llegaron por el mismo
  camino (los ACTIVOS: el que va antes, como lo correria una GPU, con la
  vuelta de cada bucle; los ayudantes no cuentan). Todas las de SM 6.0:
  Active Sum/Product/Min/Max/BitAnd/BitOr/BitXor/CountBits/Ballot/AllEqual/
  AnyTrue/AllTrue, ReadLaneAt/First, IsFirstLane, Prefix Sum/Product/
  CountBits, y QuadReadAcrossX/Y/Diagonal y QuadReadLaneAt. Los traductores
  nativos (`nativo.rs`, `nativo_computo.rs`) y el de la 3060
  (`proton-x-sm86`) NO las traducen: ese sombreador va por el interprete
  (lo dice el aviso del PSO; la puerta de la 3060, con su nombre). Con
  stencil, UAV y `[earlydepthstencil]` (N5.12b, N5.3d) cada pixel de una
  ola pasa por las MISMAS pruebas de antes y de despues que uno solo
  (`trama::poner_pixel`); los ayudantes no escriben stencil, Z ni UAV, ni
  cuentan. La muestra de Microsoft, D3D12SM6WaveIntrinsics,
  no es el juez: pinta segun como junte la GPU los pixeles (no hay huella
  que comparar) y pide D3D11On12 y Direct2D para su texto. **Como se
  sabe:** `tests/corre/muestras.rs` (e2_5): `olas.exe` dice `bien` 15
  veces: cada operacion de ola de un CS sobre 128 hilos, bit a bit, contra
  la cuenta a mano (dentro de un si, en el bucle de ESCALARIZAR de los
  juegos y en uno del que cada hilo sale en otra vuelta); en 64 x 64 cada
  pixel lee a sus vecinos de cuadro; un pixel solo lee a sus tres
  ayudantes y su ola tiene UN activo. Probado que dice NO: la casa de antes
  sale con 14 MAL; contando a los ayudantes, MAL; juntando los carriles sin
  mirar el bucle, MAL en los dos bucles. Y `src/pruebas_olas.rs`: lo mismo
  sin la casa, y que en cuadros entra a cada pixel lo MISMO que pixel a
  pixel. **Lo que puede fallar, dicho:** que la 3060 no junte los hilos de
  un CS de 32 en 32 seguidos (es lo que hace; D3D no lo promete: el juez lo
  mira primero, `WaveGetLaneIndex`) o que no reconverja en los bucles como
  aqui (la ola de un `continue` se junta al final de la vuelta); las
  DERIVADAS siguen a 0 (D4.4), tambien con cuadros; ni las de 16 o 64 bits
  ni las de SM 6.5 (`WaveMatch`, `WaveMultiPrefix*`): no compilan, dicho.
  Un pixel que no paso el stencil y corre solo para saber si lo tira va en
  su ola de AYUDANTE (D3D ni lo correria).
  **Queda:** las olas en la 3060 (`vote`, `shfl`) y en el x86 traducido;
  las derivadas con los cuadros; verlo en el metal y en Windows.
- [ ] **E2.6 -- D3D12HDR.** Render targets de float (N5.16, hecho el 05-10 con
  `prueba/hdr.exe`) y la cadena en 10 o 16 bits con su espacio de color (la
  cadena ya se acepta y se presenta en 8 bits; falta la muestra de
  Microsoft y SetColorSpace1/SetHDRMetaData). **Como se sabe:** su huella, igual, en
  el modo de 8 bits y en el de 16.
- [x] **E2.7 -- D3D12PredicationQueries** (05-10, en el banco; falta verlo
  en el metal). Un cuadro blanco lejos, uno translucido cerca que pasa por
  delante, y la caja del lejano en una consulta de oclusion BINARIA cuyo
  resultado decide, con `SetPredication`, si el lejano se dibuja en el
  fotograma siguiente. Lo que pidio: que la trama cuente los pixeles que
  PASAN la profundidad (`Cuenta::pasan`, escriban color o no),
  BeginQuery/EndQuery de verdad (se cuentan al ejecutar la lista; una
  consulta no cruza listas) y `SetPredication` (su u64 se lee al ejecutarse,
  como dice Microsoft; salta Draw, Dispatch, ExecuteIndirect, copias y
  limpiezas, y nada mas): `proton-x-casa/src/consultas.rs`. Con una
  consulta abierta el lote va por la CPU: la 3060 aun no cuenta. Nada mas:
  ni un aviso. **Como se sabe:** `tests/corre/muestras.rs`: 35 Present, y
  en cada uno se mide donde esta el cuadro cercano; el lejano sale si y
  solo si el cercano NO lo tapaba en el anterior, con los colores exactos;
  probado que dice NO sin predicacion y con la consulta siempre VISIBLE. Y
  la cuenta exacta en la trama (`src/pruebas_pixeles.rs`).

## E3 -- el jefe intermedio: MiniEngine (`MiniEngine/ModelViewer`, MIT)

- [ ] **E3.1 -- ModelViewer con Sponza.** Cientos de PSO, sombras, SSAO,
  bloom, tonemapping y TAA, casi todo por computo: un Cyberpunk en miniatura
  CON EL CODIGO A LA VISTA. Por comprobar antes: de donde salen los datos de
  Sponza que pide y su licencia. **Como se sabe:** su huella con la camara
  quieta y el TAA apagado; con el TAA, su margen (R3).
- [ ] **E3.2 -- FRAPS-X sobre ModelViewer.** El primer numero de fps de una
  escena de verdad (el nivel 9 de la escalera de LAS_TRES_GRANDES), donde un
  fotograma lento se puede seguir hasta su linea de codigo.

## E4 -- un motor ABIERTO, un juego de verdad

- [ ] **E4.0 -- elegir UNO, medido.** `rayosx` sobre los candidatos, en
  Windows, y la tabla aqui (64 bits; D3D12 con o sin Agility SDK; que DLL de
  otros trae; que pide que la casa no tiene):

  ```text
     Godot 4          MIT. Su driver D3D12 (`--rendering-driver d3d12`) hace
                      sus sombreadores DXIL EN MARCHA: el camino "compilar al
                      cargar" de DXVK, al reves. Una escena exportada propia
     Wicked Engine    MIT. D3D12 de serie, con su editor y sus demos
     RBDOOM-3-BFG     GPL. D3D12 por NVRHI; pide los datos de DOOM 3 BFG, que
                      hay que tener comprados
  ```

  **Como se sabe:** la tabla escrita, y el elegido con su motivo.
- [ ] **E4.1 -- el elegido, en una escena repetible.** Paso de tiempo fijo
  (Godot: `--fixed-fps`) o una demo grabada (RBDOOM: `timedemo`), para que el
  fotograma N sea siempre el mismo. **Como se sabe:** su huella o su margen
  (R3), en el banco y en el metal.

## E5 -- un juego CERRADO mas chico que Cyberpunk

- [ ] **E5.0 -- la lista del propietario.** Sus juegos D3D12, de 64 bits y
  sin DRM (GOG primero, la regla de la LUDOTECA), medidos con `rayosx`. El
  candidato que ya esta en el MAESTRO: The Witcher 3 4.0 en DX12 (GOG, el
  mismo estudio). **Como se sabe:** la lista aqui, con lo que pide cada uno.
- [ ] **E5.1 -- el elegido, hasta su menu.** El primer juego cerrado que
  llega a su menu sin forense: lo que pida ya tiene escalon debajo.

## FINAL -- Cyberpunk 2077

Sigue donde esta: su escalera (niveles 1 a 9) y sus muros en
[`PLAN_LAS_TRES_GRANDES.md`](PLAN_LAS_TRES_GRANDES.md) seccion 6. Desde hoy
se mide con R5: una corrida por escalon cerrado.

# 4. QUE CASILLA DE LAS_TRES_GRANDES PRUEBA CADA ESCALON

```text
   casilla de LAS_TRES_GRANDES             su escalon
   D5.7   la cadena de intercambio          E1.1
   D2.3   Map persistente (UPLOAD)          E1.4
   D5.2   vallas                            E1.5, E2.3a y E2.3b (entre colas)
   N5.17  ExecuteBundle / ExecuteIndirect   E1.6, E2.4 (hechos)
   H2.7   listas desde varios hilos         E2.1
   N5.4   el indice dinamico (bindless)     E2.2
   N5.5   el COMPUTO                        E2.3a y E2.3b (hechos)
   N5.3c  los UAV                           E2.3a y E2.3b (los de bufer)
   N5.3d  los UAV de un DIBUJO              uavpixel.exe (hecho, sin escalon)
   D5.1   la cola de computo                E2.3a y E2.3b (hechos)
   (nueva) el sombreador de geometria       E2.3b (hecho)
   D4.3   las olas de verdad                E2.5 (hecho)
   N5.16  render targets de float           E2.6
   D5.5   consultas                         E2.7 (oclusion y predicacion)
```

Una casilla de LAS_TRES_GRANDES se marca cuando pasa su escalon, con "falta
verlo en Cyberpunk" hasta que la vara (R5) lo mida. Asi el plan del juego
sigue diciendo la verdad sobre el juego, y este, sobre la pieza.

# 5. Y DESPUES, EXPRIMIR: DONDE ENTRAN INTI Y VERRANO

El propietario (05-10): *"OPTIMIZACION monstruosa para que mi Cyberpunk
pueda ser exprimido con mi CPU y mi GPU, gracias a INTI y VERRANO"*. Si, y
en este orden, que es la ley 0 de
[`OPTIMIZACION_MAESTRO.md`](../maestro/OPTIMIZACION_MAESTRO.md) (correcto,
medido, rapido):

```text
   1  CORRECTO  la escalera: cada escalon con su huella. Optimizar algo que
                todavia no dibuja bien es acelerar un fallo
   2  LOS DOS SALTOS GRANDES, que no son micro-optimizacion:
                N6  los sombreadores del juego a la 3060 (hoy los interpreta
                    la CPU, pixel a pixel)
                H1  el Ring 3 en los 6 nucleos (hoy `smp all` solo lleva
                    faenas del kernel: el juego entero corre en UN nucleo)
   3  MEDIDO    FRAPS-X sobre E3.2 (ModelViewer): el fotograma partido en sus
                trozos, con el codigo abierto para seguir el lento hasta su
                linea. Sobre Cyberpunk solo despues: es cerrado
   4  RAPIDO    INTI (la CPU, el samurai: la instruccion exacta, AVX2) y
                VERRANO con TITAN++ encima (la 3060: el BSF ya traducido, la
                GPU no compila nada) van DONDE el numero de 3 lo diga
```

**Lo primero que ya se exprimio (05-10, E2.3b): el COMPUTO traducido a
x86-64** (`bmo_proton_x::nativo_computo`). Lo de `nativo.rs` (los dibujos
sin saltos) con lo que el computo pide: saltos de verdad, enteros, la
memoria compartida, los buferes y la BARRERA (guarda donde va y vuelve; al
llamarla otra vez, salta detras). 50 veces el interprete en el banco: un
paso de nBodyGravity, de 73 s a uno o dos. Su juez es el interprete, bit a
bit, y la fisica en f64. No es que la CPU dibuje: es calcular lo que el
juego pide mientras la 3060 no corre computo (N6).

- [x] **X1 -- la VELOCIDAD de los DIBUJOS: los que saltan, traducidos**
  (`platform/shared/proton-x/src/nativo.rs`, 05-10, en el banco; falta
  verlo en el metal). Un VS o un PS con `si`, bucles, enteros,
  comparaciones, conversiones, `discard` o arrays de registros se
  INTERPRETABA (el aviso "un PSO cuyo sombreador salta o hace cuentas
  ENTERAS"): asi irian casi todos los de Cyberpunk. Ahora `nativo::compilar`
  los traduce con el cuerpo del COMPUTO (`nativo_computo`, el mismo codigo
  y el mismo juez) y una entrada de nueve instrucciones que habla la
  llamada de los dibujos y le pone un `Contexto` a medias en la pila; la
  casa los llama igual, y el PS dice al volver si se tiro (`DESCARTADO`).
  Y en los DOS caminos, el MXCSR solo se carga si su control es otro: un
  `ldmxcsr` que lo cambia costaba en el Xeon del banco unos 50 ns por
  llamada (el VSId, de 33 a 87), y en BMO-X quien llama es soft-float y
  su control es siempre el de D3D. **Como se sabe:**
  `proton-x-casa/tests/nativo/saltos.rs`, contra el interprete BIT A BIT
  (las salidas y si el pixel queda): los VS y PS de `instancias.exe` y
  `hdr.exe` (20.000 casos al azar cada uno, con el cbuffer, y los ids de 0 a
  63); los PS de `dxc` con saltos (anidado, enteros con `switch`, division
  por un cbuffer con n = 0, -1 e i32::MIN, `clip`/`discard`, arrays) y
  `mientras`; los de `ejemplos` y seis de `fxc`; y cada operacion entera,
  comparacion y conversion sobre 52 x 52 valores raros (NaN, -0, lo que no
  cabe en un i32 o un u32, desplazamientos de 32 o mas). Probado que dice
  NO: con un `shl` de 64 bits en vez de 32, 324 distintos. Y el MXCSR por
  los dos caminos con cuatro de quien llama (hacia cero, DAZ+FTZ, el de
  D3D con banderas). En el banco de `.exe`: `instancias.exe`, ni un aviso;
  `hdr.exe`, solo el de texturas (su PSLee). **Medido** (100.000
  llamadas, la mejor de cinco rondas, el Xeon del banco COMPARTIDO: los
  numeros bailan): sin optimizar, VSCuadro 5 veces el interprete, VSId 6-7,
  VSInst 3.5, division (un bucle) 17-18, anidado (dos bucles) 45; en
  `--release` (alli el interprete es Rust CON SSE; en el metal es
  soft-float, asi que esto es lo de menos) 1.6, 1.5, 1.3, 2.6 y 7. Los
  `.exe` de 64 x 64 no lo notan (0,13-0,19 s, ruido).
  **Lo que puede fallar, dicho:** (1) un bucle que no acaba cuelga el
  traducido como cuelga el interprete (no hay tope en ninguno); (2) las
  banderas de excepcion del MXCSR de quien llama se quedan con las del
  sombreador cuando su control es el de D3D (un juego podria leerlas con
  `_statusfp`; sus propias cuentas ya las ponen a cada rato); (3) la entrada pone SOLO cuatro campos del
  `Contexto`: lo que lea otros (ids, vistas, compartida, barreras) no se
  traduce como dibujo (`con_saltos` lo mira; y su prueba,
  `pruebas_saltos.rs`). **Queda:** los que MUESTREAN (DynamicIndexing, el
  PSLee de `hdr.exe`, casi todos los PS de un juego: el aviso de texturas
  sigue), la matematica (`Mate`: exp, log, sin), los cbuffers con fila
  calculada (`ConstantesEn`: luces, huesos; falta pasarle al codigo la
  medida del cbuffer) y verlo en el Ryzen.

**LA CPU GUIA, LA 3060 DIBUJA.** El propietario (05-10): *"que la CPU no
tiene que ser la que dibuje, sino que tenga el mapa por via de RAM y que le
guie a la GPU constantemente"*. Es como trabaja todo driver de verdad, y en
la casa ya tiene sus piezas:

```text
   hoy           la casa EJECUTA en la CPU las listas de ordenes del juego:
                 el interprete pinta cada pixel. La CPU dibuja
   el modelo     la CPU traduce cada lista a un EMPUJE de la 3060 y lo deja en
                 RAM del PC prestada a la 3060 (por la IOMMU); lo apunta en el
                 ANILLO (el GPFIFO de `canal::GR`, que ya existe) y avanza su
                 puntero. La 3060 lo trae por DMA y dibuja, sin parar mientras
                 haya anillo. La CPU solo escribe el MAPA: que dibujar, con que
                 datos (matrices, descriptores, vertices) y en que orden
   las piezas    N6  la traduccion: lo que hoy va al interprete, al SASS de
                     SM86 (PLAN_LAS_TRES_GRANDES)
                 C3  los empujes en RAM prestada, no por PRAMIN
                 C4  dos tandas en vuelo: la CPU prepara la N+1 mientras la
                     3060 hace la N
                 A3  esperar a la 3060 por INTERRUPCION, sin girar
                     (C3, C4 y A3 en PLAN_LA_3060_AFINADA)
   como se sabe  B1 de AFINADA: el fotograma cuesta el MAYOR de CPU y 3060,
                 no su suma, y la CPU queda libre para el juego
```

# 6. LO QUE ESTE PLAN NO CAMBIA

- Cyberpunk sigue siendo el NORTE de PROTON-X: este plan es el camino, no
  otro destino.
- Ni Wine, ni vkd3d-proton, ni su codigo dentro de la casa. Las muestras de
  Microsoft entran como `.exe` de PRUEBA (MIT, con su aviso), igual que las
  tandas: la casa no lleva ni una linea suya.
- Las tandas siguen: un muro que no cae en ningun escalon se prueba con una
  tanda, como hasta hoy.

# 7. LO QUE TE PIDO, Y LO QUE PUEDE FALLAR (05-10)

El propietario: *"anota lo que me pidas luego"* y *"anotar que potencial
fallo es, para saber y asi evitar sorpresa"*. Aqui, para que no se pierda
en una conversacion.

## 7.1 Lo que te pido (lo decides tu)

- [ ] **Nombres LARGOS en `platform/drivers/storage/fat32/src/buscar.rs`**
  (Ring 0, en CODEOWNERS: tu permiso). Los `.cso` de las muestras se
  llaman `shaders_VSMain.cso`: no caben en 8.3 y en el metal no se
  encuentran. Mientras, se corren desde `D:` (NTFS).
- [ ] **`opt-level = 3` para `bmo-proton-x` en el `/Cargo.toml` raiz** (en
  CODEOWNERS: tu permiso). El interprete en debug es lento en el banco (un
  paso de nBodyGravity eran 73 s antes del computo traducido).
- [ ] **`SquidRoom.bin` (43 MB) en `platform/shared/proton-x/prueba/muestras`**
  para E2.1 (Multithreading): sin el, ese escalon no se sube. Iria como
  `occcity.bin` (E2.2), con su `.gitignore` de `*.bin` abierto.
- [ ] **Juntar en `main` lo de `docs/plan/PLAN_LAS_TRES_GRANDES.md`** de
  este trabajo: E2.3, E2.4, E2.7, N5.13 a N5.15, N5.3b y N5.3c (un PR).
- [ ] **Correr en el Ryzen y en Windows lo de `platform/shared/proton-x/prueba`**
  (HACER.txt; la hoja entera, prueba a prueba y con lo que tiene que
  salir, en [`docs/metal/PRUEBAS_DX12_EN_EL_RYZEN.md`](../metal/PRUEBAS_DX12_EN_EL_RYZEN.md)): `computo.exe` dice `bien` 4 veces, `instancias.exe` 3 y
  `vistas.exe` 7, `uavpixel.exe` 4, y salen con 0; `nbody.exe` (las dos nubes que se
  juntan), `indirect.exe` (lo mismo dentro de la franja con el ESPACIO y
  sin el) y `predica.exe` (el cuadro blanco se va cuando el rojo lo tapa
  entero).
- [ ] **El CARTEL ROJO** (`Ultra_userspace/apps/proton-x/src/cartel.rs`):
  hoy se queda puesto para siempre desde el primer aviso. Si molesta en un
  juego, se puede quitar a los diez segundos del ultimo aviso nuevo: tu
  dices.

## 7.2 Lo que puede fallar (y por que no seria una sorpresa)

```text
   NADA DE ESTO SE HA VISTO EN EL METAL   todo lo de 05-10 paso en el banco
                                          del anfitrion; en el Ryzen corre
                                          con el codigo traducido y la 3060,
                                          que el banco no usa
   los .cso de nombre largo               no se encuentran en FAT32 (7.1)
   la VELOCIDAD                           el computo traducido va 50 veces
                                          el interprete, y desde X1 (05-10)
                                          los de DIBUJO con saltos o cuentas
                                          enteras tambien van traducidos (3
                                          a 45 veces en el banco); siguen
                                          interpretados los que muestrean,
                                          la matematica y los cbuffers con
                                          fila calculada (lo dice un aviso);
                                          Cyberpunk, lejos de sus fotogramas
   el HDR (N5.16 y N5.16b, 05-10)         los de 1 a 4 canales y sus UAV ya
                                          son float; al presentar lo de mas
                                          de 1 se recorta (sin monitor HDR);
                                          un R32_UINT de destino, aun no
                                          (aviso)
   el stencil (05-10, N5.12b)             ya recorta, con las reglas de D3D12
                                          (`stencil.exe`); sus lotes van por
                                          la CPU (la 3060 no lo sabe: lo
                                          dice), y el plano 1 no se lee con
                                          CopyTextureRegion todavia
   AlphaToCoverage                        se apunta y no se usa (lo dice un
                                          aviso al crear el PSO): con
                                          SampleDesc.Count 1 no cubre nada,
                                          no hay muestras que tapar; cuenta
                                          cuando haya MSAA
   un UAV en un sombreador de DIBUJO      desde el 05-10 se escribe (N5.3d,
                                          uavpixel.exe), por el interprete y
                                          por la CPU (la 3060 no lo lleva: lo
                                          dice una vez). El orden entre
                                          pixeles es el de la trama: D3D no
                                          da ninguno, y un juego que dependa
                                          de el podria ver otra cosa. En un
                                          sombreador de GEOMETRIA, todavia se
                                          pierde (lo dice un aviso); un
                                          dibujo SOLO con UAV (sin render
                                          target) no se dibuja (lo dice)
   un UAV de textura 3D, de array o de    el PSO de computo no se crea (lo
   cubo                                   dice)
   ClearUnorderedAccessView con           limpia la vista entera (lo dice)
   rectangulos
   DepthClipEnable = FALSE                N5.16b (05-10): sin recorte en z,
                                          la Z sujeta, en la CPU; la 3060
                                          recorta siempre: esos lotes, por la
                                          CPU (lo dice la puerta)
   un SRV estructurado en la RAIZ de un   el paso sale de `dx.resources`; un
   sombreador sin metadatos (SM5, DXBC)   SM5 no los trae como DXIL: se lee
                                          crudo, mal, y NO lo dice todavia
   las OLAS (E2.5)                        por el interprete siempre, y un
                                          sombreador de pixeles con olas va en
                                          cuadros (mas lento); las derivadas,
                                          aun 0 (lo dice `dxil/olas.rs`, no
                                          un aviso)
   la consulta de oclusion                sus lotes van por la CPU (la 3060
                                          aun no cuenta): mas lento, no
                                          distinto
   las instancias y los datos por         por la CPU tambien
   instancia
   el cartel rojo                         no se ve cuando la 3060 pinta
                                          DIRECTO en la pantalla (Z1); un
                                          .exe de GDI que lea su ventana lo
                                          leeria
   un .exe hecho con Visual Studio        llama a mas cosas de las que usa
                                          (el runtime, la telemetria, el
                                          COM): cada una que falte es un
                                          aviso y sale en el cartel. Ninguna
                                          se contesta con un exito mentido
```

La regla de siempre, y la del propietario: que PROTON-X hable HONESTO.
Lo que no sabe hacer lo dice (`aviso`), una vez por cosa, en la consola y en
el cartel rojo; lo que dice que hizo, lo hizo.
