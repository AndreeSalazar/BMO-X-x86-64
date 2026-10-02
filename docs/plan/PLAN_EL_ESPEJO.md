# PLAN EL ESPEJO -- lo que Windows le contesta a Cyberpunk, apuntado en Windows

> Pedido por el propietario (2026-10-02): *"app en window para que PROTON
> hable con cyberpunk 2077 en window; mas rapido?"* y, con el nombre:
> *"ESPEJO me gusta"*.

---

## 0. La respuesta corta

Hoy PROTON-X trabaja a ciegas por un lado. En BMO-X el DIARIO dice que pide
el juego y que le contesta LA CASA; lo que contesta WINDOWS se deduce de la
documentacion y de las cabeceras, y cada diferencia se descubre cuando el
juego se cae: **una pared por vuelta al metal**.

ESPEJO es la otra mitad. Un programa para el Windows del propietario que
lanza el juego, apunta cada llamada a Windows con lo que entro y lo que
salio, y lo deja en el MISMO formato que el DIARIO. Se comparan los dos
ficheros y **la primera linea distinta es la proxima pared**, vista antes de
que el juego se caiga.

```text
   Windows del propietario              BMO-X (el metal)
   espejo.exe -> Cyberpunk2077.exe      proton-x -> Cyberpunk2077.exe
        |  ESPEJO.TXT                        |  DIARIO.TXT + SYSPROTO.TXT
        +---------------> espejo-compara <---+
                          "la primera diferencia: VirtualQuery(...)"
```

Ejemplo de lo que habria dicho el 02-10: la casa contestaba 0 a
`VirtualQuery` de una direccion de la imagen; Windows contesta 48 con
`MEM_IMAGE`. Eso costo tres vueltas al metal (tandas 38 y 39 y dos lupas de
la autopsia).

## 1. Las reglas (las de siempre, aqui tambien)

- **Codigo propio.** Nada de Wine/DXVK/vkd3d/ReactOS, y tampoco Detours ni
  MinHook: el enganche es una tabla de importaciones reescrita a mano, que
  son cuarenta lineas. Las cabeceras publicas de Windows sirven para las
  disposiciones (como en las tandas).
- **Solo mirar.** La version de GOG no tiene DRM: no se salta nada, no se
  cambia ningun fichero del juego, y el juego corre en Windows igual que
  siempre. ESPEJO no toca `D:` (BMO-X) ni la carpeta del juego: escribe en la
  suya.
- **Reproducible como una tanda.** Se compila aqui con clang + lld-link (los
  `.def` de `platform/shared/proton-x/prueba/`), con sha256 apuntado en un
  HACER, y lo prueba el propietario en su Windows.
- ASCII, castellano sin enes perdidas, un paso por commit, guardianes.

## 2. Las piezas

```text
   espejo.exe     el lanzador: CreateProcessW(SUSPENDED) del juego, mete
                  espejo.dll (VirtualAllocEx + WriteProcessMemory +
                  CreateRemoteThread(LoadLibraryW)), espera a que diga
                  "listo" y ResumeThread
   espejo.dll     al cargar: reescribe la IAT del .exe y de cada DLL del
                  juego (y de las que se carguen despues: LoadLibrary*
                  tambien pasa por el espejo) para que cada funcion
                  importada pase por un trampolin suyo
   el trampolin   apunta la llamada y salta a la de Windows; para las
                  "vigiladas", apunta tambien los argumentos y lo que
                  devolvio
   ESPEJO.TXT     el registro, al lado de espejo.exe
```

**El formato**, el del DIARIO (`proton-x-casa/src/diario.rs`, `CABECERA`),
con una cabecera propia y una columna mas para las vigiladas:

```text
   # ESPEJO de Windows: cada funcion, la PRIMERA vez que el .exe la llama
   # orden hilo dll funcion
       1     7 KERNEL32.dll GetSystemTimeAsFileTime
   ...
   # vigiladas: cada llamada, con lo que entro y lo que salio
   VirtualQuery(0x7ff6a0001000, 48) = 48 base 0x7ff6a0001000 region 0x7ff6a0000000 MEM_IMAGE MEM_COMMIT prot 0x20
```

El `hilo` es el orden de creacion (7 el principal, como PROTON-X da los
suyos), no el TID de Windows, para que los dos ficheros se comparen.

## 3. Los pasos

### [x] Paso 0a -- el enganche de la IAT, SIN inyectar (02-10)

La mitad segura primero: `espejo.dll` se carga en su PROPIO proceso y engancha
la IAT del `.exe` que la cargo. `tanda40.exe` la carga y llama a cinco
funciones conocidas; salen en ESPEJO.TXT en el formato del DIARIO (hilo 7, el
principal, como PROTON-X). Hecho y probado en Windows: las cinco en orden mas
`ExitProcess`. Reproducible (dos enlaces, misma huella); sha256 en HACER.txt.
El trampolin es el del DIARIO con salto ABSOLUTO (VirtualAlloc cae a mas de
2 GiB del modulo y un `jmp rel32` se pasaria).

### [ ] Paso 0b -- el lanzador que inyecta en el juego

`espejo.exe <ruta del juego>`: `CreateProcessW(SUSPENDED)`, mete `espejo.dll`
(VirtualAllocEx + WriteProcessMemory + CreateRemoteThread(LoadLibraryW)),
espera y `ResumeThread`. Es la inyeccion en un proceso de terceros, asi que va
en su propio commit, revisado aparte. El enganche (0a) ya esta probado; esto
solo lo lleva hasta Cyberpunk. Lo prueba el propietario con la tanda y luego
con el juego.

### [ ] Paso 1 -- las DLL del juego y las que llegan tarde

El enganche recorre las 24 DLL del juego (las del `mapa:` de SYSPROTO) y
`LoadLibraryExW/A` y `GetProcAddress` devuelven trampolines tambien (como
`diario::envolver`). Sin esto, Streamline (`sl.interposer.dll`) y Galaxy no
salen.

### [ ] Paso 2 -- las vigiladas, con sus datos

Una lista corta y a mano, la de las paredes de PROTON-X:
`VirtualQuery`, `VirtualAlloc`, `GetFileAttributesExW`, `FindFirstFileW`,
`CreateFileW`, `GetFileInformationByHandleEx`, `MapViewOfFile`,
`GetSystemInfo`, `GetLogicalProcessorInformationEx`,
`GetSystemCpuSetInformation`, `LoadLibraryExW`, `GetProcAddress` (el NULL y
el nombre), `CoCreateInstance` (el CLSID y el HRESULT). Cada una con su
formateador; tope de lineas por funcion para que el fichero no pese GB.

### [x] Paso 3 -- el comparador (02-10)

`espejo.py ESPEJO.TXT DIARIO.TXT` en `toolchain/tools/espejo/`, con `--check`
sobre `ejemplo_espejo.txt`/`ejemplo_diario.txt`: dice la primera funcion que
Windows ve y la casa no (la pared), las que la casa llama y Windows no, y
cuantas coinciden. Compara el `dll` sin mirar mayusculas (`kernel32.dll` =
`KERNEL32.dll`). Una pantalla, no un volcado. Las vigiladas (paso 2) quedan
para cuando existan. Hecho; el `--check` da la pared esperada.

### [ ] Paso 4 -- los objetos COM de DirectX

DXGI y D3D12 no pasan por la IAT: se llaman por su vtabla. Al salir
`CreateDXGIFactory*` / `D3D12CreateDevice`, ESPEJO envuelve la vtabla del
objeto devuelto (y de los que este crea) con trampolines por hueco, con los
nombres de `proton-x-casa/src/com.rs` (`M_*`). Es lo que ve la swap chain y
los shaders: las paredes que vienen.

## 4. Lo que NO es

- No es un depurador ni un trazador de todo: millones de llamadas por
  segundo no caben. Primera vez de cada funcion + una lista corta vigilada.
- No sustituye al metal: el juego tiene que correr en BMO-X. ESPEJO dice
  hacia donde mirar, no arregla nada.
- No va a BMO-X: es un `.exe` de Windows, como las tandas.
