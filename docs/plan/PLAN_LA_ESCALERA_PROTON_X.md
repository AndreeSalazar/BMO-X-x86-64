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
   R2  EL .EXE DE VERDAD. Compilado del fuente publico SIN cambios, salvo el
       parche de la huella (R3), que se guarda como .patch junto al binario.
       Nada de "su camino llamado a mano": eso es lo que ya hay
   R3  EL JUEZ. El .exe dice la huella del fotograma N (FNV-1a de B, G, R,
       como `bmo_cubo::referencia::huella` y `cubo12.exe`). En Windows, con
       la 3060 y su driver, y en BMO-X tiene que decir LO MISMO:
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
   R7  DE DONDE SALE CADA BINARIO. Compilado con MSVC, `/MD` (el mismo CRT
       que Cyberpunk: vcruntime140, msvcp140, api-ms-win-crt-*), `/Brepro` y
       sin la ruta del PDB de la maquina (`procedencia --check` lo caza).
       Junto al binario: su LICENSE (MIT), su .patch y un HACER.txt con el
       commit del repo de origen, la version de MSVC, las banderas y la
       huella que dio en Windows
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

- [ ] **E1.0 -- la cadena de fabricacion.** Clonar
  `microsoft/DirectX-Graphics-Samples`, compilar en Windows con R7, aplicar
  el parche de la huella (R3) y dejar cada `.exe` en
  `platform/shared/proton-x/prueba/muestras/` con su LICENSE, su .patch y su
  HACER.txt; `rayosx` sobre cada uno (DENTRO, y su lista de importaciones).
  Sus sombreadores, con `sombras.exe` en el mismo Windows: el `D3DCompile`
  de la casa no compila HLSL, sirve el `.cso` de su huella (P3c2,
  `proton-x-casa/src/compilador.rs`). Y su entrada en el NOTICE de la raiz. **Como se sabe:** los `.exe` en el
  arbol, `procedencia --check` en verde, y la huella de Windows de cada uno
  apuntada en su HACER.txt.
- [ ] **E1.1 -- HelloWindow.** Solo la cadena de intercambio, un Clear, una
  valla y Present. La capa: la cadena de un `.exe` de Microsoft (D5.7 de
  LAS_TRES_GRANDES, a medias). **Como se sabe:** su huella, igual.
- [ ] **E1.2 -- HelloTriangle, el `.exe` de verdad.** Lo que el cubo no
  pide: `d3dx12.h`, ComPtr, el CRT de C++ de MSVC y el HLSL compilado en
  marcha con `D3DCompileFromFile` (SM5; la casa da su `.cso`, E1.0). **Como
  se sabe:** su huella, igual.
- [ ] **E1.3 -- HelloTexture, el `.exe` de verdad.** Hoy solo el camino a
  mano (`tests/textura.rs`). **Como se sabe:** su huella, igual (el
  muestreador es de PUNTO: bit a bit).
- [ ] **E1.4 -- HelloConstBuffers.** Un cbuffer en un monton UPLOAD con `Map`
  PERSISTENTE que el `.exe` escribe en cada fotograma (D2.3). **Como se
  sabe:** las huellas de dos fotogramas distintos, iguales a las de Windows.
- [ ] **E1.5 -- HelloFrameBuffering.** Varios fotogramas en vuelo, una valla
  por fotograma (D5.2). **Como se sabe:** su huella, y el pulso dice que no
  se espera de mas (ni un hilo parado en una valla que ya llego).
- [ ] **E1.6 -- HelloBundles.** `ExecuteBundle` (la mitad de N5.17).
  **Como se sabe:** su huella, igual, y el aviso de "se salta" ya no sale.

## E2 -- una casilla abierta, una muestra (`Samples/Desktop/`, MIT)

- [ ] **E2.1 -- D3D12Multithreading.** Listas de ordenes grabadas desde
  varios hilos (H2.7), con sus mapas de sombras (N5.12, hecho). **Como se
  sabe:** su huella, igual, con los hilos cooperativos de hoy; y otra vez
  cuando H1 lleve los hilos a varios nucleos.
- [ ] **E2.2 -- D3D12DynamicIndexing.** El indice dinamico de descriptores
  (N5.4, el bindless). **Como se sabe:** su huella, igual, y el diario ya no
  dice "un operando que deberia ser un entero constante".
- [ ] **E2.3 -- D3D12nBodyGravity.** El COMPUTO (N5.5), los UAV (N5.3c), la
  cola de computo y la valla entre colas (D5.1, D5.2). Por confirmar con su
  fuente antes de empezar: si dibuja las particulas con un geometry shader;
  si la casa no lo tiene, es una casilla mas, y se dice aqui. **Como se
  sabe:** su huella tras N pasos, con su margen (R3): son floats sumados en
  otro orden que la 3060, y eso no da los mismos bits.
- [ ] **E2.4 -- D3D12ExecuteIndirect.** `ExecuteIndirect` (la otra mitad de
  N5.17) y su culling por computo (pide E2.3). **Como se sabe:** su huella,
  igual, con el culling encendido y apagado.
- [ ] **E2.5 -- D3D12SM6WaveIntrinsics.** Las olas de verdad (D4.3): hoy son
  "un pixel por ola" (`dxil/olas.rs`). **Como se sabe:** su huella, igual.
- [ ] **E2.6 -- D3D12HDR.** Render targets de float (N5.16) y la cadena en 10
  o 16 bits con su espacio de color. **Como se sabe:** su huella, igual, en
  el modo de 8 bits y en el de 16.
- [ ] **E2.7 -- D3D12PredicationQueries.** Consultas de oclusion y
  predicacion (D5.5). **Como se sabe:** su huella, igual.

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
   D5.2   vallas                            E1.5, E2.3
   N5.17  ExecuteBundle / ExecuteIndirect   E1.6, E2.4
   H2.7   listas desde varios hilos         E2.1
   N5.4   el indice dinamico (bindless)     E2.2
   N5.5   el COMPUTO                        E2.3
   N5.3c  los UAV                           E2.3
   D5.1   la cola de computo                E2.3
   D4.3   las olas de verdad                E2.5
   N5.16  render targets de float           E2.6
   D5.5   consultas                         E2.7
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

# 6. LO QUE ESTE PLAN NO CAMBIA

- Cyberpunk sigue siendo el NORTE de PROTON-X: este plan es el camino, no
  otro destino.
- Ni Wine, ni vkd3d-proton, ni su codigo dentro de la casa. Las muestras de
  Microsoft entran como `.exe` de PRUEBA (MIT, con su aviso), igual que las
  tandas: la casa no lleva ni una linea suya.
- Las tandas siguen: un muro que no cae en ningun escalon se prueba con una
  tanda, como hasta hoy.
