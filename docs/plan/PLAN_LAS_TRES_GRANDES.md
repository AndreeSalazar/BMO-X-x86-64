# PLAN LAS TRES GRANDES -- D3D12 de juego, el sonido del juego y varios nucleos

> Abierto el 2026-09-30 por el propietario: *"preparar TODAS LISTAS LARGAS
> porque ese mismo analizar por completo que ES para poder dividir y dominar"*.
>
> Las funciones sueltas del censo de Cyberpunk van por tandas
> ([`PLAN_LA_LUDOTECA.md`](PLAN_LA_LUDOTECA.md)): cada una, un `.exe` de
> prueba que Windows juzga. Lo que queda DESPUES no son funciones: son tres
> piezas grandes. Este plan las parte en casillas chicas, cada una con **como
> se sabe** que esta hecha. Ninguna casilla es "hacer D3D12": todas caben en
> un commit.

---

# 0. LAS REGLAS DE ESTE PLAN

- **Medir antes de hacer.** Cada pieza empieza por una casilla de MEDIDA:
  que pide el juego de verdad. Lo que el juego no pide no se hace (el censo
  maduro ya separa DURAS, RETRASADAS, EN VIVO y NO APORTAN).
- **Manda Windows.** Donde se pueda, la prueba es un `.exe` que dice
  `bien`/`MAL` en el banco y en el Windows del propietario. Con la 3060 de
  verdad en los dos lados, las capacidades de D3D12 se comparan UNA A UNA.
- **Ring 0 pide permiso.** Toda casilla marcada `[RING 0]` se propone antes
  de tocarla.
- **Nada de Wine, DXVK, vkd3d ni ReactOS.** Nada de saltarse DRM (Cyberpunk
  de GOG no lo lleva). D: es de solo lectura.
- **Lo que no aporta, fuera.** Trazado de rayos, DLSS (`nvngx`), FSR de AMD
  como DLL, Reflex y la superposicion de GOG van al final, detras de "se ve
  el menu": el juego arranca sin ellos si se apagan en su configuracion.

---

# 1. EL PASO CERO: EL PRIMER CONTACTO (manda el orden de todo lo demas)

Sin el, las tres listas de abajo son opiniones. Con el, el juego dice que
pide y en que orden.

- [x] P0.1 -- `personal censo` en el metal con el censo maduro: cuantas DURAS
  faltan. **HECHO el 30-09** (el `CENSO.TXT` del metal): 1601 funciones de
  Windows distintas; **DURAS 1120, la casa tiene 950, FALTAN 170**;
  retrasadas 5 (faltan 5); en vivo 476 (faltan 325; 225 de ellas son de
  `mfc140u.dll`); no aportan 0. Dos DLL del juego (`libxess.dll`,
  `libxess_fg.dll`) no se pudieron mirar por sus retrasadas: arreglado el
  30-09 (una ventana con todas las secciones de datos, y el resto cuenta
  aunque las retrasadas fallen); su numero sale en el censo siguiente.

  Las 170 DURAS, por grupos (cada grupo, una tanda):

  ```text
      79  MSVCP140        iostreams y locale (con DATOS importados: cerr y
                          los `id` de las facetas; el diario NO debe envolver
                          un dato con un trampolin)
      18  WLDAP32         por ordinal (LDAP, de libcurl): contestar "no hay"
      15  ole32 + OLEAUT32  COM lo justo (CoInitializeEx, CoCreateInstance,
                          CoTaskMem*, PropVariantClear; SysAllocString...)
      23  HID + SETUPAPI + CFGMGR32   enumerar aparatos: "no hay ninguno"
       4  VCRUNTIME140 + crt-private  RTTI: __RTDynamicCast, __RTtypeid,
                          __unDName(Ex)
       1  VCRUNTIME140_1  __CxxFrameHandler4 (pide tanda4m.exe)
      30  los chicos      SHELL32 4, ntdll 4, WINMM 4, seguridad 4, VERSION
                          3, SHLWAPI 2, IPHLPAPI 2, MSWSOCK 2, XInput 1,
                          GDI32 1, POWRPROF 1, WININET 1, Normaliz 1
  ```
- [ ] P0.2 -- Las DURAS que falten, a CERO (una tanda por grupo de arriba).
  **Como se sabe:** el censo dice `NO FALTA NINGUNA DURA`.
  - [x] P0.2a -- Las 30 chicas (tanda 14a, 30-09): `dll_chicas.rs`;
    `tanda14.exe` dice `bien` 27 veces en el banco. (Y las 5 RETRASADAS:
    el ETW y UuidCreate. Y, de las 15 de COM, las cinco faciles:
    CoTaskMem* y CoCreateGuid, StringFromGUID2.)
  - [x] P0.2b -- VERSION (GetFileVersionInfo* y VerQueryValueA, leyendo el
    recurso de version del fichero) y la seguridad (GetFileSecurityW,
    ImpersonateSelf, AccessCheck, RevertToSelf): tanda 14b, HECHA el 30-09
    (`version_y_seguridad.rs`; `tanda14b.exe`, 12 bien en el banco).
  - [x] P0.2c -- COM lo justo (ole32 y OLEAUT32 por ordinal): tanda 15,
    HECHA el 30-09 (`com_basico.rs`; `tanda15.exe`, 11 bien en el banco).
    Sin clases de COM: CoCreateInstance dice REGDB_E_CLASSNOTREG y el CLSID
    por la consola.
  - [x] P0.2d -- Aparatos que no hay (HID, SETUPAPI, CFGMGR32) y WLDAP32 por
    ordinal: tanda 16, HECHA el 30-09 (`aparatos.rs`; `tanda16.exe`, 8 bien
    en el banco). WLDAP32: sus ordinales no se adivinan; todos van a una
    funcion que dice "la casa no tiene LDAP" (libcurl solo la usa con
    ldap://).
  - [x] P0.2e -- RTTI (__RTDynamicCast, __RTtypeid, __unDName): tanda 17,
    HECHA el 30-09 (`rtti.rs`, y bad_cast/bad_typeid en `msvcp_errores`;
    `tanda17.exe`, 11 bien en el banco). __unDName: tipos y nombres de
    funciones (UNDNAME_NAME_ONLY); la firma entera de una funcion, todavia
    no (da su nombre).
  - [x] P0.2i -- `__CxxFrameHandler4` (30-09): `cxx4.rs` lee las tablas
    comprimidas (medidas en `tanda4m.exe`, hecho con `cl` 19.44 en el
    Windows del propietario) y el manejador de FH3 las usa; `tanda4m.exe`
    dice `bien` 11 veces en el banco, como en Windows.
  - [x] P0.2f -- iostreams y locale de MSVCP140, con sus DATOS: tandas 18-19.
    Tanda 18 (30-09): el locale, 36 de las 79 (msvcp_locale.rs, tanda18.exe;
    16/16 en Windows). Tanda 19 (30-09): los flujos, las 43 que quedaban
    (msvcp_flujos.rs, msvcp_tiempo.rs, tanda19.exe).

- [ ] P0.2g -- DONDE GUARDA EL JUEGO: hoy USERPROFILE (y con el APPDATA,
  LOCALAPPDATA y "Saved Games") es la carpeta del `.exe`, y la del juego esta
  en D:, de solo lectura. Hace falta un perfil en la particion de datos de
  BMO-X, y que la casa sepa crear carpetas (CreateDirectoryW dice NO hoy).
- [ ] P0.2h -- El globo de `run` recomienda `smp all` para un juego SOLO
  cuando sirva: con H2.2 (cada hilo de Windows, un hilo del kernel). Hoy los
  hilos del juego se turnan en una tarea y los nucleos de mas no los ven.
- [x] P0.3a -- El DIARIO de la casa: cada funcion de Windows llamada por
  primera vez, en orden, a `informe/diario.txt` (orden, hilo, DLL, nombre).
  `run sys/proton-x.bex --diario <ruta>`. **HECHO el 30-09:** trampolines
  hechos de antemano en `diario.rs`; `diario.exe` dice lo mismo con y sin
  diario (doubles en xmm, siete argumentos, GetProcAddress) y el banco lee
  su fichero: diez funciones, una vez cada una, en orden.
- [ ] P0.3b -- Cada `QueryInterface` con un IID que la casa no conoce, al
  diario, con el IID entero (los metodos de COM no pasan por trampolines).
- [x] P0.4a -- El tope del `.exe` en la app (`TOPE_EXE`, 16 MiB): el de
  Cyberpunk mide 57 MiB (el censo del metal). Leerlo por secciones, como ya
  hace el censo. **Hecho (30-09):** sin tope; solo las cabeceras pasan por el
  monton y cada seccion va del disco a su RVA (`colocar_en`). Queda UN techo,
  el del kernel: codigo y datos, cada uno en un bloque de 64 MiB. El censo
  dice ahora `LA IMAGEN: ... cabe / NO CABE`; si no cabe, es ring 0.
- [ ] P0.4b -- **LA DECLARACION DE IMAGEN `[RING 0]`** (permiso del propietario
  el 30-09: "si al kernel"). Esquema escrito ANTES del codigo; abajo.
  - [x] P0.4b.1 -- `bmo-imagen-juicio` (puro, como `bmo-carga-juicio`): el
    juez, con sus pruebas en el anfitrion. **HECHO el 30-09**: 8 pruebas
    (la de Cyberpunk cabe; sin RAM dice cuanta pide y cuanta hay; codigo y
    datos seguidos; lo desordenado, lo vacio y lo absurdo, no).
  - [x] P0.4b.2 -- El kernel: la operacion de declarar, la ventana de
    imagenes, sellar cada parte y devolverlo todo al morir. **HECHO el
    30-09**: `obj/imagen.rs`, `TASK_OP_IMAGEN_DECLARAR/PARTE/SELLAR`
    (0x37-0x39) y el peaje en `LA_COMPATIBILIDAD.md` 4.3. La pila del
    syscall mas hondo sigue cabiendo (29.662 de 40.960). Falta verlo en el
    metal (P0.4b.5).
  - [x] P0.4b.3 -- `bmo-abi` y `bmo::Imagen` en Ring 3. **HECHO el 30-09**
    (`userland/src/imagen.rs`: declarar, parte, sellar; el NO con su frase
    y sus MiB).
  - [x] P0.4b.4 -- La app: del censo a la declaracion, y cargar el `.exe` y
    las DLL del juego en sus partes (lo que hace el banco con `cargar_dll` y
    `registrar_dll`). **HECHO el 30-09** (`apps/proton-x/src/cargador.rs`):
    las DLL del juego junto al `.exe` (importadas y RETRASADAS), dependencias
    primero; una declaracion; cada seccion por un bloque de paso de 2 MiB;
    registrar, resolver, sellar y los DllMain. Las retrasadas se cargan (y su
    DllMain corre) al arrancar, no en su primera llamada: dicho. Las que el
    juego abre EN VIVO (LoadLibrary de un nombre en sus datos), todavia no.
  - [x] P0.4b.6 -- TRAMOS (30-09): el primer golpe del metal fue
    `bink2w64.dll`, con `.rdata` ENTRE dos secciones de codigo. Cada PE se
    declara ahora por tramos de paginas del mismo permiso (codigo, datos,
    codigo...: `bmo_proton_x::tramos`), el juez acepta que alternen, y se
    sellan todos los de codigo. `.rdata` queda R+W (Windows: solo R); una
    seccion escribible en una pagina de codigo sigue siendo un NO.
  - [x] P0.4b.7 -- El segundo golpe (30-09): la imagen ya se CONCEDE en el
    metal (25 modulos, 52 partes, codigo 70 MiB + datos 168 MiB), y
    `dbghelp.dll` pide a `api-ms-win-downlevel-kernel32-l2-1-0.dll`. Los API
    set "downlevel" se resuelven como su DLL; y el cargador APUNTA lo que
    falta de TODAS las DLL antes de decir NO (un viaje, la lista entera).
  - [x] P0.4b.8 -- EL JUEGO CORRIO (30-09): 4793 funciones resueltas, los 24
    DllMain, y el `.exe` hizo 59 llamadas (el diario) antes de un #PF en 0+0x8.
    Lo que el metal enseno: `LoadLibrary` de un API set es su anfitrion
    (kernelbase, ucrtbase) y `VirtualProtect` sobre la imagen contesta como
    Windows sin romper W^X (tanda 21). Y un MAPA de modulos con el diario,
    para leer el `rip` de la autopsia como modulo + RVA.
  - [ ] P0.4b.5 -- En el metal: los DllMain de las 26 corren y el `.exe`
    llega a su entrada (o el diario dice donde se paro).
- [ ] P0.4c -- **LA MEMORIA DEL JUEGO `[RING 0]`**: lo que el juego pide EN
  MARCHA (VirtualAlloc, HeapAlloc: varios GiB). Hoy la casa tiene UN monton
  de un bloque de 64 MiB. Misma idea que P0.4b (dinamica, declarada y
  juzgada, devuelta al morir); se traza cuando P0.4b corra y el diario diga
  cuanto pide de verdad.

### P0.4b -- el esquema

**POR QUE HACE FALTA EL KERNEL** (lo pregunto el propietario). Hoy un proceso
de BMO-X tiene, como mucho, **8 bloques vivos de 64 MiB**, cada uno
contiguo en fisico, dentro de una ventana de VA de **512 MiB** que no se
reusa (`obj/memory.rs`: `MAX_PETICIONES`, `MAX_BYTES`, `MEMORIA_VA_TOPE`).
Eso sobra para DOOM y el escritorio, y no llega para Cyberpunk:

```text
   el .exe          79 MiB = 43 de codigo + 37 de datos     2 partes
   sus 26 DLL       unos 52 partes (codigo y datos de cada una)
   libxess.dll      70 MiB de datos: UNA parte ya no cabe en un bloque
   la casa          su monton y su codigo sellado           2-3 bloques
```

Son ~55 partes contra 8 bloques, una parte mayor que el bloque maximo, y
mas que los 512 MiB de VA. Ninguna de las tres cosas se arregla desde Ring
3: son los topes del kernel. Y **no se suben para todos**: DOOM no tiene que
poder pedir 500 MiB por accidente. Lo que cambia es que un proceso que SABE
lo que necesita lo DECLARA, y el kernel lo juzga.

**LA IDEA (OM: nada se concede sin declararse).**

```text
   la app      DECLARA la imagen entera, UNA vez: la lista de partes
               (de que PE, codigo o datos, cuantos bytes), sacada del censo
   el juez     (puro, sin constantes) la mira contra lo que el kernel le da:
               cuantas partes, la mayor, el total, la RAM libre AHORA menos
               el margen del kernel, y la VA de la ventana
   el kernel   CONCEDE (y dice donde queda cada parte) o NIEGA con el motivo
               y los dos numeros ("pide 612 MiB, hay 540 libres")
```

**DINAMICO, NO UN TOPE FIJO** (el propietario, 30-09: "me sobra RAM; si el
juego pide mas, que el kernel sea dinamico"). El techo de una declaracion es
**la RAM libre en ese momento menos `MARGEN_DEL_KERNEL`** (los 64 MiB que ya
guarda la admision en `task/admitir.rs`), no un numero escrito. Con la RAM
de la maquina del propietario, el juego recibe lo que pida mientras la
maquina lo tenga; si no lo tiene, se le dice que no y por que, ANTES de
cargar nada.

**LAS REGLAS.**

- **Una declaracion por proceso**, juzgada una vez. Una segunda se niega
  ("ya declaro"). Lo concedido solo vale para ESA declaracion: una parte no
  crece ni se agrega despues.
- **Cada parte se sella como hoy**: nace escribible y SIN ejecucion; la de
  codigo se sella (R+X, sin escritura, irreversible), igual que
  `MEM_OP_SELLAR`. W^X no cambia.
- **Sin fisica contigua**: las partes de imagen no las ve ningun aparato
  (no hay DMA), asi que se mapean pagina a pagina con marcos sueltos. Asi
  400 MiB no dependen de encontrar un hueco seguido en la RAM.
- **Su propia ventana de VA**, lejos de los bloques y de los prestamos (que
  acaban en `0x1_4000_0000`): una ventana de imagenes arriba, con cada `.exe`
  y DLL en su sitio y su codigo y sus datos SEGUIDOS (lo que ya exige el
  cargador de la app).
- **Los demas no cambian**: `MAX_PETICIONES` (8), `MAX_BYTES` (64 MiB) y la
  ventana de bloques siguen igual para DOOM, el escritorio y cualquier app
  que no declare.
- **Al morir el proceso**, las partes se ponen a cero y se devuelven, como
  los bloques (`process_died`).
- **Todo queda en el informe**: la declaracion (partes, MiB de codigo y de
  datos), el veredicto, y si se niega, el motivo con sus numeros. `run` lo
  pinta, como ya pinta `RECHAZO_MOTIVO`.

**EL JUEZ, puro** (`platform/shared/bmo-imagen-juicio`, como
`bmo-carga-juicio` y `bmo-prestamo-juicio`): recibe las partes y los limites
que le da el kernel (partes maximas, RAM libre, margen, la ventana) y
devuelve `Concedida { donde va cada parte }` o `Negada { motivo, pide,
hay }`. No tiene ni una constante de medida: un juez que no puede
inventarse el techo no puede equivocarse en el techo. Sus pruebas corren en
el anfitrion (una declaracion como la de Cyberpunk; una de mas; una parte
de 0 bytes; una que no cabe en la ventana; codigo y datos que deben quedar
seguidos).

- [ ] P0.4d -- **LA CACHE DE LA IMAGEN VERIFICADA** (idea del propietario,
  30-09: *"la primera vez tarda, es normal; pero el cache se acuerda, y la
  segunda vez se verifica solo lo sospechoso: si todo es lo mismo, pasa; si
  no, lo rebota"*). La primera carga de Cyberpunk lee ~80 MiB del `.exe` y sus
  26 DLL de D: (NTFS, solo lectura), y cada vez son los mismos bytes. La idea:
  - la PRIMERA vez se lee todo, y se apunta la HUELLA de cada fichero (medida,
    fecha y un resumen por seccion) junto a la imagen ya colocada, en la
    particion de datos de BMO-X (D: no se toca);
  - la SEGUNDA vez se mira solo la huella (lo "sospechoso": lo que pudo
    cambiar, p. ej. un parche del juego): si cuadra, la imagen sale de la
    cache y no se vuelve a leer ni a relocalizar; si NO cuadra, se rebota y
    se carga de cero (y se dice cual cambio);
  - la cache tambien depende de `proton-x.bex` (las direcciones de la casa
    estan resueltas dentro): otra version, otra cache;
  - la ventana de imagenes y el orden de la declaracion son deterministas
    (`bmo-imagen-juicio`), asi que las bases salen iguales cada vez.
  Va DESPUES de que el juego cargue una vez entero: primero, que funcione.
- [ ] P0.4 -- `run window/.../Cyberpunk2077.exe` desde D: (solo lectura) en
  el metal. Llega hasta donde llegue. **Como se sabe:** la autopsia o el
  aviso de la casa dicen DONDE se paro, y el diario, POR QUE CAMINO.
- [ ] P0.5 -- La lista de lo que pidio el primer contacto, repartida en las
  tres secciones de abajo (cada casilla, marcada "LO PIDIO" o "todavia no").

---

# 2. D3D12 DE JUEGO

## Lo que ya hay (no se rehace)

`d3d12.rs`, `tuberia.rs`, `dxgi.rs` de la casa; el lote y su ejecutor (en la
CPU y en la 3060: la receta VRN2, la profundidad en VRAM, texturas y
muestreadores); los traductores de DXIL y de SM5 con su cache ("pagar una
vez"); BMOX-12, el cubo y HelloTexture de punta a punta.

## 2A. Medir lo que pide el juego

- [ ] D0.1 -- `rayosx` (en Windows, sobre la carpeta del juego) cuenta las
  interfaces y metodos de D3D12/DXGI que se importan o se piden por IID.
  **Como se sabe:** una tabla en esta seccion, medida.
- [ ] D0.2 -- Los sombreadores del juego: sacar los DXIL de su cache de
  sombreadores (sin tocar D:, sobre una copia en C: del propietario) y contar
  sus OPCODES, su modelo (6.x) y sus recursos. **Como se sabe:** la lista de
  opcodes que los traductores todavia no saben, con cuantas veces aparece
  cada uno.
- [ ] D0.3 -- El diario de P0.3 filtrado a D3D12: que metodos, cuantas
  veces, en que orden hasta el primer `Present`.

## 2B. El dispositivo y lo que dice que sabe

- [ ] D1.1 -- `D3D12CreateDevice` a nivel 12_1 (y 12_2 si la 3060 lo dice).
  **Como se sabe:** `tandaD1.exe` en Windows con la 3060 y en el banco dicen
  lo mismo.
- [ ] D1.2 -- `CheckFeatureSupport` ENTERO: `OPTIONS` 1 a 12,
  `SHADER_MODEL` (hasta 6.6), `ROOT_SIGNATURE` 1.1, `ARCHITECTURE1`,
  `GPU_VIRTUAL_ADDRESS_SUPPORT`, `FEATURE_LEVELS`, `SHADER_CACHE`. Lo que
  diga la 3060 real, campo a campo. **Como se sabe:** `tandaD1.exe` lo
  compara con los numeros medidos en Windows.
- [ ] D1.3 -- `FORMAT_SUPPORT` y `FORMAT_INFO` de los ~120 formatos DXGI (la
  tabla de la 3060). **Como se sabe:** `tandaD1.exe`, formato a formato.
- [ ] D1.4 -- La cadena de `QueryInterface`: `ID3D12Device1` a `Device9` y
  los `GraphicsCommandList1` a `7` (lo que D0.1 diga que se usa).
- [ ] D1.5 -- Sin capa de depuracion: `D3D12GetDebugInterface`, DRED y
  `ID3D12InfoQueue` contestan "no hay" como en un Windows sin las
  herramientas. **Como se sabe:** el juego sigue.
- [ ] D1.6 -- DXGI: `IDXGIFactory6::EnumAdapterByGpuPreference`, la 3060 con
  su VRAM (12 GiB) y su LUID, `IDXGIOutput6::GetDesc1` (HDR apagado),
  `CheckFeatureSupport(ALLOW_TEARING)`. **Como se sabe:** `tandaD1.exe`.
- [ ] D1.7 -- `IDXGIAdapter3::QueryVideoMemoryInfo` y
  `SetVideoMemoryReservation`: el PRESUPUESTO de VRAM (el juego carga
  texturas hasta el). **Como se sabe:** el presupuesto sale de la VRAM libre
  de la 3060 en BMO-X, y cambia al pedir.

## 2C. La memoria y los recursos

- [ ] D2.1 -- `GetCopyableFootprints` y `GetResourceAllocationInfo` EXACTOS
  (alineaciones de 256 y 512, de 64 KiB y de 4 MiB). **Como se sabe:**
  `tandaD2.exe`, contra Windows, con texturas de todas las formas.
- [ ] D2.2 -- Los MONTONES de verdad: `CreateHeap`, recursos colocados
  (`CreatePlacedResource`) y que se solapan (ALIAS), sobre VRAM de la 3060.
- [ ] D2.3 -- Montones de subida y de lectura: `Map` persistente, memoria
  combinada para escribir. **Como se sabe:** un `.exe` escribe 256 MiB por
  un `Map` y la 3060 los lee.
- [ ] D2.4 -- Recursos RESERVADOS (tiled: `UpdateTileMappings`,
  `CopyTileMappings`) -- SOLO si D0 dice que el juego los usa.
- [ ] D2.5 -- Texturas de todas las formas: mips, arrays, 3D, cubos;
  `CopyTextureRegion` y `CopyBufferRegion` con subrecursos. **Como se sabe:**
  imagen igual a la de Windows (se comparan los pixeles).
- [ ] D2.6 -- Los formatos COMPRIMIDOS BC1 a BC7: la 3060 los lee de por si
  (solo es el formato en la cabecera de la textura); el interprete de la CPU
  los descomprime. **Como se sabe:** una textura BC7 se ve igual en los dos.
- [ ] D2.7 -- Los formatos sin tipo (`TYPELESS`) y sus vistas que lo cambian,
  con las reglas de Windows de que se puede ver como que.

## 2D. Los descriptores y la firma raiz

- [ ] D3.1 -- Montones de descriptores A ESCALA: un millon de CBV/SRV/UAV
  visibles al sombreador, y copiar descriptores en masa.
- [ ] D3.2 -- Firma raiz 1.1 ENTERA: constantes, CBV/SRV/UAV directos, tablas
  con rangos sin limite (`unbounded`), samplers estaticos, sus banderas.
  **Como se sabe:** `tandaD3.exe` serializa y deserializa firmas y Windows
  da los mismos bytes.
- [ ] D3.3 -- SIN ATAR (bindless, modelo 6.6: `ResourceDescriptorHeap`),
  si D0.2 lo encuentra.
- [ ] D3.4 -- Los UAV: bufferes con tipo, crudos y estructurados; sus
  contadores; `ClearUnorderedAccessView*`.

## 2E. Los sombreadores

- [ ] D4.1 -- Los opcodes que D0.2 dijo que faltan, en los dos traductores
  (DXIL y SM5), de los mas usados a los menos. Uno por commit.
- [ ] D4.2 -- COMPUTE (`Dispatch`): memoria compartida del grupo, barreras,
  atomicas. **Como se sabe:** un sombreador de suma en paralelo da lo mismo
  que en Windows.
- [ ] D4.3 -- Las operaciones de ONDA (wave, modelo 6.0): la 3060 va en
  warps de 32. **Como se sabe:** `WaveActiveSum` y compania, igual que en
  Windows.
- [ ] D4.4 -- Pixel: `discard`, derivadas, 8 destinos (MRT), profundidad de
  salida, fusion (blend) completa, estarcido (stencil).
- [ ] D4.5 -- Vertices: instancias, `SV_VertexID`/`InstanceID`. Casco y
  dominio (teselado) y geometria SOLO si D0.2 los encuentra.
- [ ] D4.6 -- La CACHE de PSO a escala: miles de pipelines traducidos una vez
  y guardados en disco; `ID3D12PipelineLibrary` (el juego guarda la suya).
  **Como se sabe:** el segundo arranque no traduce nada.
- [ ] D4.7 -- Lo que un traductor no sepa, al interprete de la CPU, DICHO
  (una linea en el diario), nunca una imagen rota en silencio.

## 2F. Ejecutar un fotograma entero en la 3060

- [ ] D5.1 -- Tres colas (DIRECT, COMPUTE, COPY) y `ExecuteCommandLists`
  con muchas listas.
- [ ] D5.2 -- Vallas (fences) entre colas: `Signal`/`Wait` del lado de la
  GPU, `SetEventOnCompletion`.
- [ ] D5.3 -- Las BARRERAS de recursos (transicion, UAV, alias) como
  vaciados de cache de la 3060.
- [ ] D5.4 -- `ExecuteIndirect` (firmas de ordenes), si D0 lo encuentra: los
  juegos que dibujan "desde la GPU" viven de el.
- [ ] D5.5 -- Consultas: sellos de tiempo, oclusion, estadisticas;
  `ResolveQueryData`, `GetTimestampFrequency`.
- [ ] D5.6 -- La receta de la 3060 a escala: miles de dibujos por lote,
  varios destinos, fusion, estarcido, recortes y ventanas multiples.
  **Como se sabe:** `cubo12` con 10.000 cubos, a sus fps medidos.
- [ ] D5.7 -- La cadena de intercambio de juego: FLIP_DISCARD con 3 bufferes,
  `ResizeBuffers`, `GetFrameLatencyWaitableObject`, VSync y sin VSync.
- [ ] D5.8 -- Una imagen del JUEGO: el primer fotograma de Cyberpunk que se
  ve (aunque sea el logo). **Como se sabe:** una foto de la pantalla.

## 2G. Lo que no aporta (al final)

- [ ] D6.1 -- DXR (rayos): apagado en la configuracion del juego; la casa
  dice "no hay" en `OPTIONS5`.
- [ ] D6.2 -- DLSS (`nvngx`), Reflex, la superposicion: EN VIVO, fuera.
- [ ] D6.3 -- FSR 2 va dentro del juego como COMPUTE: sale gratis con D4.2.

---

# 3. EL SONIDO DEL JUEGO

## Lo que ya hay (no se rehace)

El audifono entro en el metal ([`PLAN_AUDIO.md`](PLAN_AUDIO.md), A10): tubo
isocrono a 48.000 Hz, estereo, WAV, el bufer prestado y el amplificador con
medidor ([`PLAN_EL_SONIDO.md`](PLAN_EL_SONIDO.md), S4). Pendientes alli: la
cadena S1, la fraccion S2, el mezclador S3, el maestro S4c y las voces S4d.

## 3A. Medir lo que pide el juego

- [ ] A0.1 -- El censo EN VIVO clase `sonido` y el diario de P0.3: WASAPI
  (`MMDevAPI`, `IAudioClient`), XAudio2, o los dos. **Como se sabe:** una
  tabla aqui, medida.
- [ ] A0.2 -- El formato que pide el juego (canales, frecuencia, bits) y su
  periodo.

## 3B. Del lado del juego (la casa)

- [ ] A1.1 -- COM lo justo: `CoInitializeEx`, `CoCreateInstance` de
  `MMDeviceEnumerator`, `PROPVARIANT`, `IPropertyStore` (el nombre del
  audifono). **Como se sabe:** `tandaA1.exe` enumera en Windows y en el
  banco, y mira RELACIONES (no el nombre de tu audifono).
- [ ] A1.2 -- `IMMDeviceEnumerator`: `GetDefaultAudioEndpoint`,
  `EnumAudioEndpoints`, `RegisterEndpointNotificationCallback` (sin avisos).
- [ ] A1.3 -- `IAudioClient`/`IAudioClient3`: `GetMixFormat`
  (WAVEFORMATEXTENSIBLE), `IsFormatSupported`, `Initialize` compartido y por
  evento, `GetBufferSize`, `GetDevicePeriod`, `SetEventHandle`,
  `Start`/`Stop`/`Reset`, `GetCurrentPadding`. **Como se sabe:**
  `tandaA1.exe` abre el dispositivo y lo cierra SIN SONAR, en Windows igual.
- [ ] A1.4 -- `IAudioRenderClient` (`GetBuffer`/`ReleaseBuffer`) e
  `IAudioClock` (`GetFrequency`/`GetPosition`): el RELOJ del sonido, del que
  cuelgan los videos (Bink).
- [ ] A1.5 -- `ISimpleAudioVolume`, `IAudioSessionControl` (sin eventos).
- [ ] A1.6 -- XAudio2, SOLO si A0.1 lo encuentra: voces de origen, de mezcla
  y la maestra; un mezclador en la casa.
- [ ] A1.7 -- X3DAudio, si se usa: es solo matematicas, se prueba contra
  Windows numero a numero.
- [ ] A1.8 -- Remuestrear (44.100 a 48.000) y bajar de 7.1 a estereo (el
  alt setting del audifono es estereo): es S2 y S6 de `PLAN_EL_SONIDO`.

## 3C. Del lado de BMO-X

- [ ] A2.1 -- S1, S2 y S3 de `PLAN_EL_SONIDO`: la cadena, la fraccion, el
  mezclador (lo del juego es UNA fuente mas).
- [ ] A2.2 -- S4c y S4d: el maestro y las voces: la puerta por la que una app
  de Ring 3 entrega su bufer y el kernel lo toca. `[RING 0]`
- [ ] A2.3 -- La POSICION reproducida, leida del tubo, para `IAudioClock`
  (sin ella, el video y el sonido se separan).
- [ ] A2.4 -- La latencia medida (10 a 20 ms) y los vacios (underruns)
  contados en la cabina de audio.

---

# 4. VARIOS NUCLEOS

## Lo que ya hay (no se rehace)

`smp` despierta 12 de 12 en el metal (08-07), sus obreros laten, `crew`
reparte y el `atril` recibe encargos de Ring 3. Los hilos de Windows de la
casa son COOPERATIVOS en UNA tarea (`hilos.rs`). Y hay **42 `unsafe impl
Sync` en 34 ficheros de la casa** que dicen, cada uno, "una tarea, hilos
cooperativos": el dia que dos hilos corran A LA VEZ, cada uno es un sitio
donde dos `&mut` del mismo estado pueden chocar.

## 4A. Medir

- [ ] H0.1 -- El diario de P0.3: cuantos hilos crea el juego, con que
  prioridad y afinidad, y con que se esperan (SRW, secciones criticas,
  `WaitOnAddress`, eventos). **Como se sabe:** una tabla aqui.
- [ ] H0.2 -- La lista de los 42 `Sync` de la casa, cada uno con su clase:
  (a) no se toca desde otro hilo, (b) necesita cerrojo, (c) debe ser por
  hilo. **Como se sabe:** un guardian nuevo que falla si aparece un `Sync`
  sin su clase escrita.

## 4B. El kernel `[RING 0]`, pieza a pieza

- [ ] H1.1 -- Colas de ejecucion por nucleo y el reloj del LAPIC en cada uno;
  una IPI para "replanifica".
- [ ] H1.2 -- Los hilos de UN proceso en varios nucleos: el mismo espacio de
  direcciones; las IPI de TLB al desmapear o cambiar permisos.
- [ ] H1.3 -- El GS (el TEB) por hilo en cada nucleo, puesto en cada cambio.
- [ ] H1.4 -- El estado de la FPU, SSE y AVX por hilo (XSAVE): el juego usa
  AVX2.
- [ ] H1.5 -- Esperar por DIRECCION entre nucleos (lo del futex): es
  `WaitOnAddress`, las SRW y las variables de condicion.
- [ ] H1.6 -- Temporizadores de 1 ms (`timeBeginPeriod`) por nucleo.
- [ ] H1.7 -- La topologia de verdad: 6 nucleos, 12 hilos, la L3; SMT no son
  seis nucleos mas (`SMP_MAESTRO`, seccion 4).
- [ ] H1.8 -- La cabina de SMP dice quien espera a quien (sin esto, un
  bloqueo entre nucleos es una pantalla congelada muda).

## 4C. La casa

- [ ] H2.1 -- Los 42 `Sync`, segun H0.2: cerrojos donde toca, por hilo donde
  toca. Uno por commit, con su prueba.
- [ ] H2.2 -- `CreateThread` = un hilo del kernel con su TEB (deja de ser
  cooperativo), detras de una bandera hasta que H2.1 este entero.
- [ ] H2.3 -- Eventos, mutex, semaforos y `WaitForMultipleObjects` (uno o
  todos) sobre esperas del kernel, atomicas.
- [ ] H2.4 -- TLS y FLS por hilo; las llamadas TLS al crear y al acabar un
  hilo.
- [ ] H2.5 -- El monton (`HeapAlloc`) seguro con varios hilos.
- [ ] H2.6 -- El pool de hilos (`kernel32_pool`) sobre obreros de verdad.
- [ ] H2.7 -- D3D12 desde varios hilos: el juego GRABA listas de ordenes en
  paralelo; la casa de D3D12 tiene que aguantarlo.
- [ ] H2.8 -- `GetLogicalProcessorInformation(Ex)`,
  `SetThreadAffinityMask`, `SetThreadIdealProcessor`, `SetThreadPriority` y
  MMCSS (`avrt.dll`) con la topologia de H1.7.

## 4D. Pruebas

- [ ] H3.1 -- `tandaH1.exe`: 12 hilos con `Interlocked`, secciones
  criticas, SRW y variables de condicion; el resultado, igual que en Windows.
  Primero en la casa cooperativa (debe dar lo mismo), despues con H2.2.
- [ ] H3.2 -- La misma, en el metal, mil veces seguidas sin bloquearse.

---

# 5. EL ORDEN QUE PROPONE ESTE PLAN

```text
   1  P0.1 a P0.5    el primer contacto: el juego dice que pide
   2  D0, A0, H0     las tres medidas (sin tocar nada)
   3  D1, D2         el dispositivo y la memoria: pruebas contra la 3060 de
                     Windows, sin riesgo
   4  A1.1 a A1.5    el sonido del lado de la casa, SIN SONAR todavia
   5  H0.2, H2.1     los 42 Sync ordenados (sin encender nucleos)
   6  D3, D4, D5     descriptores, sombreadores, el fotograma
   7  H1 [RING 0]    los nucleos, con permiso
   8  A2 [RING 0]    el sonido saliendo por el audifono
   9  D5.8           la primera imagen del juego
```

La regla: nada de la fila 7 empieza sin la fila 5 hecha (encender nucleos con
42 sitios que suponen uno solo seria buscar fallos a ciegas).
