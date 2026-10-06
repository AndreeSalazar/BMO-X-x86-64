# LAS PRUEBAS DE DX12 EN EL RYZEN -- la hoja para el metal (05-10)

El propietario: *"anotar que test REAL para que mi Ryzen 5 5600X ... pueda
aplicarse ... y luego en CPU lo testeo"*. Todo lo de PROTON-X del 05-10 paso
en el banco del anfitrion (`platform/shared/proton-x-casa/tests/corre/muestras.rs`)
y NADA en el metal. Esta hoja dice, prueba a prueba, que correr en BMO-X, que
tiene que salir, y que mandar si no sale. Cada juez dice lo mismo en Windows:
si en el Ryzen sale distinto, la diferencia es de BMO-X. **Antes**, la lista
de Windows (`PRUEBAS_EN_WINDOWS.md`, 06-10): que Windows diga lo que pide
cada juez es lo que hace que esta hoja cuente.

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

**La forma corta (06-10): `run sys/jueces.bex`.** Los 85 de consola, uno
tras otro y SOLOS: cada uno con su consola y un tope de 600 s, contados con
la MISMA tabla que uso Windows (`platform/shared/proton-x/prueba/jueces.txt`,
84 de 84 en la 3060 del propietario; `limpieza`, el 85, aun no). Dice una linea por juez (`bien`,
`DISTINTO` y por que, o `COLGADO`) y lo deja todo en `informe/jueces.txt`:
ese fichero es lo que hay que mandar. Un grupo solo: `run sys/jueces.bex
d3d12` (o `tandas`, `dentro`); unos pocos: `run sys/jueces.bex olas tipos`;
otro tope: `tope=900`. Los avisos `PROTON-X:` no cuentan: van al informe.
La tabla de abajo dice lo mismo, juez a juez, para leerla a mano.

Se lanzan con `run sys/proton-x.bex window/<nombre>.exe`. Lo que tiene que
salir es lo mismo que pide el banco: el numero de `bien`, la ultima linea, y
NINGUN `MAL`. Los avisos `PROTON-X:` que se esperan estan dichos; otro
cualquiera es una diferencia.

| .exe | que juzga | `bien` | ultima linea | avisos que SI se esperan |
|---|---|---|---|---|
| `computo.exe` | el computo: memoria compartida y barrera (E2.3a) | 4 | `computo.exe: el computo de D3D12 es el de Windows` | ninguno |
| `instancias.exe` | instancias, tres ranuras de vertices, dibujar sin bufer (N5.13) | 3 | `instancias.exe: las instancias de D3D12 son las de Windows` | ninguno (desde X1, 05-10: sus enteros van traducidos) |
| `vistas.exe` | vistas en la raiz, UAV de textura, RWBuffer con tipo, ClearUAV (N5.3b/c) | 7 | `vistas.exe: las vistas de D3D12 son las de Windows` | ninguno |
| `hdr.exe` | render targets de float: RGBA16F, R11G11B10F, leidos como textura (N5.16) | 4 | `hdr.exe: los render targets de float son los de Windows` | ninguno (desde X2, 06-10: su PSLee muestrea en x86)
| `uavpixel.exe` | UAV escritos desde un DIBUJO: textura por pixel, `InterlockedAdd`, `RWBuffer` desde el de vertices (N5.3d) | 4 | `uavpixel.exe: los UAV de un dibujo son los de Windows` | dos: `PROTON-X: un PSO cuyo sombreador lee o escribe un UAV: ...` y, de la puerta de la 3060, `PROTON-X: este lote va por la CPU: sus sombreadores leen o escriben UAV ...` (una vez) |
| `flotante1.exe` | render targets de UN float (R32F, R16F), UAV de un RGBA16F, DepthClipEnable = FALSE (N5.16b) | 10 | `flotante1.exe: los floats de un canal, sus UAV y DepthClipEnable son los de Windows` | ninguno (desde X2, 06-10)
| `stencil.exe` | el STENCIL: REPLACE/EQUAL, INCR_SAT y las mascaras, las dos caras, solo profundidad (N5.12b) | 5 | `stencil.exe: el stencil de D3D12 es el de Windows` | ninguno por la CPU; con la 3060 despierta, los de la puerta, una vez cada uno: `este lote va por la CPU: el lote usa STENCIL ...` y `... mezcla (o escribe solo algunos canales) ...` (las sondas) |
| `olas.exe` | las OLAS de 32 carriles (Wave*, Quad*): cada operacion de ola en un CS (dentro de un si y en dos bucles), y en un PS los cuadros de 2x2 con sus AYUDANTES; y OPTIONS1 dice 32 (E2.5); y SV_VertexID sin el vertice base (D y E, 06-10) | 17 | `olas.exe: las olas de D3D12 son las de Windows` | uno: `PROTON-X: un PSO cuyo sombreador usa las olas (Wave*, Quad*: van de 32 en 32 carriles): sus sombreadores se interpretan (el codigo nativo aun no lo sabe)`; con la 3060 despierta, ademas, una vez: `este lote va por la CPU: sus sombreadores usan las olas (Wave*, Quad*): la 3060 no las lleva todavia`. En Windows, en B, el `hasta N` de la segunda linea puede ser otro (como junte la GPU los cuadros) |
| `derivadas.exe` | las DERIVADAS (`ddx`/`ddy` finas y gruesas de un cuadro de 2x2) y la MIP que eligen los muestreos: Sample por gradientes, MIP_LINEAR, SampleBias/Level/Grad, CalculateLevelOfDetail, MostDetailedMip, ResourceMinLODClamp y MaxLOD (D4.4) | 17 | `derivadas.exe: las derivadas y la mip de un muestreo son las de Windows` | ninguno (desde X3, 06-10: los dos PSO van traducidos, en cuadros de 2x2); con la 3060 despierta, los de la puerta (los dos lotes por la CPU, y dice por que)
| `restos.exe` | render targets de ENTEROS (R32_UINT, R8_UINT, RGBA16_SINT...: limpiar, saturar, mascara, leer con Load), un dibujo SOLO con UAV, UAV desde un GS, el plano de STENCIL leido (CopyTextureRegion y SRV X24_G8) y SV_StencilRef | 18 | `restos.exe: los enteros, los UAV sin destino y del GS, el plano de stencil y SV_StencilRef son los de Windows` | uno: `PROTON-X: un PSO cuyo sombreador lee o escribe un UAV: ...` (desde X2, 06-10, los que leen texturas van en x86). NINGUNA linea `nota`: la casa dice que SV_StencilRef si. En Windows, si la GPU no lo tuviera, E sale como `nota` (no es fallo)
| `multihilo.exe` | listas grabadas desde VARIOS HILOS a la vez (cuatro, en el mismo destino y en un pase de solo Z), colas de computo y de copia que ESPERAN a una valla (`Wait`), SetEventOnCompletion sin evento, SetEventOnMultipleFenceCompletion, y las reglas de Reset/Close (E2.1) | 17 | `multihilo.exe: las listas de varios hilos y las colas con vallas son las de Windows` | cuatro, los errores que C hace A PROPOSITO: `... un allocator con el que ya graba otra: en Windows es E_INVALIDARG`, `Reset de un allocator con una lista grabando ...`, `Reset de una lista que no se cerro ...` y `Close de una lista ya cerrada ...`. Si se CUELGA, es B (una cola que espera y nadie la despierta): la foto de la consola dice en cual |
| `volumen.exe` | los UAV de texturas 3D y de ARRAYS escritos por computo: un 3D entero, una vista de dos rebanadas (lo de fuera no se escribe), un array de 3 capas con 2 mips por la vista de la mip 1 de dos capas, InterlockedAdd en un 3D, GetDimensions y lecturas, y ClearUnorderedAccessViewUint de dos rebanadas o dos capas (06-10) | 10 | `volumen.exe: los UAV de texturas 3D y de arrays son los de Windows` | ninguno |
| `firmas.exe` | las ROOT SIGNATURES 1.1 (la de `dxc`, con banderas) y 1.0, la que viene DENTRO del sombreador (pasada a CreateRootSignature, y la de un PSO creado sin root signature), una DESC1 serializada, y CheckFeatureSupport diciendo 1.1 (06-10) | 6 | `firmas.exe: las root signatures 1.1 y las de dentro del sombreador son las de Windows` | ninguno |
| `postpro.exe` | el COMPUTO de un posproceso: cada hilo elige SU textura de un array sin limite (bindless), muestrea la escena, escribe un RWTexture2D creado sin descripcion, InterlockedAdd y GetDimensions; corre TRADUCIDO a x86 (06-10) | 3 | `postpro.exe: el computo de un posproceso es el de Windows` | ninguno |
| `tipos.exe` | las VISTAS QUE CAMBIAN EL TIPO (D2.7): texturas TYPELESS escritas por una vista y leidas por otra (UNORM, R32_UINT, R10G10B10A2_UNORM, halfs como UINT, SINT), un InterlockedAdd por la vista de una palabra, y render targets RGBA8 vistos como UINT y SNORM (06-10) | 8 | `tipos.exe: las vistas que cambian el tipo son las de Windows` | ninguno |
| `limpieza.exe` | ClearUnorderedAccessView en el formato de la VISTA (R32_UINT y SNORM sobre RGBA8 TYPELESS, UINT sobre RGBA16 TYPELESS, R32_UINT sobre R10G10B10A2 TYPELESS, RG16F, un bufer R16G16_UINT) y solo en sus RECTANGULOS (una 2D, las dos capas de un array) (06-10) | 8 | `limpieza.exe: ClearUnorderedAccessView es el de Windows` | ninguno |

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
