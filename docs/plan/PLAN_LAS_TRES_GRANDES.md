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
    Lo que el metal mostro: `LoadLibrary` de un API set es su anfitrion
    (kernelbase, ucrtbase) y `VirtualProtect` sobre la imagen contesta como
    Windows sin romper W^X (tanda 21). Y un MAPA de modulos con el diario,
    para leer el `rip` de la autopsia como modulo + RVA.
  - [x] P0.4b.9 -- El mapa cazo el golpe (30-09): `rip 0x10061bbad9` es
    `libxess_fg.dll` + 0x22bad9, en su DllMain. Tiene TLS propio y la casa
    solo daba TLS al `.exe`: ahora cada modulo con TLS tiene su indice (el
    `.exe` el 0), su bloque en cada hilo y sus callbacks antes de su
    DllMain (tanda 22). Y tiempos por fase, para decidir la cache.
  - [ ] P0.4b.5 -- En el metal: los DllMain de las 26 corren y el `.exe`
    llega a su entrada (o el diario dice donde se paro).
- [ ] P0.4c -- **LA MEMORIA DEL JUEGO `[RING 0]`**: lo que el juego pide EN
  MARCHA (VirtualAlloc, HeapAlloc: varios GiB). El metal lo dijo (30-09):
  recien saltado a su entrada, `GlobalMemoryStatus` + `VirtualAlloc` de mas
  de 64 MiB, y la casa solo daba bloques de 64 MiB (8 por proceso).
  - [x] P0.4c.1 -- El juez puro: `bmo-imagen-juicio::reserva` (rango y RAM).
  - [x] P0.4c.2 -- El kernel: `TASK_OP_RESERVA_HACER/DESHACER` (0x3A, 0x3B),
    una ventana de 128 GiB (hoy 384: P0.4c.8) en `0x20_0000_0000`, paginas a cero R+W sin X,
    juzgadas contra la RAM libre de ahora; al morir las devuelve el espacio
    (`PTE_NUESTRA`). Userland: `bmo::reserva`.
  - [x] P0.4c.3 -- `Regiones` piden y devuelven las paginas por tiradas.
  - [x] P0.4c.4 -- La casa: con `Plataforma::reserva`, VirtualAlloc RESERVA
    solo direcciones y cada COMMIT pide sus paginas; las arenas del monton
    salen de ahi y de la medida que haga falta (hasta 64 arenas);
    GlobalMemoryStatus dice la RAM del kernel. tanda23 (16 bien); PROTON-X
    la usa en BMO-X.
  - [x] P0.4c.5 -- En el metal (13:37): el juego pasa su VirtualAlloc
    grande, crea semaforos y eventos, y SE RINDE: IsProcessorFeaturePresent,
    RtlCaptureContext/LookupFunctionEntry/VirtualUnwind y
    UnhandledExceptionFilter (la forma de __report_gsfailure o de un
    abort/terminate del CRT). La casa tiraba el registro de la excepcion:
    ahora dice codigo, direccion y parametros; e IsProcessorFeaturePresent
    dice la verdad del Ryzen (antes: SSE, SSE2 y NX). tanda24 (12 bien).
  - Anotado: `XCR0_PRESERVADO` del BEF2 dice x87+SSE, pero `plat/trap.rs`
    guarda con XSAVE todo lo de XCR0 (0x7: AVX incluido) en cada trap y el
    syscall usa `xsaveopt64`. Hay que comprobar que TODOS los caminos que
    cambian de tarea guardan los YMM antes de subir la constante (el juego
    usa AVX si CPUID lo dice, y CPUID lo dice).
  - [x] P0.4c.6 -- El metal (30-09, 13:04): la carga en ~4 s (disco a 290
    MiB/s, cabeceras 0,55 s), el juego salto y RESERVO 64 GiB de golpe; la
    cuenta de `Regiones` era un u32 POR PAGINA (64 MiB) y el monton del
    cargador dijo que no. Ahora va por TIRADAS: crece con lo hecho.
  - [x] P0.4c.7 -- El metal (30-09): el azar de `rand_s`. Un constructor
    global crea un `std::random_device`, que acaba en SystemFunction036
    (RtlGenRandom, por su API set o por ADVAPI32); la casa no la tenia y el
    CRT hacia abort(). tanda25 (9 bien, Windows igual).
  - [x] P0.4c.8 -- El metal (30-09): pasado el azar, su redMemory se rindio
    con "Out of Memory!" (un `int3` a proposito). La casa ahora dice cada NO
    de memoria con el diario encendido, y el metal lo dijo: 16 + 64 + 32 GiB
    y pools de 1 y 4 GiB, y el noveno no cupo en la ventana de 128 GiB. La
    ventana pasa a 384 GiB, hasta el final de `PML4[0]` (lo que
    `destroy_address_space` recorre): solo direcciones, cada pagina la
    sigue juzgando la RAM libre. Si pide mas, lo siguiente es caminar mas
    entradas del PML4 al morir.
  - Anotado para exprimir: el monton hace cada arena entera al pedirla (64
    MiB de RAM de golpe), podria hacerla a medida que crece.

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
    fecha y un resumen por seccion) junto a la imagen ya colocada, en
    ESTRATOS (el disco de BMO-X); PERSONAL (D:, NTFS) es de solo lectura y no
    se toca, tampoco para la cache (el propietario lo confirmo, 30-09);
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
- [~] D5.5 -- Consultas: sellos de tiempo, oclusion, estadisticas;
  `ResolveQueryData`, `GetTimestampFrequency`. 05-10: la OCLUSION (y la
  binaria) cuenta de verdad y `SetPredication` salta lo que debe (E2.7 de
  la ESCALERA, `proton-x-casa/src/consultas.rs`); los sellos de tiempo y
  ResolveQueryData ya estaban. Queda: las estadisticas (dan ceros), que la
  3060 cuente (hoy esos lotes van por la CPU) y verlo en Cyberpunk.
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

- [x] A0.1 -- El censo EN VIVO clase `sonido` y el diario de P0.3: WASAPI
  (`MMDevAPI`, `IAudioClient`), XAudio2, o los dos. **Como se sabe:** una
  tabla aqui, medida.

  ```text
     medido en el metal el 03-10 (cuarta corrida, SYSPROTO y DIARIO)
     WASAPI    SI: CoCreateInstance {BCDE0395-E52F-467C-8E3D-C4579291692E}
               = CLSID_MMDeviceEnumerator, y la casa no la tiene: SIN SONIDO
     XAudio2   no: ni LoadLibrary de xaudio2_*.dll ni importacion
     Bink      bink2w64.dll levanta sus dos hilos: sus videos (la entrada)
               cuelgan del RELOJ del sonido (A1.4)
  ```
- [ ] A0.2 -- El formato que pide el juego (canales, frecuencia, bits) y su
  periodo.

## 3B. Del lado del juego (la casa)

- [x] A1.1 -- COM lo justo: `CoInitializeEx`, `CoCreateInstance` de
  `MMDeviceEnumerator`, `PROPVARIANT`, `IPropertyStore` (el nombre del
  audifono). **Como se sabe:** `tandaA1.exe` enumera en Windows y en el
  banco, y mira RELACIONES (no el nombre de tu audifono). (03-10:
  `proton-x-casa/src/wasapi.rs`; probado por la vtabla como un `.exe` en
  `proton-x-casa/tests/wasapi.rs`; `tandaA1.exe` sigue pendiente.)
- [x] A1.2 -- `IMMDeviceEnumerator`: `GetDefaultAudioEndpoint`,
  `EnumAudioEndpoints`, `RegisterEndpointNotificationCallback` (sin avisos).
  (03-10, `wasapi.rs`: una salida; de entrada, E_NOTFOUND.)
- [x] A1.3 -- `IAudioClient`/`IAudioClient3`: `GetMixFormat`
  (WAVEFORMATEXTENSIBLE), `IsFormatSupported`, `Initialize` compartido y por
  evento, `GetBufferSize`, `GetDevicePeriod`, `SetEventHandle`,
  `Start`/`Stop`/`Reset`, `GetCurrentPadding`. **Como se sabe:**
  `tandaA1.exe` abre el dispositivo y lo cierra SIN SONAR, en Windows igual.
  (03-10, `proton-x-casa/src/wasapi_flujo.rs`; el evento lo enciende el
  latido del planificador, una vez por periodo.)
- [x] A1.4 -- `IAudioRenderClient` (`GetBuffer`/`ReleaseBuffer`) e
  `IAudioClock` (`GetFrequency`/`GetPosition`): el RELOJ del sonido, del que
  cuelgan los videos (Bink). (03-10: lo sonado sale del anillo del tubo
  (`apps/proton-x/src/sonido.rs`) o, sin aparato, de la hora.)
- [x] A1.5 -- `ISimpleAudioVolume`, `IAudioSessionControl` (sin eventos).
  (03-10, con `IAudioStreamVolume` e `IChannelAudioVolume`.)
- [ ] A1.6 -- XAudio2, SOLO si A0.1 lo encuentra: voces de origen, de mezcla
  y la maestra; un mezclador en la casa.
- [ ] A1.7 -- X3DAudio, si se usa: es solo matematicas, se prueba contra
  Windows numero a numero.
- [x] A1.8 -- Remuestrear (44.100 a 48.000) y bajar de 7.1 a estereo (el
  alt setting del audifono es estereo): es S2 y S6 de `PLAN_EL_SONIDO`.
  (03-10, `proton-x/src/pcm.rs`: interpolacion lineal y la bajada de
  Windows, el centro y los de atras a -3 dB, sin el LFE.)

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

---

# 6. DONDE ESTAMOS -- LA ESCALERA HASTA JUGAR (03-10)

> El propietario: *"no olvides anotar donde estamos para avanzar"*, y
> *"vamos a EXPRIMIR luego los FPS hasta que esten estables; eso es posible
> porque es mi kernel"*. Esta es la escalera, de abajo arriba; se actualiza
> con cada corrida del metal (la ultima: 02-10, tanda53 de
> [`PLAN_LA_LUDOTECA.md`](PLAN_LA_LUDOTECA.md)).
>
> **Desde el 05-10, Cyberpunk es la VARA DE MEDIR** (R5 de
> [`PLAN_LA_ESCALERA_PROTON_X.md`](PLAN_LA_ESCALERA_PROTON_X.md)): una
> corrida por escalon cerrado de esa escalera, no una por muro. Un muro nuevo
> se apunta aqui y se arregla en el escalon que lo prueba solo, con su
> huella de Windows delante; la tabla de que casilla de este plan prueba
> cada escalon esta en su seccion 4.

```text
   nivel                                          donde se ve que esta hecho
   1  cargar el .exe y sus 24 DLL                 SYSPROTO: "4793 funcion(es)
                                                  resueltas; codigo SELLADO"
   2  arrancar por dentro: CRT, hilos, ficheros,  el DIARIO pasa de los
      Galaxy, red                                 DllMain al bucle del juego
   3  montar D3D12: dispositivo, firmas, PSO      <- AQUI (02-10): murio en
                                                  los primeros PSO
   4  la ventana y el primer Present              CreateWindowEx en el
                                                  DIARIO; una ventana
   5  la pantalla de carga y el MENU              los .archive leidos; el
                                                  menu en la pantalla
   6  dibujar en la 3060 de verdad                los sombreadores del juego
                                                  por el emisor sm86, no en
                                                  la CPU
   7  velocidad: los 6 nucleos y los hilos de     H1 [RING 0] (seccion 4)
      verdad
   8  sonido, mando y guardar partida             A2 [RING 0]; la entrada;
                                                  un save del juego
   9  EXPRIMIR: los FPS ESTABLES                  FRAPS-X: el fotograma
                                                  peor y los bajos del 1 %
                                                  y del 0,1 % dentro de su
                                                  presupuesto
```

**Estado al 03-10: nivel 3.** Antes giraba sin hacer nada (lo arreglo la
tanda52: preguntar la hora cede el turno); ahora sus hilos corren y crea
PSO. Murio porque el monton de PROTON-X no devolvia lo soltado (tanda53, ya
arreglado: `bmo-monton`). El siguiente paso que el juego YA PIDIO:

- [x] **N3.1 -- input layouts que no son float de 32 bits** (03-10,
  `proton-x/src/formato_ia.rs`): UNORM, SNORM, UINT, SINT, half, 10:10:10:2
  y 11:11:10, de 8, 16 y 32 bits; y SV_VertexID y SV_InstanceID, que no
  vienen del input layout (`lote::Fuente`). En la 3060 todavia no: esos van
  por la CPU (N6.1).
- [x] **N3.2 -- la corrida siguiente con el monton nuevo** (03-10): 2,2 MiB
  en uso al cargar (antes, 48 MiB llenos al morir): era basura, y ya no
  mata. Con 1065 PSO creados (163 ms) y 201 recursos.
- [x] **N4 -- la ventana** (03-10): CreateWindowExA, ShowWindow, su
  WM_PAINT y SetWindowPos; una ventana de 1898x1064 en el escritorio, NEGRA
  (el juego aun no presento nada: 0 ExecuteCommandLists, 0 Present).

**Estado al 03-10, segunda corrida: nivel 4.** Lo que lo para ahora, por
orden (las dos corridas murieron igual: es determinista):

- [x] **N4.1 -- el puntero NULO** (03-10, `trampas.rs`; falta verlo en el
  metal): a los ~11 s, el hilo principal llama a la
  direccion 0 desde `Cyberpunk2077.exe+0x1d4c6cf` con `rcx = 0` (no es un
  metodo COM: ahi `rcx` es el objeto). Los unicos nulos que la casa le dio
  fueron dos GetProcAddress: `crypt32!CryptMsgClose` e
  `iphlpapi!if_nametoindex`; `CryptMsgClose(NULL)` es una limpieza que
  Windows acepta y encaja con `rcx = 0` (tras fallar la firma, sin
  `wintrust.dll`). Sin probar: una llamada a 0 no pasa por el diario. El
  arreglo y la red: esas dos de verdad, y un GetProcAddress de algo que la
  casa no tiene de una DLL que si tiene da una TRAMPA con nombre (dice quien
  es al llamarla y devuelve 0), no un nulo.
- [x] **N4.2 -- el fondo de la ventana** (03-10, `pinceles.rs`): FillRect
  con cualquier pincel: los de serie (`GetStockObject`, el NEGRO de
  Cyberpunk), `CreateSolidBrush` y `DeleteObject`.
- [x] **N5.1 -- los sombreadores de Cyberpunk: espacios y registros altos**
  (03-10). El espacio de cada recurso sale de la parte PSV0
  (`dxil/recursos.rs`); cada lugar (espacio, registro, etapa) es una RANURA
  del programa (`programa::Ranuras`), el enlace une las del de vertices y
  las del de pixeles en una tabla, y la casa busca cada ranura en la root
  signature (`donde.rs`: rangos "a continuacion", sin medida, por etapa, y
  los samplers estaticos con su espacio). Ya no hay "t0..t31 y s0..s15", ni
  "una tabla con un espacio que no es el 0: se salta". Probado con
  `prueba/espacios.dxil` (dxc: t40 de space1, un array en space2, s20).

**Estado al 03-10, tercera corrida: nivel 4, sin el NULO.** Ya no murio a
los ~11 s llamando a la direccion 0: las dos de verdad (`CryptMsgClose`,
`if_nametoindex`) y las trampas lo pasaron (cinco de crypt32 recibieron
trampa y ninguna se llamo). Ventana de 1738x1064, 176 PSO. Murio a los 9 s
por OTRA cosa:

- [x] **N4.3 -- el monton lleno por los PSO** (03-10): `memory allocation of
  57344 bytes failed; monton 67098656 B en uso de 67108864`. Cada PSO
  guardaba sus dos `Sombreador` leidos (el modulo de LLVM, unas 6 veces el
  DXIL: ~380 KB por PSO) para sacar al dibujar solo el nombre de su
  funcion; se noto al abrir los PSO con N3.1. Ahora el PSO guarda los
  nombres, y lo compilado se comparte entre los PSO con el mismo VS, PS y
  layout (`enlaces.rs`, llave: la huella de 16 bytes del contenedor): al
  acertar no se lee ni el DXIL. El pulso dice cuantos enlaces distintos hay.

**Estado al 03-10, cuarta corrida: nivel 4, sin morir de monton.** N4.3
funciono: 1065 PSO en 27 ms con solo **119 enlaces distintos** (los demas se
comparten), de los que 12 se pueden correr hoy. Vivo a los 14,6 s con ~30
hilos y 637.000 llamadas por segundo, y paso de donde se quedaba: enumera
monitores y modos (`EnumDisplayMonitors`, `EnumDisplaySettingsW`), coloca su
ventana (`SetWindowPos`) y arranca los hilos de **Bink** (la entrada en
video). Todavia 0 ExecuteCommandLists y 0 Present: no dibujo nada. Despues
cayo con un fallo de Ring 3 (en `datos/fallos.txt`, pendiente de leer):

- [x] **N4.4 -- el fallo de Ring 3** (04-10, falta verlo en el metal):
  era `ffxDispatch` de `amd_fidelityfx_dx12.dll`, cargada en vivo; ver la
  undecima corrida. Un salto a 0 por `call [rip+..]`
  desde `Cyberpunk2077.exe+0x1d4c6cf` (ranura `+0x35848f8`), igual en la
  quinta y la sexta corrida. Tapados los dos sospechosos (retrasada sin
  DLL, NULL del sistema): la proxima corrida dice cual era.
  **Desde el 03-10 (novena corrida)** PROTON-X lo explica solo al arrancar
  (`apps/proton-x/src/el_nulo.rs`, la parte pura en `bmo_proton_x::nulo`):
  lee `datos/fallos.txt`, saca la casilla del `ff 15` de antes del retorno,
  comprueba que esos bytes siguen en la imagen, y dice en SYSPROTO su
  seccion, si es una importacion (y cual) o una variable, y CADA
  instruccion del `.exe` que la apunta (lee, escribe, llama); de las que
  escriben, el nombre que se paso en `rdx` (el de un GetProcAddress) y la
  importacion llamada justo antes. Y el pulso la VIGILA: cada foto del
  diario dice cuanto vale.
- [x] **N4.5 -- el sonido del juego** (03-10, falta oirlo en el metal):
  WASAPI en la casa (A1.1 a A1.5 y A1.8 de la seccion 3): el
  `MMDeviceEnumerator`, un aparato de salida, `IAudioClient3` por evento, lo
  del juego convertido a s16 estereo y al anillo del TUBO del audifono
  (`apps/proton-x/src/sonido.rs`), y su reloj. Sin aparato (o con el
  audifono de otro proceso), el reloj corre con la hora y el juego sigue,
  mudo.

**Estado al 03-10, quinta corrida: nivel 4, vivo a los 47 s** (tres veces
mas que la cuarta). Abre el audifono (`sonido: el audifono para el juego,
48000 Hz estereo`), crea su ventana de 1738x1064, y ya no se quejan ni el
discard, ni el G-buffer, ni SV_Position (N5.7 a N5.9). Por primera vez
`fallos.txt` llego: a los 47 s el `.exe` SALTO A 0 (un puntero a funcion
nulo; el retorno, `Cyberpunk2077.exe+0x1d4c6cf`; `rcx` 0, `r9` 0x438). La
autopsia no pudo dar el `call` (miraba antes del `rip`, que es 0): desde
el 03-10, con `rip` nulo, da los bytes de antes del RETORNO de `[rsp]`, y
la proxima corrida dice que registro y que tabla daban el nulo (N4.4). Lo
nuevo que dijo de sus sombreadores, ya hecho el mismo dia: las derivadas
(83, 84), las olas (118) y SV_Depth; quedan alloca/GEP (19, 43) y el
operando no constante (N5.4).

**Estado al 03-10, sexta corrida: el sonido ARRANCA.** `IAudioClient::
Initialize: 48000 Hz, 2 canal(es), 32 bits float, por evento: aceptado` y
`Start: el juego empieza a sonar, por el audifono` (el bloque con sonido de
verdad no llego: aun no habia nada que oir). Murio DOS veces (a los 40 s y
a los 8 min, otra sesion) en el MISMO sitio, ahora con el `call`: `ff 15
29 82 83 01`, un `call [rip+0x1838229]` -- la ranura `Cyberpunk2077.exe
+0x35848f8`, una importacion (o un puntero a funcion global) que vale 0.
Los dos sospechosos, tapados el mismo dia: una importacion RETRASADA cuya
DLL la casa no tiene (el cargador retrasado de MSVC lanza 0xC06D007E y, si
alguien la continua, salta a `pfnCur` = 0: ahora `pfnCur` es la TRAMPA de
`dll!funcion`, y se dice), y un NULL de GetProcAddress del sistema (ahora
se apunta tambien). Pidio ademas OperacionD3d 15 (acos): los arcos y los
hiperbolicos, hechos.

**Estado al 03-10, septima corrida: ventana, sin fotogramas.** Ya no
aparece la importacion retrasada (no era eso) y el diario apunta, por
primera vez, los NULL del sistema que da GetProcAddress (GetCurrentPackageId,
SetDefaultDllDirectories, AddDllDirectory, EnumSystemLocalesEx,
IsValidLocaleName, Get/SetFileInformationByHandle...). Sus sombreadores
pidieron OperacionD3d 10 (isfinite: hechos 8 a 11, isnan/isinf/isfinite/
isnormal), los arrays (N5.10, hecho) y el operando no constante (el de los
cbuffers, hecho; el bindless dira su nombre).

**Estado al 03-10, octava corrida: el mismo salto a 0** (a los 33 s, en
el hilo de la VENTANA, tid 7; `call [rip+..]` desde `+0x1d4c6cf`). La casa
ya da lo que el diario dijo que daba NULL: GetCurrentPackageId (sin
paquete), SetDefaultDllDirectories, AddDllDirectory, RemoveDllDirectory,
EnumSystemLocalesEx, IsValidLocaleName; y apunta al diario los
GetProcAddress sobre un handle que no es de ninguna DLL (antes, NULL
callado). Los sombreadores dijeron sus nombres: bindless (createHandle con
registro calculado, N5.4), las de bits (32: firstbitlow; hechas 30 a 34),
SampleCmpLevelZero (65) y TextureGather (73) -- hechos, con GatherCmp y
SampleCmp: el PCF de 2x2 de las sombras --, un UAV en el de pixeles
(N5.3c) y "un bucle con mas de una salida" (el estructurador). Y el
muestreador ANISOTROPICO, que se NEGABA (las texturas salian negras), se
lee lineal; el de comparacion guarda su funcion.

**Estado al 03-10, novena corrida: con `smp all`, lo mismo.** El mismo
salto a 0 (a los 53 s, tid 7, `+0x1d4c6cf`). Era lo esperado: los
nucleos que levanta `smp all` solo hacen faenas del kernel (`plat/smp/
crew.rs`: *"ni tareas de Ring 3 corriendo en otro nucleo"*); el juego, la
casa y el interprete de sombreadores son Ring 3 y corren en el BSP. El
100 % de CPU es UN nucleo. Lo que lo cambiaria: obreros de Ring 3 (repartir
la trama por franjas entre los nucleos) o la 3060 (N6). La pista nueva del
SYSPROTO: justo antes de la ventana del tid 8 el juego pide
`Wtsapi32.dll`, y la casa no la tiene (tambien `nvapi64.dll`,
`amd_fidelityfx_dx12`, `GFSDK_Aftermath_Lib.x64.dll`); la proxima corrida,
con `el_nulo.rs`, dice si la casilla es de una de ellas.

**Estado al 04-10, decima corrida: el lector del nulo CALLO.** El mismo
salto a 0 (a los 66 s, tid 7), y en el SYSPROTO ni una linea de
`el_nulo.rs`: el parser lee bien ese `fallos.txt` (probado con el del
metal), asi que no se abrio el fichero o el build no llego. La primera
version callaba si no podia leer: ahora dice SIEMPRE que miro y que salio
(el codigo del NO al abrir, o que el informe no acaba en un salto a 0), y
lee primero las autopsias que el KERNEL guarda de este arranque (`bmo::
autopsia_*`): con dos lanzamientos en el mismo arranque no depende del
disco. Del pulso: 23 enlaces distintos, 21 se corren; un hilo (4204) da
vueltas con Enter/LeaveCriticalSection.

**Estado al 04-10, undecima corrida: EL NULO TIENE NOMBRE.** El lector
lo dijo: la casilla `Cyberpunk2077.exe+0x35848f8` es una variable de
`.data`, la llenan en `+0x1d4df09` con `GetProcAddress(h, "ffxDispatch")`,
y la llaman cinco sitios sin mirar si es nula. Es la API de FSR 3.1 de
AMD, en `amd_fidelityfx_dx12.dll`: el juego hace
`LoadLibrary("amd_fidelityfx_dx12")` y la casa le daba NULL porque solo
cargaba las DLL de la tabla de importaciones. El arreglo, de raiz
(`apps/proton-x/src/en_vivo.rs` y `bmo_proton_x::en_vivo`): al arrancar
se cargan TAMBIEN las DLL de la carpeta del `.exe` que el `.exe` nombra en
sus datos (con o sin `.dll`, ASCII o UTF-16) y que se pueden resolver
enteras; la que no, se dice con lo que le falta y no se carga. LoadLibrary
ya las encuentra (`modulos::por_nombre` busca en las propias).

**Estado al 04-10, duodecima corrida: entraron tres, no la de FSR.** Con
la carga en vivo entraron `galaxy64.dll`, `GameServicesGOG.dll` y
`PxPvdSDK_x64.dll` (27 DLL del juego), pero `amd_fidelityfx_dx12` siguio
dando NULL y el mismo salto a 0 (a los 33 s). Lo que decidio el cargador no
se vio: la consola guarda las ultimas 200 lineas y las de "en vivo" eran las
primeras. Lo mas probable: le faltaba alguna funcion de la casa y la regla
"entera o nada" la dejo fuera. Ahora (1) a una DLL en vivo lo que la casa
no tiene se le da con TRAMPA con nombre (`tabla_o_trampa`; 256 trampas),
y solo la para una DLL del juego que no este; (2) se dicen tambien las
`.dll` de la carpeta que el `.exe` NO nombra; (3) todo eso y el lector del
nulo van ademas al DIARIO (`diario::apuntar_arranque`), que se guarda
entero. Y `datos/fallos.txt` no se abre desde PROTON-X (codigo 28, no
esta): el lector tiro de las autopsias del kernel.

**Lo que dijo de sus sombreadores** (SYSPROTO, cada texto una vez), y su
casilla:

```text
   OperacionD3d(12, 13, 14)     cos, sin, tan                    N5.6
   OperacionD3d(21, 22, 23)     exp, frac, log                   N5.6
   OperacionD3d(26, 27)         round_ne, floor                  N5.6
   OperacionD3d(131)            f16tof32                         N5.6
   OperacionD3d(82)             discard                          N5.7
   mas de un render target      el G-buffer (diferido)           N5.8
   el de pixeles lee SV_Position                                 N5.9
   Instruccion(19), (43)        alloca y GEP: arrays locales     N5.10
   OperacionD3d(118)            WaveReadLaneFirst                hecho
   OperacionD3d(83, 84)         ddx, ddy (quinta corrida)        hecho
   el de pixeles escribe SV_Depth (quinta corrida)               hecho
   un operando que deberia ser un entero constante               N5.4
```

**Lo que queda para que la 3060 PINTE un fotograma de Cyberpunk**, por
orden. Cada uno lo dice el DIARIO con su texto la primera vez que pasa;
la proxima corrida del metal dice cual pesa mas:

- [x] **N5.2 -- los cbuffers que no son b0** (03-10): b1.., en otros
  espacios, como CBV en la raiz, en una TABLA o como constantes de 32 bits
  (`SetGraphicsRoot32BitConstants`, que antes se tiraban). Cada cbuffer es
  una ranura; el enlace los APLANA en un bloque (`Enlace::constantes`), asi
  que el interprete, la 3060 y el x86 siguen viendo uno; la casa lo arma por
  dibujo (`cbuffers.rs`; con uno entero, sin copiar). Un cbuffer mas corto de
  lo que se lee da 0, no deja de dibujar. Probado con `prueba/cbuffers.dxil`.
  Queda: la fila DINAMICA (`cbuffer` con arrays indexados), que va con N5.4.
- [x] **N5.3 -- los SRV de bufer** (03-10): `Buffer<T>`, `StructuredBuffer`
  y `ByteAddressBuffer` en tablas. La especie de cada uno sale de la PSV0
  (`dxil/recursos.rs`), `bufferLoad` y su GetDimensions compilan
  (`Lectura::Bufer`), `bufer.rs` los lee con las reglas de D3D12 (fuera de
  la vista, 0) y la casa guarda en la ranura del SRV el paso, los elementos
  y si es crudo. Probado con `prueba/buferes.dxil`. Corren en la CPU (el
  emisor de la 3060 aun no los sabe: N6.1).
- [x] **N5.3b -- los SRV en la RAIZ** (05-10): `Set{Graphics,Compute}Root
  ShaderResourceView` y `...UnorderedAccessView` (y sus argumentos de
  ExecuteIndirect) guardan su direccion como un CBV de la raiz, y quien
  dibuja o despacha la lee hasta el final de su bufer
  (`tuberia::bufer_de_raiz`). El PASO del estructurado sale de los
  metadatos `dx.resources` del DXIL (`recursos::pasos_estructurados`,
  `Ranuras::paso`): probado con computo.dxil, nBodyGravity y
  ExecuteIndirect (16, 32 y 24). Juez: `prueba/vistas.cpp` (A).
- [~] **N5.3c -- los UAV** (05-10): en el COMPUTO ya, los de TEXTURA de una
  y dos dimensiones (`RWTexture2D`: `textureStore`, su lectura y
  `GetDimensions`, `bufer::Modo::Textura`), los `RWBuffer` con tipo de
  cualquier formato (`formato_ia::empaquetar`) y
  `ClearUnorderedAccessViewUint` y `...Float` (buferes y texturas). Juez:
  `prueba/vistas.cpp` (B, C y D), bit a bit; dice NO sin el paso, sin la
  limpieza y con la textura mal direccionada. Queda: los de textura 3D o
  de array, y la limpieza con rectangulos (hoy, la vista entera). Los UAV
  en el de PIXELES (y en el de vertices), hechos: N5.3d.
- [x] **N5.3d -- los UAV de un DIBUJO** (`platform/shared/proton-x/src/pruebas_uav.rs`,
  05-10; en el banco, falta el metal y Windows). Lo que un sombreador de
  pixeles o de vertices escribia en un UAV se PERDIA (y desde el 05-10 lo
  decia un aviso). Ahora el lote lleva sus UAV (`Lote::uavs`: los de la
  raiz y los de las tablas, buscados como los de un Dispatch,
  `computo::uav_de`) y el interprete los ve en un dibujo
  (`Extra::Uavs`, `Programa::correr_con_uavs`): `textureStore`,
  `bufferStore`, sus lecturas, `GetDimensions`, el contador, y los
  `Interlocked*` (`atomicBinOp` y `atomicCompareExchange`, nuevos: tambien
  en el computo; `bufer::Atomo`). La trama, con un sombreador de pixeles que
  toca UAV (`trama::Efectos`): corre CADA pixel cubierto, en su orden (sin
  la memoria del ultimo pixel, que se saltaba escrituras: un `InterlockedAdd`
  que no lee nada sumaba 1 en vez de 64), y la profundidad se prueba
  DESPUES (lo de D3D: un pixel tapado tambien escribe), salvo con
  `[earlydepthstencil]` (leido de las banderas de `dx.entryPoints`,
  `recursos::banderas`), que la prueba y la escribe ANTES. Un pixel que hace
  `discard` deja lo que escribio antes y nada de despues (lo de D3D). El
  codigo nativo no traduce un sombreador con UAV (se interpreta, con su
  aviso: "lee o escribe un UAV"); la puerta de la 3060 manda esos lotes a
  la CPU y lo dice una vez. **Como se sabe:** `prueba/uavpixel.exe`
  (nuestro, `uavpixel.cpp`): A, cada pixel su posicion en SU texel de un
  `RWTexture2D<uint>` de una tabla; B, `InterlockedAdd` de cada pixel de una
  caja de 32 x 16 en un `RWByteAddressBuffer` de la RAIZ (512); C, el de
  vertices escribe 100..105 en un `RWBuffer<uint>`. Dice `bien` 4 veces;
  con la casa de antes, A, B y C salen MAL. Y `src/pruebas_uav.rs`: la
  cuenta con la Z detras (64) y con `[earlydepthstencil]` (0).
  **Lo que puede fallar, dicho:** el orden entre pixeles de un dibujo aqui
  es el de la trama (triangulo a triangulo, fila a fila); D3D no da
  ninguno, asi que un juego que dependa de el (sin `Interlocked` ni
  `RasterizerOrderedView`) puede ver otra cosa que en la 3060; un ROV
  (`RasterizerOrdered*`) no se ha probado (aqui el orden ya seria el de
  las primitivas); un sombreador con UAV va interpretado (lento); la regla
  de "Z despues con UAV" es la de la especificacion de D3D11 y no se ha
  visto contra la 3060 todavia.
  **Queda:** los UAV de un sombreador de GEOMETRIA (lo dice un aviso: lo
  que escribe se pierde), el dibujo SOLO con UAV (sin render target ni Z:
  hoy "no hay donde dibujar"), los `Interlocked` traducidos a x86 y en la
  3060, y verlo en el Ryzen y en Windows.
- [ ] **N5.4 -- el indice dinamico** (`textures[i]`, bindless): el registro
  no es una constante. Hoy el sombreador no compila (y lo dice: "createHandle
  con un registro CALCULADO"); pide que la ranura sea un RANGO y no un
  lugar. El de los CBUFFERS ya (03-10): `CBufferLoadLegacy` con la fila
  calculada (`color[i]`, los arrays de luces y huesos) es
  `Op::ConstantesEn`, con las 4096 filas de D3D reservadas; probado con
  `prueba/luces.dxil`.
  **Las TEXTURAS ya (05-10, E2.2 de la ESCALERA):** un array de texturas
  con el registro calculado, con medida o sin ella (bindless), ya no es una
  ranura: es un RANGO dinamico (`Ranuras::dinamicas`, su espacio y su primer
  registro), y `Op::EligeTextura` escoge al correr la textura de su registro
  absoluto; la casa la busca en la root signature y el monton SOLO cuando un
  pixel la pide, y la guarda (un millon de descriptores no se recorren).
  Probado con `prueba/indice.dxil` y con DynamicIndexing de Microsoft (120
  materiales, uno por dibujo). Queda: los arrays de BUFERES, de cbuffers,
  de muestreadores y de UAV (lo dicen por su nombre), la 3060 (va por la
  CPU), y verlo en Cyberpunk.
- [x] **N5.6 -- la matematica que falta** (03-10): sin, cos, tan, exp2,
  log2, frac, los cuatro redondeos y los medios floats, en el interprete
  (`proton-x/src/mates.rs`, sin `libm`; contra la del anfitrion y los
  65.536 halfs ida y vuelta), probado con `prueba/mates.dxil`. En la 3060
  (MUFU) todavia no: esos sombreadores van por la CPU (N6.1).
- [x] **N5.7 -- discard** (03-10): `Op::Descarta` (DXIL 82, y
  `discard_nz`/`discard_z` de SM5; `clip()` llega como uno de ellos). El
  interprete acaba ahi y dice que el pixel NO queda; la trama no escribe
  ni color ni profundidad (la Z va despues del sombreador) y lo cuenta en
  `Cuenta::tirados`. Probado con `prueba/descarte.dxil` y un SM5 hecho a
  mano. En la 3060 (KILL) todavia no: va por la CPU (N6.1).
- [x] **N5.8 -- mas de un render target** (03-10, hasta 8): el G-buffer
  de Cyberpunk. El PSO lee los 8 `RTVFormats` y la mezcla de cada uno (con
  IndependentBlendEnable); `OMSetRenderTargets` guarda los N (consecutivos
  o en array; uno nulo, lo suyo se pierde); el enlace dice a que render
  target va cada salida (`Enlace::objetivos`, por su SV_Target) y la trama
  pinta cada una en el suyo (`Destino::otros`). Probado con
  `prueba/gbuffer.dxil` (SV_Target 0, 1 y 3) en el banco y por las puertas
  de Windows (`proton-x-casa/tests/gbuffer.rs`). En la 3060 todavia uno:
  el G-buffer va por la CPU (N6.1).
- [x] **N5.11 -- la MEZCLA** (03-10): la luz que se suma, las
  particulas, el humo, el cristal, la interfaz. `proton-x/src/mezcla.rs`:
  los factores, las cinco operaciones, color y alfa por separado y la
  mascara por canal; cada render target la suya (IndependentBlendEnable) y
  el factor de `OMSetBlendFactor`. La trama guarda en su memoria el color
  SIN mezclar y mezcla en cada pixel con el que esta. Falta: la operacion
  logica y las dos fuentes (SRC1), que se dicen; sRGB se mezcla en sus
  bytes. En la 3060 todavia no: por la CPU (N6.1).
- [x] **N5.12 -- solo PROFUNDIDAD** (03-10): los mapas de sombras y el
  prepaso de Z. Un PSO con `NumRenderTargets` 0 y sin sombreador de
  pixeles se crea (`lote::enlazar_con`, el de pixeles `Programa::vacio`);
  el Draw pinta en el DSV, con la medida de la Z, y la trama solo escribe Z
  (`Destino::pixeles` vacio). Probado por las puertas de Windows: un mapa
  de sombras D32 (`tests/gbuffer.rs`). La 3060 no lo toma todavia (sin
  back buffer que darle a la puerta): por la CPU.
- [x] **N5.12b -- el STENCIL** (05-10, `proton-x/src/stencil.rs`, `prueba/stencil.exe`):
  las mascaras de luz y las calcomanias de Cyberpunk. Antes el PSO lo
  apuntaba y no lo usaba (un aviso), y un juego que recorta con stencil
  pintaba de mas. Ahora, con las reglas de D3D12: StencilEnable, las
  mascaras de lectura y escritura, las dos caras (la del giro del
  triangulo) con sus tres operaciones (fallo, fallo de Z, pasa) y su
  funcion; las ocho operaciones; la referencia de `OMSetStencilRef` (y
  `OMSetFrontAndBackStencilRef`, una por cara), enmascarada para comparar
  y entera para REPLACE; la prueba con la de Z (antes del de pixeles, y la
  de Z despues si hay SV_Depth), tambien en los dibujos de solo
  profundidad; un pixel TIRADO no cambia el stencil. El plano: un byte por
  texel DETRAS de la profundidad en D24S8, D32S8X24 y sus TYPELESS
  (`proton-x-casa/src/d3d12_stencil.rs`); ClearDepthStencilView con
  CLEAR_FLAG_STENCIL (y BeginRenderPass) lo limpia. Juez: `stencil.exe`
  (`tests/corre/muestras.rs`: 5 `bien`, ningun aviso; con la casa de antes
  A, B, C y D salen MAL) y las pruebas de `stencil.rs` en la trama.
  **Lo que puede fallar, dicho:** (1) la 3060 no lo sabe: un lote con
  stencil va por la CPU (la puerta lo dice una vez), y si ademas usa Z, la
  de la 3060 deja de estar viva (como con cualquier lote de Z por la CPU);
  (2) el juez lo LEE por el color: leer el plano 1 con CopyTextureRegion no
  esta (su huella R8 no se ha medido contra Windows; CopyResource si lo
  copia entero); (3) SV_StencilRef (el de pixeles da la referencia) no: la casa
  anuncia `PSSpecifiedStencilRefSupported` NO; (4) el subobjeto
  DEPTH_STENCIL2 de un flujo (el 26, que la casa leia como RASTERIZER1,
  que es el 27: el flujo se torcia) ya se lee, pero con mascaras DISTINTAS
  en cada cara el PSO no se crea (lo dice).
  **Queda:** AlphaToCoverage, que con `SampleDesc.Count` 1 no cubre nada
  (sin MSAA no hay muestras que tapar): sigue apuntado y dicho.
- [x] **N5.13 -- las INSTANCIAS** (05-10, con N5.14): el follaje, la
  gente, los coches. Antes un `DrawInstanced` dibujaba UNA y solo se leia la ranura 0. Ahora: las 16
  ranuras de `IASetVertexBuffers` (y quitarlas con NULL), los elementos POR
  INSTANCIA con su `InstanceDataStepRate` (0: todas leen el primero),
  `StartInstanceLocation` (mueve lo que se lee por instancia;
  SV_InstanceID cuenta desde 0), los argumentos de ExecuteIndirect y los
  bundles, y dibujar SIN bufer de vertices (los que solo leen
  SV_VertexID: el triangulo de pantalla completa del post-proceso). En el
  crate, `lote::Flujo` y `Lote::instancias`; en la casa, `Estado::vertices`
  con sus 16. Juez: `prueba/instancias.cpp` (NUESTRO, de consola), en el
  banco (`tests/corre/muestras.rs`) pixel a pixel; probado que dice NO sin
  el bucle de instancias y sin el StepRate. Queda: la 3060 (esos lotes van
  por la CPU). Las cuentas enteras de SV_InstanceID ya las traduce a
  x86-64 el de los dibujos (05-10, la VELOCIDAD: ESCALERA seccion 5):
  `instancias.exe` ya no dice ni un aviso.
- [x] **N5.14 -- los buferes de vertices de mas de una ranura** (05-10):
  las 16, con N5.13 (arriba), y el mismo juez.
- [x] **N5.15 -- el RECORTE** contra el plano cercano y el lejano (05-10):
  la trama recorta (Sutherland-Hodgman, `trama::recortar`) lo que cruza
  un plano y pinta lo que queda, en abanico y en su sitio; los atributos,
  en linea recta en el espacio de recorte (la perspectiva sale igual), y
  una banda de guarda de 64 pantallas para x e y. Antes no se pintaba
  (en 3D de cerca faltaba suelo y pared; nBodyGravity lo decia). Juez:
  `proton-x/src/pruebas.rs` (los mismos pixeles que recortado a mano, y en
  cada pixel el atributo de la cuenta en f64 del triangulo ENTERO).
  `DepthClipEnable = FALSE` (sin recorte en z, la Z sujeta) ya no recorta
  igual: hecho con N5.16b (abajo, `flotante1.exe`, C).
- [x] **N5.16 -- render targets de floats de 2 a 4 canales** (05-10, `prueba/hdr.exe`):
  RGBA16F, RG16F, RGBA32F, R11G11B10F y R10G10B10A2 se guardan en FLOAT
  (cuatro palabras por texel, `Almacen::Flotantes4`), se mezclan en float
  y se cuantizan al formato de su vista (half, float11/10 con redondeo al
  par, sin signo a 0); ClearRenderTargetView en float; se copian y se
  leen como textura; y una cadena RGBA16F (scRGB lineal, llevada a sRGB) o
  R10G10B10A2 se presenta en la ventana de 8 bits. Juez: `hdr.exe`
  (`tests/corre/muestras.rs`, n5_16: 4 `bien`, bit a bit; con la casa de
  antes sale MAL: el 1.0 recortado) y `dxgi::pruebas_hdr`.
  **Lo que puede fallar, dicho:** (1) un monitor HDR no existe aqui: lo de
  mas de 1 se RECORTA al presentar, sin mapeo de tonos (un juego que confie
  en el monitor se vera quemado); (2) la 3060 no pinta en float: su puerta
  es de 8 bits, y un lote en float va por la CPU (lo dice un aviso); (3)
  cuatro palabras por texel son 4 veces la memoria de un RGBA8 (un RGBA16F
  de 1080p son 33 MB, no 8).
- [x] **N5.16b -- lo que quedaba de los floats, y DepthClipEnable = FALSE** (05-10, `prueba/flotante1.exe`):
  (1) un render target de UN float: R32_FLOAT (y la vista R16_FLOAT de un
  R16_TYPELESS) se guarda en UNA palabra por texel (`Almacen::Flotante`,
  la de la profundidad), se mezcla en float y se cuantiza a su vista
  (`tuberia::como_se_pinta`; la trama sabe cuantas palabras por lo que
  mide el destino); ClearRenderTargetView cuantizado a la vista; la copia
  y el SRV ya eran exactos. Un R16_FLOAT de verdad ya era float desde
  N5.16 (cuatro palabras). (2) Los UAV de una textura de float de 2-4
  canales: `bufer::CUATRO_FLOATS` (un bit del formato de la vista): el CS
  lee los cuatro f32 tal cual y escribe cuantizado al formato de la vista;
  ClearUnorderedAccessViewFloat/Uint en ese formato. (3) `DepthClipEnable
  = FALSE`: el PSO lo lee (`RasterizerState` +24) y llega a la trama como
  el bit `trama::SIN_RECORTE_Z` del descarte: sin recorte contra el plano
  cercano ni el lejano (la banda de x e y sigue), y la Z SUJETA al
  viewport [MinDepth, MaxDepth] antes de la prueba. Juez: `flotante1.exe`
  (`tests/corre/muestras.rs`, n5_16b: 10 `bien`, bit a bit; con la casa de
  antes, 6 MAL) y dos pruebas de la trama (`pruebas_pixeles.rs`) y una del
  UAV (`bufer.rs`).
  **Lo que puede fallar, dicho:** (1) la 3060 no pinta en float ni sin
  recorte en z: esos lotes van por la CPU (lo dice la puerta, una vez);
  (2) un render target R32_UINT/R32_SINT (o un UAV R32_UINT sobre un
  R11G11B10F, el truco de leer con tipo) sigue en "todavia no", con su
  aviso; (3) la vista R16_FLOAT de un R16_TYPELESS se pinta, pero leerla de
  vuelta con una copia aun no (la casa no sabe si sus floats son un D16 o
  un half); (4) SV_Position.z en el de pixeles, sin recorte, va SIN sujetar
  (como el FragCoord de Vulkan con depthClamp; si D3D lo sujetara, un
  sombreador que la lea veria otra cosa).
  **Queda:** la 3060 pintando en float y sin recorte en z (su puerta es de
  8 bits y su receta recorta siempre).
- [x] **N5.17 -- ExecuteIndirect y ExecuteBundle** (05-10). ExecuteBundle
  corre desde E1.6 de la ESCALERA (HelloBundles, bit a bit), y
  ExecuteIndirect desde E2.4 (D3D12ExecuteIndirect): se apunta con el
  estado de ese momento (el de computo si su firma despacha) y se resuelve
  al ejecutar la lista, leyendo entonces su cuenta y sus argumentos (DRAW,
  DRAW_INDEXED, DISPATCH, CBV, constantes y las vistas de vertices e
  indices), con los cambios de estado vivos para las ordenes de detras. Y
  el contador de un UAV (`Append`, `Consume`, `IncrementCounter`). Queda:
  los SRV/UAV en la raiz (N5.3b), los rayos y la malla (se dicen y se
  para), y verlo en Cyberpunk.

El ABI entero (que hace cada hueco de las 28 interfaces, que es falla
documentada y que falta) esta en `docs/maestro/D3D12_MAESTRO.md`, y lo
escribe una prueba (`proton-x-casa/tests/abi.rs`): 465 huecos, 0 faltan.
La ESCALERA de juegos (por que DX9 y Left 4 Dead 2 no) esta en su seccion
5; desde el 05-10, con sus casillas, en
[`PLAN_LA_ESCALERA_PROTON_X.md`](PLAN_LA_ESCALERA_PROTON_X.md).
- [x] **N5.9 -- SV_Position en el de pixeles** (03-10): `Enlace::pos_ps`
  dice que entrada es; la trama pone en ella (x + 0.5, y + 0.5, z, w) de
  cada pixel (la w de recorte, con perspectiva: la de D3D, no la 1/w de
  GL). Probado con `prueba/posicion.dxil` en un cuadro de 8x8. En la 3060
  todavia no (`NoVa::Entrada`): por la CPU (N6.1).
- [x] **N5.10 -- arrays locales y lo de las olas** (03-10). Los ARRAYS
  (`dxil/arreglos.rs`): `alloca`, `getelementptr` (indice constante o
  calculado; los de dos dimensiones, aplanados), `load`/`store` como
  `Op::LeeIndexado`/`EscribeIndexado`, y las tablas GLOBALES constantes
  (`static const float x[] = {...}`, float o int); probado con
  `prueba/arreglos.dxil`. Las OLAS ya (03-10, `dxil/olas.rs`): con un pixel
  por ola, ReadLaneFirst/At, ActiveOp, AnyTrue/AllTrue, el prefijo, los
  carriles y la cuenta de bits dan lo de un carril; y las derivadas (ddx,
  ddy, fwidth) dan 0 hasta que la trama corra cuadros de 2x2. Y SV_Depth:
  el de pixeles que escribe su Z (la prueba va despues de el). Probado con
  `prueba/olas.dxil` y `prueba/profundidad.dxil`.
- [ ] **N5.5 -- el COMPUTO** (`Dispatch`, `SetComputeRoot*`): hoy se dice
  y se salta. Cyberpunk calcula con el la luz, las sombras y el
  post-proceso; sin el, la imagen sale pero a medias. Primero en la CPU
  (el mismo interprete, con UAV de N5.3c y `SV_DispatchThreadID`), luego
  en la 3060.
  **En la CPU ya (05-10, E2.3a de la ESCALERA):** el CS se compila al crear
  el PSO (`dxil::computo::preparar`; `numthreads` de la PSV0), y `Dispatch`
  se apunta en la lista con el estado de COMPUTO (su raiz es otra que la
  de dibujo: `SetComputeRootSignature`, tablas, CBV y constantes) y corre
  al ejecutarla: grupo a grupo, cada hilo hasta su BARRERA
  (`GroupMemoryBarrierWithGroupSync`: el interprete se para y sigue), con
  la memoria COMPARTIDA del grupo (`groupshared`, addrspace 3), los cuatro
  ids del hilo y los UAV de BUFER de las tablas (`bufferStore` y
  `bufferLoad`: estructurados, crudos y tipados de 32 bits por canal). Una
  cola de computo y la valla entre colas, tambien. Probado con
  `prueba/computo.dxil` en el crate y con `computo.exe` en el banco, bit a
  bit. Queda: los UAV de TEXTURA y
  `ClearUnorderedAccessView` (N5.3c), las vistas en la RAIZ (N5.3b), las
  atomicas, y la 3060 (el emisor no lo sabe: va por la CPU, y cada grupo
  en un hilo es lo siguiente de EXPRIMIR).
  **Y TRADUCIDO a x86-64 (05-10, E2.3b):** `bmo_proton_x::nativo_computo`
  traduce el CS una vez al crear el PSO (saltos, enteros, la compartida,
  los buferes, y la barrera como un punto donde la funcion vuelve y por
  donde sigue); 50 veces el interprete. Con el, D3D12nBodyGravity (10.000
  particulas, la barrera dentro de un bucle) corre en el banco. Su juez
  (`proton-x-casa/tests/nativo_computo.rs`): el interprete bit a bit, y la
  fisica en f64. Lo que no traduce (texturas, `mates.rs`, buferes tipados,
  cbuffers con fila calculada) va por el interprete, que da lo mismo.
- [x] **N5.18 -- el sombreador de GEOMETRIA** (05-10, E2.3b de la
  ESCALERA): el GS de nBodyGravity hace de cada punto un cuadro. En el
  crate: `EntradaDe` (el elemento de un vertice de la primitiva), `Emite` y
  `Corta`, lo de su PSV0 (primitiva de entrada, topologia de salida,
  `maxvertexcount`), el enlace VS -> GS -> PS (`lote::enlazar_con_gs`) y
  las primitivas de un lote (`lote::primitivas`); en la casa, el PSO con
  GS y los puntos y lineas (solo con GS: la trama pinta triangulos).
  Probado con los tres de nBodyGravity (`pruebas_geometria.rs`, contra la
  geometria del cuadro y su degradado; dice NO si el GS lee mal) y con la
  muestra entera. Queda: un vertice CALCULADO (`input[i]` con `i` en un
  registro), la adyacencia, emitir puntos o lineas, varios flujos y el
  stream output, `SV_PrimitiveID` y las instancias de GS, y la 3060 (va por
  la CPU).
- [ ] **N6.1 -- a la 3060 lo que hoy va a la CPU**: SV_VertexID y
  SV_InstanceID, los formatos de vertice que no son float de 32 bits
  (`proton-x-sm86/src/pso.rs`, `NoVa::Entrada`: el pegamento los convierte
  antes de que corra el sombreador), y las lecturas que el emisor no sabe
  (`Op::Lee`: Load, SampleLevel, arrays, cubos, buferes).
- [ ] **N6.2 -- ExecuteCommandLists y Present en el metal**: el primer
  fotograma del juego. Hasta la segunda corrida: 0 y 0.
- [ ] **N6.3 -- el juez en el metal**: cada PSO que va a la 3060, comparado
  con la CPU la primera vez (lo que ya hace con el cubo); lo que no casa,
  a la CPU y dicho.

**El nivel 9 es posible por ser el kernel propio**, y no es un deseo: estas
son las palancas que ningun juego tiene en Windows, cada una medible con
FRAPS-X antes y despues:

- [ ] **N9.1 -- solo el juego**: con el corriendo, el kernel no hace nada
  mas que el bus USB y el disco (el escritorio, apartado; ningun servicio de
  fondo que robe un nucleo).
- [ ] **N9.2 -- paginas grandes**: la memoria del juego en paginas de 2 MiB
  (y de 1 GiB, que el Ryzen tiene sin usar): menos fallos de TLB.
- [ ] **N9.3 -- los hilos del juego en su nucleo**: SetThreadIdealProcessor
  y las prioridades que pide, respetadas de verdad, con el contrato de
  tiempo de cada hilo (el COMPAS) vigilado.
- [ ] **N9.4 -- la 3060 sin capas**: los lotes van del juego a la puerta de
  la 3060 sin driver de por medio; y a P0 mientras se juega (E-1 de
  [`PLAN_LA_3060.md`](PLAN_LA_3060.md) la duerme cuando no).

**Y despues, GOG de verdad**: con el juego corriendo, que la LUDOTECA
lo traiga de la cuenta de GOG del propietario, autentico y sin DRM que
saltar (J2 de [`PLAN_LA_LUDOTECA.md`](PLAN_LA_LUDOTECA.md): la antena pide
la lista a GOG, en Rust), y lo instale y lo arranque desde BMO-X.

**"Todo el potencial", dicho claro**: el trazado de rayos (RT Overdrive) pide
los nucleos RT de la 3060 desde nuestro propio emisor, y va al final, detras
de "se ve el menu" (seccion 0). DLSS es una biblioteca cerrada de NVIDIA
(`nvngx`) que habla con SU driver: aqui no hay ese driver, asi que no. FSR 3
si: lo trae el juego en sus DLL (`ffx_*.dll`, ya cargadas el 02-10) y corre
en cualquier grafica.
