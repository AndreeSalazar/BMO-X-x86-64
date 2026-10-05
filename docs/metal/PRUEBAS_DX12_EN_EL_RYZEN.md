# LAS PRUEBAS DE DX12 EN EL RYZEN -- la hoja para el metal (05-10)

El propietario: *"anotar que test REAL para que mi Ryzen 5 5600X ... pueda
aplicarse ... y luego en CPU lo testeo"*. Todo lo de PROTON-X del 05-10 paso
en el banco del anfitrion (`platform/shared/proton-x-casa/tests/corre/muestras.rs`)
y NADA en el metal. Esta hoja dice, prueba a prueba, que correr en BMO-X, que
tiene que salir, y que mandar si no sale. Cada juez dice lo mismo en Windows:
si en el Ryzen sale distinto, la diferencia es de BMO-X.

## 0. Antes de empezar

- El build entero (`.\bmo.ps1 -Paralelo -Desplegar ...` o como lo corras):
  la ventana VERIFICAR tiene que decir BIEN. Desde el 05-10 un `[X]` PARA de
  verdad (antes seguia de largo y decia BIEN igual).
- Los `.exe` nuestros los copia el build a `window\` del volumen de datos
  (la lista de `Ultra_kernel_x86-64/build/ejemplos.ps1`).
- **Primero la CPU.** Estas pruebas no necesitan la 3060: los dibujos que
  la puerta no sabe hacer van por la CPU, y lo dicen. Si la 3060 esta
  despierta, puede llevarse los dibujos de 8 bits: el resultado tiene que
  ser EL MISMO.
- En cada prueba: una foto de la consola, o el `informe/` que deje.

## 1. Los jueces de consola (dicen `bien` / `MAL`, y salen con los fallos)

Se lanzan con `run sys/proton-x.bex window/<nombre>.exe`. Lo que tiene que
salir es lo mismo que pide el banco: el numero de `bien`, la ultima linea, y
NINGUN `MAL`. Los avisos `PROTON-X:` que se esperan estan dichos; otro
cualquiera es una diferencia.

| .exe | que juzga | `bien` | ultima linea | avisos que SI se esperan |
|---|---|---|---|---|
| `computo.exe` | el computo: memoria compartida y barrera (E2.3a) | 4 | `computo.exe: el computo de D3D12 es el de Windows` | ninguno |
| `instancias.exe` | instancias, tres ranuras de vertices, dibujar sin bufer (N5.13) | 3 | `instancias.exe: las instancias de D3D12 son las de Windows` | uno: `... cuentas ENTERAS ...: sus sombreadores se interpretan` |
| `vistas.exe` | vistas en la raiz, UAV de textura, RWBuffer con tipo, ClearUAV (N5.3b/c) | 7 | `vistas.exe: las vistas de D3D12 son las de Windows` | ninguno |
| `hdr.exe` | render targets de float: RGBA16F, R11G11B10F, leidos como textura (N5.16) | 4 | `hdr.exe: los render targets de float son los de Windows` | uno: `... cuentas ENTERAS ...` |
| `uavpixel.exe` | UAV escritos desde un DIBUJO: textura por pixel, `InterlockedAdd`, `RWBuffer` desde el de vertices (N5.3d) | 4 | `uavpixel.exe: los UAV de un dibujo son los de Windows` | dos: `... cuentas ENTERAS ...` y, de la puerta de la 3060, `PROTON-X: este lote va por la CPU: sus sombreadores leen o escriben UAV ...` (una vez) |

**Lo que el banco NO puede ver y el Ryzen si** (por eso cuentan):

- El codigo TRADUCIDO a x86 de verdad: en el banco corre en el anfitrion
  (Linux); en el Ryzen, en Ring 3 de BMO-X, con su soft-float y su pila.
- La memoria: `hdr.exe` pide tres destinos de 64 x 64 (el de float son 16
  bytes por texel); un juego de verdad pide cientos de MB.
- El tiempo: en el banco nadie mide; en el Ryzen, si tarda mas de unos
  segundos, anotalo (con la orden `pulso` si la tienes a mano).

**Si sale `MAL`:** la linea dice que texel y que bits salieron contra los
que tenian que salir. Esa linea entera es lo que hace falta.

## 2. Las muestras de Microsoft (necesitan sus `.cso`)

Leen sus sombreadores de ficheros de nombre LARGO (`shaders_VSMain.cso`), y
el FAT32 de BMO-X solo encuentra nombres 8.3
(`docs/plan/PLAN_LA_ESCALERA_PROTON_X.md` 7.1). Hasta que eso se decida: la
carpeta de cada muestra en `D:` (NTFS), con su `.exe` y sus `.cso` juntos,
como salen de `platform/shared/proton-x/prueba/muestras/construir.sh`.

| muestra | que tiene que verse |
|---|---|
| HelloWindow (`hwindow.exe`, en `window\`) | la ventana entera AZUL `{0, 0.2, 0.4}` cada fotograma |
| HelloTriangle | el triangulo de colores (rojo, verde, azul en las esquinas) sobre el azul |
| HelloFrameBuffering, HelloBundles | el MISMO triangulo, igual que HelloTriangle |
| HelloConstBuffers | el triangulo moviendose de izquierda a derecha |
| HelloTexture | el triangulo con un tablero de damas, pixeles duros (sin mezclar) |
| DynamicIndexing | una ciudad de 15 x 8 copias, cada una con su material |
| nBodyGravity | dos nubes de particulas que se juntan |
| ExecuteIndirect | 1024 triangulos que entran desde la izquierda; DENTRO de la franja central se ve lo mismo con culling y con ESPACIO pulsado (sin culling) |
| PredicationQueries | el cuadro blanco lejano NO se dibuja en el fotograma que sigue a uno en que el translucido cercano lo tapaba ENTERO |

## 3. La 3060 (despues, con el GSP)

El propietario: *"la GPU creo que voy a darle un buen apagado y que me
responda mi BMO-X"*. Es justo la prueba que falta del `0x15`
(`docs/plan/PLAN_LA_3060.md`, G0, paso 1 de W2), y nunca se hizo entera:

1. En Windows, como administrador: `powercfg /h off` (quita el inicio
   rapido: con el, "apagar" NO apaga la tarjeta).
2. En la BIOS: `ErP Ready` en `Enabled`.
3. De Windows a BMO-X: **APAGAR** (no Reiniciar), esperar 15-30 s, encender.
4. En BMO-X, `save mode` como siempre, y apuntar la fila `despierto` de
   `cabina` (el GSP-RM arriba, o el `0x15` con su autopsia).

**Como se sabe:** diez arranques seguidos, viniendo de Windows y de BMO-X,
con la fila `despierto` de cada uno. Si sale 10 de 10, el cable deja de
hacer falta; si no, las filas dicen que cambio entre los buenos y los malos.

Con el GSP arriba: `run sys/proton-x.bex window/bmox12.exe` tiene que
decir que la 3060 pago (y los fps); los jueces de la seccion 1 tienen que
decir lo mismo que por la CPU.
