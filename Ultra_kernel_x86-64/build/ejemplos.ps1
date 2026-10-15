# LOS PROGRAMAS DE EJEMPLO, Y EL VOLUMEN DE DATOS POR CATEGORIAS.
#
# ** Por que esto es un fichero (2026-08-28)
#
# Porque es lo que MAS CRECE de todo el build: cada lenguaje nuevo trae su
# tabla, su llamada a `Compilar-Ejemplos` y su Step. La lista de subidas de
# techo de L6a tiene una entrada de `build.ps1` por cada uno --INTI, el
# `.ibx`, la sonda-- y todas predijeron la siguiente. Sacarlo aqui es lo que
# hace que agregar un lenguaje deje de engordar el fichero que lo orquesta todo.
#
# Dentro: el reparto del volumen por categorias (`sys cobol c ada inti datos-componentes informe`),
# `Compilar-Ejemplos` --un bucle, cuatro lenguajes-- y el empaquetado de recursos dentro del `.bex`.
#
# [!] Se carga con punto: corre en el ambito de `build.ps1`, y `$dataBase` se
# define AQUI y lo usan despues el guardian de fantasmas y el despliegue. Es el
# mismo texto en el mismo orden.

# -- El volumen de datos, POR CATEGORIAS ---------------------------
#
# Antes todo caia en un solo `apps\`: los siete .bex de COBOL, los de C, el de
# Ada, el compositor y los .txt de entrada, revueltos. Un `ls` daba diecisiete
# lineas sin orden, y para lanzar algo habia que acordarse del nombre exacto.
#
# La primera division es **programa o dato**; dentro de los programas, por quien
# los compila:
#
#     sys\     el sistema: lo que arranca solo (d.bex, el DIRECTOR)
#     cobol\  c\  ada\      los ejemplos, por lenguaje
#     datos-componentes\   lo que los programas LEEN: validacion y bibliotecas
#     informe\             TODO lo que se ESCRIBE (12-10: el kernel lo sabe;
#                          `datos/x` escrito va a `informe/x`, leido busca en
#                          `datos-componentes/x`)
#
# * Y se teclea MENOS que antes: `cobol/banco.bex` es mas corto que
#   `apps/banco.bex`. Ordenar no ha costado tecleo, lo ha ahorrado.
#
# Desde el 12-10 el FAT32 del kernel lee y escribe NOMBRES LARGOS (y crea sus
# carpetas): `datos-componentes` es un nombre largo, como lo haria Windows.
$dataBase = Join-Path $root 'staging\BMO-DATA'
# `apps\` es para APLICACIONES, no para ejemplos del toolchain, y la distincion
# no es cosmetica: `c\`, `cobol\` y `ada\` los llena este build desde el repo, y
# `apps\` lo llena lo que alguien traiga de fuera -- hoy DOOM. Hasta ahora no lo
# creaba nadie aunque varios mensajes ya nombraban rutas `apps/...`.
# `informe\` es donde `save` deja sus hojas (una por capitulo, 2026-09-21).
# FAT32 sabe crear ficheros y no carpetas, y ensenarle seria codigo de Ring 0
# para ahorrarse esta linea: la carpeta nace aqui, con un LEEME que dice que es.
# `capturas\` es donde Impr Pant deja las suyas (2026-09-22), por lo mismo.
# `window\` es donde van los .exe de WINDOWS que corre PROTON-X (2026-09-27, lo
# pidio el propietario: "asi entran los .exe, organizado"). Seis letras: 8.3.
# `titan\` (2026-10-04): los .bex de TITAN++, desde su primer `hola` (T3).
foreach ($d in @('sys', 'cobol', 'c', 'ada', 'inti', 'titan', 'datos-componentes', 'apps', 'informe', 'capturas', 'window')) {
    New-Item -ItemType Directory -Path (Join-Path $dataBase $d) -Force | Out-Null
}
# * Y dentro de cobol\, un nivel por carpeta (ver el bloque de $cobolEjemplos
# para por que el nombre es el numero a secas). Esas las crea
# `Compilar-Ejemplos`, una por fila: aqui habia `1..10` escrito a mano, y el
# 03-10 llego `cobol\11` (BankCat) sin que nadie lo subiera a 11.
$compositorBex = Join-Path $dataBase 'sys\d.bex'
Push-Location (Split-Path -Parent $root)
try {
    if (Test-Path $compositorBex) { Remove-Item $compositorBex -Force }
    $out = & (Obrero bmo-bex-link) $compositorElf $compositorBex 2>&1
    $out | ForEach-Object {
        $linea = $_.ToString()
        if ($linea -match '^\s+(\.text|\.rodata|\.data|\.bss|entrada|->)|error|!!') {
            Write-Host ('    [bex-link] ' + $linea.Trim()) -ForegroundColor DarkGray
        }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'bex-link failed' }
    # Se borro antes a proposito: si `bex-link` no lo ha vuelto a escribir, se
    # copiaria al disco el compositor de la vez anterior y el build mentiria.
    if (-not (Test-Path $compositorBex)) { Fail 'bex-link no produjo d.bex' }

    # -- La MEDIDA, que no es un servicio -------------------------------
    #
    # `medida/coste` mide la puerta con el bucle escrito en ensamblador y la
    # juzga con `bmo-juicio`, cuyos invariantes se prueban en el anfitrion.
    #
    # ** NO REEMPLAZA a `c/coste.bex`: los dos se quedan a proposito. El fallo
    # del 16-08 lo cazo una DISCREPANCIA entre dos calculos de la misma cosa, y
    # dos implementaciones en dos lenguajes que tienen que coincidir es mas
    # fuerte que una implementacion buena. Si difieren, uno miente y ya se sabe
    # donde mirar.
    $costeElf = Join-Path $usDir 'target\x86_64-unknown-none\release\coste'
    if (-not (Test-Path $costeElf)) { Fail 'no salio el ELF de medida/coste' }
    # ** `precio` y no `coster`: el de C se llama `coste`, y dos palabras para la
    # misma cosa dicen lo que son -- DOS MEDIDAS INDEPENDIENTES DE LA MISMA
    # CANTIDAD, que tienen que coincidir. Si difieren, una miente.
    $costeBex = Join-Path $dataBase 'sys\precio.bex'
    if (Test-Path $costeBex) { Remove-Item $costeBex -Force }
    $out = & (Obrero bmo-bex-link) $costeElf $costeBex 2>&1
    $out | ForEach-Object {
        $linea = $_.ToString()
        if ($linea -match '^\s+(\.text|->)|error|!!') {
            Write-Host ('    [bex-link] ' + $linea.Trim()) -ForegroundColor DarkGray
        }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'bex-link fallo con medida/coste' }
    if (-not (Test-Path $costeBex)) { Fail 'bex-link no produjo precio.bex' }

    # -- S5 del SOMBREADOR: `sys/sombra.bex` ------------------------------
    #
    # Un sombreador SPIR-V (mandelbrot) viaja dentro, se traduce EN BMO-X con
    # los mismos crates `no_std` del anfitrion, se SELLA (MEM_OP_SELLAR, W^X)
    # y se ejecuta; contra el oraculo y contra Rust, con los tiempos.
    $sombraElf = Join-Path $usDir 'target\x86_64-unknown-none\release\sombra'
    if (-not (Test-Path $sombraElf)) { Fail 'no salio el ELF de medida/sombra' }
    $sombraBex = Join-Path $dataBase 'sys\sombra.bex'
    if (Test-Path $sombraBex) { Remove-Item $sombraBex -Force }
    $out = & (Obrero bmo-bex-link) $sombraElf $sombraBex 2>&1
    $out | ForEach-Object {
        $linea = $_.ToString()
        if ($linea -match '^\s+(\.text|->)|error|!!') {
            Write-Host ('    [bex-link] ' + $linea.Trim()) -ForegroundColor DarkGray
        }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'bex-link fallo con medida/sombra' }
    if (-not (Test-Path $sombraBex)) { Fail 'bex-link no produjo sombra.bex' }

    # -- S6: su BSF, dentro (anexo 0x09) --------------------------------
    #
    # El MISMO mandelbrot, traducido AQUI en el anfitrion y metido en el `.bex`
    # con su interfaz. `bmo-bsf-x86-64` (el adaptador del emisor x86-64 al
    # sobre; el sobre no conoce emisores) paga las cinco capas al fabricar
    # (relee y re-emite); `bmo-pack -s` lo vuelve a comprobar antes de meterlo. La app
    # lo abre sin traducir y lo compara con su propio JIT.
    $sombraBsf = Join-Path $env:TEMP 'bmo-sombra.bsf'
    $spv = Join-Path (Get-Location) 'toolchain\lang\spirv\pruebas\mandelbrot.spv'
    $out = & (Obrero bmo-bsf-x86-64) 'fabricar' $spv '-o' $sombraBsf 2>&1
    $out | ForEach-Object { Write-Host ('    [bsf] ' + $_.ToString().Trim()) -ForegroundColor DarkGray }
    if ($LASTEXITCODE -ne 0) { Fail 'bmo-bsf-x86-64 no fabrico el BSF de sombra' }
    $out = & (Obrero bmo-pack) $sombraBex '-s' $sombraBsf '-o' $sombraBex 2>&1
    $out | ForEach-Object {
        if ($_ -match 'sombreadores|\[X\]') { Write-Host ('    [sombra] ' + $_.ToString().Trim()) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'no se pudo meter el BSF en sombra.bex' }
    Remove-Item -Force $sombraBsf -ErrorAction SilentlyContinue

    # -- PROTON-X P1c: `sys/proton-x.bex` y `apps/hola.exe` -------------
    #
    # El cargador de un .exe x86-64 de Windows en Ring 3: lo lee, lo coloca en
    # dos bloques seguidos, resuelve sus importaciones contra el kernel32 de
    # la casa, SELLA su codigo y salta. `hola.exe` es el del banco del
    # cargador (platform/shared/proton-x/prueba, reproducible byte a byte), y
    # va a `apps\` porque viene de fuera: es un .exe de Windows.
    #     run sys/proton-x.bex apps/hola.exe
    $protonElf = Join-Path $usDir 'target\x86_64-unknown-none\release\proton-x'
    if (-not (Test-Path $protonElf)) { Fail 'no salio el ELF de apps/proton-x' }
    $protonBex = Join-Path $dataBase 'sys\proton-x.bex'
    if (Test-Path $protonBex) { Remove-Item $protonBex -Force }
    $out = & (Obrero bmo-bex-link) $protonElf $protonBex 2>&1
    $out | ForEach-Object {
        $linea = $_.ToString()
        if ($linea -match '^\s+(\.text|->)|error|!!') {
            Write-Host ('    [bex-link] ' + $linea.Trim()) -ForegroundColor DarkGray
        }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'bex-link fallo con apps/proton-x' }
    if (-not (Test-Path $protonBex)) { Fail 'bex-link no produjo proton-x.bex' }

    # -- LAS APPS DE LAS TECLAS: F1 TALLER, F4 LUDOTECA y F3 HERMES -----
    #
    # Las tres se enlazan igual, asi que van por UNA funcion (03-10: con la
    # tercera, el mismo bloque copiado tres veces paso de las mil lineas).
    #
    #    taller     el editor de nodos de TITAN++, con el comprobador animado
    #               (PLAN_TALLER seccion 8, B1-B3)
    #    ludoteca   tus juegos de todas las tiendas (PLAN_LA_LUDOTECA, J1)
    #    hermes     mensajes, tertulias, la ONDA y las jaulas, en una sola
    #               maquina (PLAN_HERMES, H5)
    #
    # La tecla LANZA `sys/<app>.bex`: si no esta en el disco, F1 y F4 no abren
    # nada y F3 solo lo avisa con el globo.
    #
    # ** `taller` y no `estructura`: el 29-09 se desplego `estructura.bex` y en
    # el Ryzen F1 dio "un nombre no cabe en 8.3". Windows lo copia con nombre
    # largo y el FAT32 de BMO-X busca por el corto. Por eso la funcion
    # comprueba el 8.3 de cada una, como `Construir-Ejemplos`.
    function Enlazar-App([string]$app, [string]$objetivo = 'x86_64-unknown-none') {
        $elf = Join-Path $usDir ('target\' + $objetivo + '\release\' + $app)
        if (-not (Test-Path $elf)) { Fail ('no salio el ELF de apps/' + $app) }
        $bex = Join-Path $dataBase ('sys\' + $app + '.bex')
        if ([System.IO.Path]::GetFileNameWithoutExtension($bex).Length -gt 8) { Fail ($bex + ': el tallo no cabe en 8.3') }
        if (Test-Path $bex) { Remove-Item $bex -Force }
        $out = & (Obrero bmo-bex-link) $elf $bex 2>&1
        $out | ForEach-Object {
            $linea = $_.ToString()
            if ($linea -match '^\s+(\.text|->)|error|!!') {
                Write-Host ('    [bex-link] ' + $linea.Trim()) -ForegroundColor DarkGray
            }
        }
        if ($LASTEXITCODE -ne 0) { Fail ('bex-link fallo con apps/' + $app) }
        if (-not (Test-Path $bex)) { Fail ('bex-link no produjo ' + $app + '.bex') }
    }
    # Y `jueces` (06-10): no es de una tecla; los jueces de PROTON-X, uno tras
    # otro y contados (`run sys/jueces.bex`, o un grupo: `d3d12`).
    # Y `estudio` (11-10, VE2 de PLAN_VERRANO): la luz estilo RenderMan que se
    # recuerda, con todos los nucleos (`smp all`; `run sys/estudio.bex`).
    foreach ($app in @('taller', 'ludoteca', 'hermes', 'bankcat', 'jueces')) { Enlazar-App $app }
    # `estudio`, con la coma flotante por hardware (su objetivo, `build.ps1`).
    Enlazar-App 'estudio' 'x86_64-bmo-ring3'
    # Y `teb.exe` (P1d, 27-09): lee su TEB y su PEB por `gs:` como el CRT de
    # Microsoft, y dice `bien` seis veces si el GS de la casa es el de Windows.
    # Y `ventana.exe` (P2, 27-09): una ventana Win32 de manual, con el user32 y
    # el gdi32 de la casa. Letras cambian el tinte, un clic deja un cuadrado, q
    # o ESC la cierran.
    # Y `limpia.exe` (P3a, 27-09): D3D12 y DXGI de la casa limpian la ventana
    # y presentan; una letra, el color siguiente.
    # Y `cubo.exe` (P3b2, 27-09): el cubo de X1 con la tuberia ENTERA de D3D12
    # (root signature, PSO con DXIL, buferes, DrawIndexedInstanced). Desde
    # P3b3 la casa lo DIBUJA: los DXIL corridos en la CPU y su trama.
    # Y `hilos.exe` (P4, 27-09): hilos, TLS y sincronizacion de Windows con
    # los hilos cooperativos de la casa; dice `bien` diecinueve veces.
    # Y `ficheros.exe` (P4d, 27-09): CreateFileW, ReadFile, SetFilePointerEx...
    # junto a si mismo (`window/pxtest.txt`); dice `bien` dieciseis veces.
    # Y `crt.exe` (P4e, 27-09): HeapAlloc, VirtualAlloc, su nombre, su linea y su entorno.
    # Y `texto.exe` (P4f, 27-09): UTF-8/UTF-16, WriteConsoleW, LoadLibrary y GetProcAddress.
    # Y `esperas.exe` (P4f2, 27-09): mutex, temporizadores, WaitOnAddress, FLS, la hora.
    # Y `carpetas.exe` (P4f3, 27-09): FindFirstFileW, rutas, una carpeta abierta, copiar.
    # Y `sistema.exe` (P4f4, 27-09): NtReadFile, ProcessPrng, FormatMessageW, la red, procesos.
    # Y `ucrt.exe` (P4f5, 27-09): el CRT de MSVC de sus DLL (argv, _initterm, exit, malloc, memcpy).
    # Y `stdio.exe` (P4f5, 27-09): el printf del CRT de MSVC, stdout y stderr en modo texto.
    # Y `peek.exe` (P3c1, 27-09): lo chico que le faltaba a BMOX-12 (PeekMessageW, ceil...).
    # Y `compila.exe` (P3c2, 28-09): D3DCompile pagando una vez, sobre window\sombras.
    # Y `usadll.exe` con su `saludo.dll` (P5a, 28-09): una DLL propia, como las de un juego.
    # Y `cubo12.exe` (P3c4, 28-09): el cubo por el camino de BMOX-12 (Factory6, SwapChain3, profundidad, SM5).
    # Y `bmox12.exe` (P3c, 28-09): el BMOX-12 de EPICX sin tocar, compilado en el Windows del propietario.
    # Y `hwindow.exe` (E1.1 de la ESCALERA, 05-10): D3D12HelloWindow de Microsoft, su fuente sin tocar (prueba\muestras).
    # Y `computo.exe` (E2.3a de la ESCALERA, 05-10): el COMPUTO de D3D12 (memoria compartida, barrera, dos colas), juzgado por el mismo .exe.
    # Y `instancias.exe` (N5.13, 05-10): las INSTANCIAS y los buferes de vertices de varias ranuras, juzgados por el mismo .exe.
    # Y `vistas.exe` (N5.3b y N5.3c, 05-10): las vistas en la RAIZ, los UAV de textura y con tipo, y ClearUnorderedAccessView.
    # Y `hdr.exe` (N5.16, 05-10): los render targets de FLOAT (RGBA16F, R11G11B10F) y leerlos como textura.
    # Y `uavpixel.exe` (05-10): los UAV escritos desde un DIBUJO (pixeles y vertices, InterlockedAdd).
    # Y `flotante1.exe` (N5.16b, 05-10): los de UN float (R32F, R16F), los UAV de un RGBA16F y DepthClipEnable = FALSE.
    # Y `stencil.exe` (05-10, tabla 7.2 de la ESCALERA): el STENCIL de D3D12 (REPLACE/EQUAL, INCR_SAT, mascaras, las dos caras, solo profundidad).
    # Y `olas.exe` (E2.5, 05-10): las OLAS de 32 carriles (Wave*, Quad*) en el computo y en los pixeles, con sus ayudantes.
    # Y `derivadas.exe` (D4.4, 05-10): las DERIVADAS de un cuadro de 2x2 y la MIP que eligen con ellas los muestreos.
    # Y `restos.exe` (05-10): render targets de ENTEROS, UAV sin destino y desde un GS, el plano de stencil y SV_StencilRef.
    # Y `multihilo.exe` (E2.1, 05-10): listas grabadas desde VARIOS HILOS, y colas que esperan a una valla.
    # Y `volumen.exe` (06-10): los UAV de texturas 3D y de ARRAYS (la niebla volumetrica, las cascadas de sombras).
    # Y `firmas.exe` (06-10): las ROOT SIGNATURES 1.1 y las que vienen dentro del sombreador ([RootSignature]).
    # Y `postpro.exe` (06-10): el COMPUTO de un posproceso (bindless, SampleLevel, RWTexture2D, InterlockedAdd), traducido.
    # Y `tipos.exe` (06-10): las vistas que CAMBIAN EL TIPO (D2.7): UAV, SRV y render targets de otro formato del mismo tamanio.
    # Y `limpieza.exe` (06-10): ClearUnorderedAccessView en el formato de la VISTA y con RECTANGULOS (A2).
    # Y `escena.exe` (06-10): una escena 3D DURA (sombra, mips, instancias, HDR, tonemap) contra la imagen que guardo Windows (A11).
    # Y `leefirma.exe` (06-10): los deserializadores de root signatures y la 1.1 serializada con los bytes de Windows (A3, A4).
    # Y `capas1d.exe` (06-10): los UAV de arrays de una dimension, escritos, leidos, contados y limpiados (A5).
    # Y `adyacencia.exe` (06-10): las topologias con adyacencia, con un GS y sin el (A6).
    # Y `mapeo.exe` (06-10): Map sobre una textura de un monton de la CPU, con WriteToSubresource (A7).
    # Y `preguntas.exe` (06-10): las preguntas de CheckFeatureSupport que no se contestaban (A8).
    # Y `libreta.exe` (06-10): lo raro de un render target; en el metal, la libreta de la 3060 lo apunta (9d).
    foreach ($exe in @('hola.exe', 'teb.exe', 'ventana.exe', 'limpia.exe', 'cubo.exe', 'hilos.exe', 'ficheros.exe', 'crt.exe', 'texto.exe', 'esperas.exe', 'carpetas.exe', 'sistema.exe', 'ucrt.exe', 'stdio.exe', 'peek.exe', 'compila.exe', 'usadll.exe', 'saludo.dll', 'cubo12.exe', 'seh.exe', 'bmox12.exe', 'tanda1.exe', 'tanda2.exe', 'tanda3.exe', 'tanda3b.exe', 'tanda3c.exe', 'tanda4.exe', 'tanda5.exe', 'tanda6.exe', 'tanda7.exe', 'tanda8.exe', 'tanda9.exe', 'tanda10.exe', 'tanda11.exe', 'tanda12.exe', 'tanda13.exe', 'diario.exe', 'tanda14.exe', 'tanda4m.exe', 'tanda14b.exe', 'tanda15.exe', 'tanda16.exe', 'tanda17.exe', 'tanda18.exe', 'tanda19.exe', 'tanda19m.exe', 'tanda20.exe', 'tanda21.exe', 'tanda22.exe', 'tanda22d.dll', 'tanda23.exe', 'tanda24.exe', 'tanda25.exe', 'tanda26.exe', 'tanda27.exe', 'tanda28.exe', 'tanda29.exe', 'tanda30.exe', 'tanda31.exe', 'tanda32.exe', 'tanda33.exe', 'tanda34.exe', 'tanda35.exe', 'tanda36.exe', 'tanda37.exe', 'tanda38.exe', 'tanda39.exe', 'tanda41.exe', 'tanda42.exe', 'tanda43.exe', 'tanda44.exe', 'tanda45.exe', 'tanda46.exe', 'tanda47.exe', 'tanda48.exe', 'vueltas.exe', 'hwindow.exe', 'computo.exe', 'instancias.exe', 'vistas.exe', 'hdr.exe', 'uavpixel.exe', 'flotante1.exe', 'stencil.exe', 'olas.exe', 'derivadas.exe', 'restos.exe', 'multihilo.exe', 'volumen.exe', 'firmas.exe', 'postpro.exe', 'tipos.exe', 'limpieza.exe', 'escena.exe', 'leefirma.exe', 'capas1d.exe', 'adyacencia.exe', 'mapeo.exe', 'preguntas.exe', 'libreta.exe')) {
        Copy-Item (Join-Path (Get-Location) ('platform\shared\proton-x\prueba\' + $exe)) (Join-Path $dataBase ('window\' + $exe)) -Force
    }
    # Y la imagen de Windows de `escena.exe` (A11): la de la 3060 del
    # propietario (06-10, `escena_3060.ref` en el repo), al lado del .exe con
    # el nombre que busca; si no esta, la que dejo `escena.exe guardar` aqui.
    $escenaRef = Join-Path (Get-Location) 'platform\shared\proton-x\prueba\escena_3060.ref'
    if (-not (Test-Path $escenaRef)) { $escenaRef = Join-Path (Get-Location) 'platform\shared\proton-x\prueba\escena.ref' }
    if (Test-Path $escenaRef) { Copy-Item $escenaRef (Join-Path $dataBase 'window\escena.ref') -Force }
    $leemeWin = @(
        'WINDOW -- los .exe de Windows (x86-64) que corre PROTON-X',
        '',
        '  run sys/proton-x.bex window/hola.exe           uno de aqui',
        '  run sys/proton-x.bex window/crt.exe -nivel 3   lo de detras es su linea de ordenes',
        '  run sys/jueces.bex                            TODOS los jueces, solos (o: d3d12, tandas, dentro)',
        '',
        'Su directorio actual es ESTA carpeta: lo que un .exe escriba con una ruta',
        'relativa (ficheros.exe deja pxtest.txt) cae aqui, y aqui lo encuentra.',
        'Los de prueba los pone el build; los tuyos, copialos aqui (nombres 8.3).'
    )
    Set-Content -LiteralPath (Join-Path $dataBase 'window\LEEME.TXT') -Value $leemeWin -Encoding ascii
    # P3c2 (28-09): la carpeta del HLSL que se compila una vez en Windows. La
    # crea el build porque el FAT32 de BMO-X no crea carpetas desde Ring 3.
    New-Item -ItemType Directory -Path (Join-Path $dataBase 'window\sombras') -Force | Out-Null
    $leemeSom = @(
        'SOMBRAS -- el HLSL que un .exe de Windows compila en marcha (D3DCompile)',
        '',
        'En BMO-X no hay compilador de HLSL. La primera vez, PROTON-X deja aqui',
        'cada pedido: <huella>.hls (la fuente) y <huella>.ent (entrada, perfil...).',
        'En Windows, con este disco puesto:',
        '',
        '  platform\shared\proton-x\obrero\sombras.exe A:\window\sombras',
        '',
        'compila cada .hls pendiente con el d3dcompiler_47 de Windows y deja',
        '<huella>.cso al lado. La proxima vez, en BMO-X, D3DCompile lo encuentra.',
        'El despliegue NO borra esta carpeta: lo compilado se queda.'
    )
    Set-Content -LiteralPath (Join-Path $dataBase 'window\sombras\LEEME.TXT') -Value $leemeSom -Encoding ascii
    # P3c4 (28-09): los .cso del cubo de BMOX-12 que sombras.exe ya compilo en
    # Windows (prueba\sombras): cubo12.exe (y bmox12.exe, con los suyos) no espera a nadie.
    Get-ChildItem (Join-Path (Get-Location) 'platform\shared\proton-x\prueba\sombras\*.cso') | ForEach-Object {
        Copy-Item $_.FullName (Join-Path $dataBase ('window\sombras\' + $_.Name)) -Force
    }
    # E1.2 a E1.6 y E2 de la ESCALERA (05-10): las muestras de Microsoft que
    # leen sus .cso (y sus datos) de SU carpeta (los Hello los llaman igual
    # todos), cada una en window\<muestra>\ con lo suyo (prueba\muestras\<muestra>).
    # run sys/proton-x.bex window/htriang/htriang.exe
    foreach ($muestra in @('htriang', 'htexture', 'hcbuffer', 'hframes', 'hbundles', 'dynindex', 'nbody', 'indirect', 'predica')) {
        $d = Join-Path $dataBase ('window\' + $muestra)
        New-Item -ItemType Directory -Path $d -Force | Out-Null
        Copy-Item (Join-Path (Get-Location) ('platform\shared\proton-x\prueba\' + $muestra + '.exe')) $d -Force
        Copy-Item (Join-Path (Get-Location) ('platform\shared\proton-x\prueba\muestras\' + $muestra + '\*')) $d -Force
    }
    Write-Host '    [proton-x] sys\proton-x.bex y window\{hola,teb,ventana,limpia,cubo,hilos,ficheros,crt,texto,esperas}.exe (run sys/proton-x.bex window/esperas.exe)' -ForegroundColor DarkGray
} finally { Pop-Location }

# El CATALOGO (que se compila y adonde va): en `catalogo.ps1` desde el 13-10
# (L6a). Solo define las listas.
. (Join-Path $PSScriptRoot 'catalogo.ps1')
if ($null -ne $script:salida) { exit $script:salida }

function Compilar-Ejemplos {
    # ** UN bucle, tres lenguajes. Estaba escrito TRES VECES -- COBOL, Ada y C--
    # con la misma comprobacion de 8.3, el mismo `Join-Path`, el mismo filtro de
    # salida y los mismos dos `Fail`. Lo unico distinto era el crate, la
    # etiqueta y que el de Ada aceptaba ademas la palabra `linea` en su filtro.
    #
    # Tres copias de una regla es tres sitios donde arreglarla, y el dia que
    # alguien arregle dos se notara en el tercero -- que es exactamente el
    # patron que esta casa lleva pagando todo el dia.
    #
    # [!] El tope de 8 caracteres NO es una convencion nuestra: el driver FAT32
    # del kernel se NIEGA a recortar un nombre, asi que un tallo de nueve letras
    # no es feo, es un fichero que no se puede abrir.
    #
    # ** `-PorObjeto` (E5c, 2026-09-17): el fuente no se compila a programa sino
    # a UNIDAD (`-c`), y el programa lo hace `bmo-enlazar`. El camino es mas
    # largo y el motivo es uno solo: **el enlazador tira lo que nadie llama y el
    # modo imagen no**. Medido antes de cambiarlo, `cubo_C` bajaba un 37,9 %.
    #
    # Solo pueden ir por aqui los frontends que saben escribir un objeto -- hoy
    # C y C++. COBOL y Ada tienen emisor propio y todavia no (E6 y E7), asi que
    # NO se les pasa la bandera: van por donde iban.
    #
    # ** `-Orden` (T3 de TITAN++, 2026-10-04): la herramienta de TITAN++ va como
    # cargo -- `titan build FICHERO -o SALIDA` --, y la orden va DELANTE del
    # fuente. Sin `-Orden` la llamada es la de siempre, byte a byte.
    param($ejemplos, $crate, $etiqueta, $patron, $dataBase, $repo, [switch]$PorObjeto, [string]$Orden = '')
    foreach ($e in $ejemplos) {
        $tallo = [System.IO.Path]::GetFileNameWithoutExtension($e.out)
        if ($tallo.Length -gt 8) { Fail ($e.out + ': el tallo no cabe en 8.3') }
        # ** La carpeta la crea la FILA, no una lista aparte (05-10). Habia un
        # `1..10` para `cobol\N`; el 03-10 entro `cobol\11` (BankCat) y
        # `libro.cob` no pudo escribirse. Y como el `Fail` de un fichero cargado
        # con punto no paraba el build (ver `Fail` en comun.ps1), DESDE ESE DIA
        # todo lo que va detras de COBOL --Ada, C++, C, INTI, TITAN++, los
        # iconos, los datos y DOOM-- se dejo de construir sin que nadie lo
        # viera, y el build dijo COMPLETE.
        $carpeta = Join-Path $dataBase $e.dir
        New-Item -ItemType Directory -Path $carpeta -Force | Out-Null
        $dst = Join-Path $carpeta $e.out
        if ($PorObjeto) {
            # El `.bo` es intermedio y no vive en el espejo: lo que se despliega
            # es el programa, no la unidad con la que se hizo.
            $bo = Join-Path $env:TEMP ($tallo + '.bo')
            $out = & (Obrero $crate) (Join-Path $repo $e.src) -c -o $bo 2>&1
            $out | ForEach-Object {
                if ($_ -match $patron) { Write-Host ('    [' + $etiqueta + '] ' + $_) -ForegroundColor DarkGray }
            }
            if ($LASTEXITCODE -ne 0) { Fail ('no compilo ' + $e.src) }
            if (-not (Test-Path $bo)) { Fail ('no salio la unidad de ' + $e.src) }
            $out = & (Obrero bmo-enlazar) -o $dst $bo 2>&1
            $out | ForEach-Object {
                if ($_ -match 'ok:|error|poda:|aviso:') { Write-Host ('    [' + $etiqueta + '] ' + $_) -ForegroundColor DarkGray }
            }
            $fallo = $LASTEXITCODE
            Remove-Item $bo -ErrorAction SilentlyContinue
            if ($fallo -ne 0) { Fail ('no enlazo ' + $e.src) }
        } else {
            $delante = @()
            if ($Orden) { $delante = @($Orden) }
            $out = & (Obrero $crate) @delante (Join-Path $repo $e.src) -o $dst 2>&1
            $out | ForEach-Object {
                if ($_ -match $patron) { Write-Host ('    [' + $etiqueta + '] ' + $_) -ForegroundColor DarkGray }
            }
            if ($LASTEXITCODE -ne 0) { Fail ('no compilo ' + $e.src) }
        }
        if (-not (Test-Path $dst)) { Fail ('no salio ' + $e.out) }
    }
}

# Las imagenes que fabrica el build (`Nuevo-Bico` y las suyas): en
# `imagenes.ps1` desde el 05-10 (L6a). Solo define funciones.
. (Join-Path $PSScriptRoot 'imagenes.ps1')
if ($null -ne $script:salida) { exit $script:salida }

$repo = Split-Path -Parent $root
Push-Location $repo
try {
    Step 'Building COBOL example programs...'
    Compilar-Ejemplos $cobolEjemplos 'bmo-cobol-x86-64' 'cobol' 'ok:|error' $dataBase $repo


    Step 'Building ADA example programs...'
    Compilar-Ejemplos $adaEjemplos 'bmo-ada-x86-64' 'ada' 'ok:|error|linea' $dataBase $repo

    Step 'Building C++ example programs...'
    Compilar-Ejemplos $cppEjemplos 'bmo-cpp-x86-64' 'cpp' 'ok:|error|linea' $dataBase $repo -PorObjeto

    Step 'Building C example programs...'
    # Sin --base ni --asm-path: ese camino usa el PREPROCESADOR, que es lo que
    # resuelve `#include <bmo/...>`. Con ellos se toma el de modulos, que no lo
    # llama.
    Compilar-Ejemplos $cEjemplos 'bmo-c-x86-64' 'c' 'ok:|error' $dataBase $repo -PorObjeto

    Step 'Building INTI probes...'
    # ** `run inti/cpu.ibx`. Por el MISMO helper que los otros tres: si INTI
    # necesitara un camino propio al disco, seria que no es un frontend mas. Y es
    # el fallo que este bloque ya tenia escrito de C -- *compila y no se
    # despliega* -- repetido con INTI, cuyo binario vivia fuera del espejo.
    #
    # ** `.ibx` desde el 2026-08-22, y no es un cambio de gusto: es el MISMO
    # formato --lo carga el mismo cargador y lo lee el mismo gate-- con un nombre
    # que dice a que se ha comprometido. Un `.ibx` en el disco declara su
    # perfil, sus piezas y su mesa de katanas, y no habria llegado aqui si esa
    # mesa no cuadrara con sus bytes. `.bex` se queda para los otros tres.
    # ** `run inti/pulso.ibx` (2026-09-12): el perfil en TIEMPO REAL. El kernel
    # lee los contadores del silicio y la sonda los PREGUNTA cada medio segundo.
    # (`bico.ibx`, la primera HERRAMIENTA en INTI -- 2026-09-12 --, salio el
    # 10-10 con `png.inti`, corte 4e: las dos son de TITAN++ --
    # `titan/bico.bex` y `titan/png.bex`, abajo --. 1d de EL_FOCO.)
    # ** LOS QUE SALIERON SE BORRAN DEL DISCO (15-10, el propietario: *"tengo 2
    # app navegador [...] elimines uno que genera inti"*). Quitar una fila de
    # abajo dejaba de CONSTRUIR el programa, pero el `.ibx` de antes se quedaba
    # en la carpeta de datos y el escritorio lo seguia ofreciendo: NAVEGAR salia
    # dos veces, el de INTI (muerto el 10-10) y el de TITAN++. Cada programa que
    # sale de INTI con su relevo en TITAN++ se apunta AQUI, y se borra.
    # ** Y en `apps\` tambien (15-10, la captura del propietario: SEGUIA saliendo
    # NAVEGAR dos veces). `navegar.ibx` no vivia en `inti\`: vivia en `apps\`,
    # que es la carpeta que el escritorio convierte en iconos.
    # (`ecce.bex` suelto, tambien: desde el 15-10 ECCE es `apps\ecce\`, KA1.)
    foreach ($retirado in @('navegar.ibx', 'ventana.ibx', 'cubo.ibx', 'bico.ibx', 'png.ibx', 'musica.ibx', 'ecce.bex')) {
        foreach ($carpeta in @('inti', 'apps')) {
            $viejo = Join-Path (Join-Path $dataBase $carpeta) $retirado
            if (Test-Path $viejo) {
                Remove-Item $viejo -Force
                Write-Host ('    [inti] retirado del staging: ' + $carpeta + '\' + $retirado + ' (su relevo es de TITAN++)') -ForegroundColor DarkGray
            }
        }
    }
    Compilar-Ejemplos @(
        @{ src = 'toolchain\lang\inti\sondas\cpu.inti'; out = 'cpu.ibx'; dir = 'inti' },
        @{ src = 'toolchain\lang\inti\sondas\pulso.inti'; out = 'pulso.ibx'; dir = 'inti' }
        # (`cubo.ibx` salio el 09-10: VERRANO lo manda TITAN++. Su relevo es
        # `titan/cubogira.bex`, abajo -- corte 4 de PLAN_INTI_SAMURAI.)
        # (`navegar.ibx` y la sonda `ventana.ibx` salieron el 10-10, corte 4c:
        # NAVEGAR es de TITAN++ -- `apps/navegar.bex`, abajo -- y la ventana
        # tambien -- `titan/ventana.bex` --. F5 de docs/plan/EL_FOCO.md.)
        # (`musica.ibx`, el REPRODUCTOR de INTI del 13-09, salio el 15-10: el
        # sonido es de TITAN++ -- `apps/musica.bex`, abajo, Q2 de EL_FOCO --.
        # Su fuente y su banco se quedan como prueba del compilador de INTI
        # hasta que el Ryzen suene con el de TITAN++.)
    ) 'bmo-inti-x86-64' 'inti' 'ok:|error|aviso' $dataBase $repo

    Step 'Building TITAN++ programs...'
    # ** `run titan/hola.bex` (T3, 2026-10-04): el PRIMER .bex de TITAN++, y
    # escribe en la consola (decidido el 30-09; la ventana, despues). Su propio
    # emisor --no el de INTI--, la puerta de `bmo-lower` y el gate con
    # manifiesto. `.bex` y no `.ibx`: el `.ibx` es el compromiso de INTI.
    Compilar-Ejemplos @(
        @{ src = 'toolchain\lang\titan\ejemplos\nivel0\hola.titan'; out = 'hola.bex'; dir = 'titan' },
        @{ src = 'toolchain\lang\titan\ejemplos\nivel0\dos_saludos.titan'; out = 'dos.bex'; dir = 'titan' },
        # ** LB7 (2026-10-09): el cubo de bmo_cubo que GIRA -- contado por gpu
        # fn (al correr, en la CPU) y publicado en la lamina de VERRANO cada
        # 16 ms, diez minutos. Un PAQUETE (`screen` y `gpu` en su Titan.toml):
        # se construye desde su `src/main.titan`. En el metal, como con INTI:
        # `run titan/cubogira.bex` y `gpu verrano banco inti`, que dibuja con
        # la 3060 lo que haya en la lamina (LB7 de PLAN_LAS_LIBRERIAS).
        @{ src = 'toolchain\lang\titan\ejemplos\nivel11\cubo_gira\src\main.titan'; out = 'cubogira.bex'; dir = 'titan' },
        # ** DOS CUBOS QUE SE TAPAN (M1 = V2 de PLAN_VERRANO, 2026-10-11): la
        # lamina con `director.profundidad(true)`, en su ventana. En el metal:
        # `run titan/doscubos.bex` desde Ejecutar.
        @{ src = 'toolchain\lang\titan\ejemplos\nivel11\dos_cubos\src\main.titan'; out = 'doscubos.bex'; dir = 'titan' },
        # ** F1 de EL_FOCO (2026-10-10): la VENTANA de TITAN++ -- una
        # superficie de BMO-X como la de INTI, un degradado y una barra que la
        # cruza diez segundos. En el metal: `run titan/ventana.bex` desde
        # Ejecutar (desde el shell no hay escritorio que componga: lo dice).
        @{ src = 'toolchain\lang\titan\ejemplos\nivel11\ventana\src\main.titan'; out = 'ventana.bex'; dir = 'titan' },
        # ** LA LAMINA (A4 de EL_FOCO, 15-10): el nivel 15 de TITAN++ al disco,
        # para su prueba del Ryzen (hoja 6b, fila A4). `suelta.bex` pide VEINTE
        # laminas con un tope de OCHO a la vez: si no se soltaran, la novena
        # diria NO. Debe escribir `20 laminas de 4096 bytes, una por vuelta: 190`.
        @{ src = 'toolchain\lang\titan\ejemplos\nivel15\pide_y_suelta.titan'; out = 'suelta.bex'; dir = 'titan' },
        @{ src = 'toolchain\lang\titan\ejemplos\nivel15\dos_laminas.titan'; out = 'laminas.bex'; dir = 'titan' },
        # ** NAVEGAR en TITAN++ (F4 y F5 de EL_FOCO, 2026-10-10): la cara de la
        # LAMINA -- la que ofrece el antenista, o la del disco -- en su
        # ventana, con los mismos pixeles que el de INTI, que se fue. Va a
        # `apps/`, con DOOM, porque es una APP; su icono, en el empaquetado.
        @{ src = 'Ultra_userspace\apps\navegar\src\main.titan'; out = 'navegar.bex'; dir = 'apps' },
        # ** MUSICA en TITAN++ (Q2 de EL_FOCO, 15-10; el propietario: "titan
        # que aprenda del audio y no inti"): el reproductor, por las VOCES del
        # orquestador. `run apps/musica.bex` toca `datos/tema.mus`.
        @{ src = 'Ultra_userspace\apps\musica\src\main.titan'; out = 'musica.bex'; dir = 'apps' },
        # ** RESOLUCION (R1 de EL_FOCO, 2026-10-10): elegir la medida de las
        # ventanas de TITAN++ y guardarla en `datos\resol.txt`
        # (`director.guarda`, TA4); `titan/ventana.bex` ya se abre con ella.
        @{ src = 'Ultra_userspace\apps\resolucion\src\main.titan'; out = 'resol.bex'; dir = 'apps' },
        # ** ECCE (PL1 y PL3 de docs/plan/PLAN_ECCE.md, 15-10): el bloc de
        # notas, el primer peldanio de la app de escribir del propietario.
        # Guarda solo en `datos\notas.txt`. Una APP: con su icono.
        # ** UNA APP CARPETA desde el 15-10 (KA1 de PLAN_EL_KERNEL_DE_LA_APP):
        # `apps\ecce\nucleo.bex`, el que arranca; sus piezas iran al lado. El
        # escritorio la ve como UNA app con el nombre de la carpeta, y su cara
        # (IC2) la mete `titan build` desde el `icono.svg` del Titan.toml.
        @{ src = 'Ultra_userspace\apps\ecce\src\main.titan'; out = 'nucleo.bex'; dir = 'apps\ecce' },
        # ** TINTA (TB1 de docs/plan/PLAN_LA_TINTA.md, 2026-10-10): el lienzo
        # minimo -- pincel redondo con antialias, goma y PNG --, el primer
        # peldanio del Adobe + Clip Studio Paint propio. Una APP: con su icono.
        @{ src = 'Ultra_userspace\apps\tinta\src\main.titan'; out = 'tinta.bex'; dir = 'apps' },
        # ** MUNDO (MC1 de docs/plan/PLAN_MUNDO.md, 2026-10-11): el primer
        # peldanio del Minecraft propio -- bloques en 3D por VERRANO, una
        # camara que anda, quitar y poner --. Una APP: con su icono.
        @{ src = 'Ultra_userspace\apps\mundo\src\main.titan'; out = 'mundo.bex'; dir = 'apps' },
        # ** BICO y PNG (corte 4e de INTI, 2026-10-10): las herramientas de
        # imagen, de TITAN++ (`director.crea`, `escribe`, `cierra`). En el
        # metal: `run titan/bico.bex` (datos/foto.bmp y foto.qoi a BICO, sus
        # datos se generan abajo) y `run titan/png.bex` (deja datos/hola.png).
        @{ src = 'Ultra_userspace\apps\bico\src\main.titan'; out = 'bico.bex'; dir = 'titan' },
        @{ src = 'Ultra_userspace\apps\png\src\main.titan'; out = 'png.bex'; dir = 'titan' }
    ) 'bmo-titan-x86-64' 'titan' 'ok:|T00|no se ha' $dataBase $repo -Orden 'build'

    # -- Las dos imagenes que `titan/bico.bex` convierte ------------------
    #
    # Se GENERAN y no se copian: un binario en el repo es un fichero que nadie
    # puede leer en un diff. 16x16 las dos, con un degradado que se reconoce a
    # simple vista -- si el conversor invirtiera las filas, se veria.
    $imgDst = Join-Path $dataBase 'datos-componentes'
    New-Item -ItemType Directory -Force $imgDst | Out-Null
    [System.IO.File]::WriteAllBytes((Join-Path $imgDst 'foto.bmp'), (Nuevo-Bmp))
    [System.IO.File]::WriteAllBytes((Join-Path $imgDst 'foto.qoi'), (Nuevo-Qoi))
    Write-Host '    [datos-componentes] foto.bmp y foto.qoi (16x16, para titan/bico.bex)' -ForegroundColor DarkGray
    # ** Y un PNG y un JPEG de verdad (2026-09-20), para el visor: son los
    # ficheros con los que `bmo-imagen` se prueba en el anfitrion contra
    # Pillow, asi que lo que se ve en el Ryzen es lo que el banco ya juzgo.
    # Binarios chicos que viven en `pruebas/` de la crate, no aqui.
    $imgPruebas = Join-Path $repo 'platform\shared\bmo-imagen\pruebas'
    Copy-Item (Join-Path $imgPruebas 'inti256.png') (Join-Path $imgDst 'inti.png') -Force
    Copy-Item (Join-Path $imgPruebas 'foto.jpg') (Join-Path $imgDst 'arranque.jpg') -Force
    Write-Host '    [datos-componentes] inti.png (256x256 RGBA) y arranque.jpg (640x362 4:2:0), para el visor' -ForegroundColor DarkGray
    # La melodia que `musica.ibx` toca si no le dan otra. Es TEXTO: esta en el
    # repo y se lee en un diff.
    Copy-Item (Join-Path $repo 'toolchain\lang\inti\ejemplos\tema.mus') (Join-Path $imgDst 'tema.mus') -Force
    Write-Host '    [datos-componentes] tema.mus (Vivaldi, para apps/musica.bex)' -ForegroundColor DarkGray
    # La lamina de example.com, la misma que sirve la antena: NAVEGAR la pinta
    # si esta (N2 de PLAN_NAVEGAR). Texto, en el repo, y se lee en un diff.
    # [!] 8.3: el FAT32 de BMO-X busca por nombre corto y se salta las
    # entradas de nombre largo, asi que `ejemplo.lamina` no lo encontraria.
    Copy-Item (Join-Path $repo 'toolchain\tools\antena\ejemplo.lamina') (Join-Path $imgDst 'ejemplo.lam') -Force
    Write-Host '    [datos-componentes] ejemplo.lam (example.com, para apps/navegar.bex)' -ForegroundColor DarkGray
    # El aspecto del escritorio: lo lee el DIRECTOR al arrancar. Texto, en el
    # repo, y se edita ahi -- el despliegue pisa el de A:\sys\.
    $sysDst = Join-Path $dataBase 'sys'
    New-Item -ItemType Directory -Force $sysDst | Out-Null
    Copy-Item (Join-Path $repo 'Ultra_userspace\services\director\director.cfg') (Join-Path $sysDst 'director.cfg') -Force
    Write-Host '    [sys] director.cfg (el aspecto del escritorio)' -ForegroundColor DarkGray
    # ** EL FONDO ES LA CIUDAD DEL GATO (2026-09-22): el arte del propietario,
    # `activos/fondo/ciudad.webp`, cubriendo 1920x1080. En el repo va el arte
    # y el conversor que se lee (`toolchain/tools/fondo/a_qoi.py`); el `.qoi`
    # se hace aqui, y solo si el arte o el conversor son mas nuevos que el que
    # hay (codificar 2 millones de pixeles en python son unos segundos).
    # Sin python o sin Pillow se queda la noche generada, y se DICE.
    $fondoDst = Join-Path $sysDst 'fondo.qoi'
    $ciudad = Join-Path $repo 'activos\fondo\ciudad.webp'
    $aQoi = Join-Path $repo 'toolchain\tools\fondo\a_qoi.py'
    $pyFondo = Get-Command python -ErrorAction SilentlyContinue
    $fondoOk = $false
    if ((Test-Path $fondoDst) -and (Test-Path $ciudad) -and
        ((Get-Item $fondoDst).LastWriteTime -gt (Get-Item $ciudad).LastWriteTime) -and
        ((Get-Item $fondoDst).LastWriteTime -gt (Get-Item $aQoi).LastWriteTime) -and
        ((Get-Item $fondoDst).Length -gt 1000000)) {
        $fondoOk = $true
    } elseif ($pyFondo -and (Test-Path $ciudad)) {
        & $pyFondo.Source $aQoi $ciudad $fondoDst 1920 1080 | Out-Null
        $fondoOk = ($LASTEXITCODE -eq 0) -and (Test-Path $fondoDst)
    }
    if ($fondoOk) {
        Write-Host '    [sys] fondo.qoi (1920x1080, la ciudad del gato)' -ForegroundColor DarkGray
    } else {
        [System.IO.File]::WriteAllBytes($fondoDst, (Nuevo-Fondo))
        Write-Host '    [sys] fondo.qoi (480x270 GENERADO: sin python y Pillow no hay ciudad)' -ForegroundColor Yellow
    }

    # -- Meter los datos DENTRO del .bex ---------------------------
    #
    # Un `.bex` empaquetado sigue siendo un `.bex` que arranca: el cargador
    # mapea Code/RoData/Data/Bss y **salta el resto contandolo**. Lo que cambia
    # es que la app pasa a ser UN fichero.
    if ($cRecursos.Count -gt 0) {
        Step 'Packaging C examples (datos DENTRO del .bex)'
        $tmp = Join-Path $env:TEMP 'bmo-pack-tmp'
        New-Item -ItemType Directory -Force $tmp | Out-Null
        foreach ($paq in $cRecursos) {
            $bex = Join-Path $dataBase $paq.bex
            if (-not (Test-Path $bex)) { Fail ('no esta ' + $paq.bex + ' para empaquetar') }
            $args = @($bex)
            foreach ($r in $paq.recursos) {
                $f = Join-Path $tmp $r.nombre
                if ($r.ContainsKey('texto')) {
                    # Sin salto final y sin BOM: el programa cuenta los bytes.
                    [System.IO.File]::WriteAllText($f, $r.texto, (New-Object System.Text.UTF8Encoding $false))
                } elseif ($r.ContainsKey('desde')) {
                    # ** UN RECURSO QUE ES UN FICHERO DEL REPO, tal cual.
                    #
                    # `texto` sirve para una linea; una guia de cien no cabe en
                    # un literal de PowerShell sin volverse ilegible, y ademas
                    # **se edita mejor como fichero**: cambiar `guia.txt` y
                    # reempacar no toca ni una linea del programa que la muestra.
                    $orig = Join-Path $repo $r.desde
                    if (-not (Test-Path $orig)) { Fail ('no esta el recurso ' + $r.desde) }
                    Copy-Item -LiteralPath $orig -Destination $f -Force
                } elseif ($r.ContainsKey('icono')) {
                    [System.IO.File]::WriteAllBytes($f, (Nuevo-Bico $r.icono))
                } else {
                    [System.IO.File]::WriteAllBytes($f, [byte[]]$r.bytes)
                }
                $args += @('-r', ($r.nombre + '=' + $f))
            }
            $args += @('-o', $bex)
            $out = & (Obrero bmo-pack) @args 2>&1
            # Solo las lineas de ESTE paso. Un `-match '->'` a secas se traga
            # los `-->` de las advertencias de cargo, y entonces el paso que
            # importa queda enterrado en avisos que no son suyos.
            $out | ForEach-Object {
                if ($_ -match 'recurso\(s\)' -or $_ -match '^\s{4}\S+\s+\d+ B$' -or $_ -match '\[X\]') {
                    Write-Host ('    [pack] ' + $_) -ForegroundColor DarkGray
                }
            }
            if ($LASTEXITCODE -ne 0) { Fail ('no se pudo empaquetar ' + $paq.bex) }
        }
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }

    # -- DOOM: en `build\doom.ps1` (26-09, L6a) ------------------
    . (Join-Path $PSScriptRoot 'doom.ps1')
    if ($null -ne $script:salida) { exit $script:salida }

    # -- Los DATOS de los ejemplos ---------------------------------
    #
    # * Este paso tampoco existia, y era peor que el de C: los .txt que leen
    # `batch`, `conceptos` y `cartera` vivian SOLO en staging\, que esta en el
    # .gitignore. O sea, no eran del repositorio. Un `-Clean` o un disco nuevo
    # los borraba y **no habia forma de regenerarlos**: los ejemplos de ficheros
    # quedaban sin entrada y sin nadie que supiera que debian contener.
    #
    # Ahora viven en toolchain\lang\cobol\examples\datos\ y se despliegan como
    # se despliega un .bex.
    Step 'Staging example data...'
    $leeme = @(
        'INFORME -- las hojas que escribe `save` desde el escritorio',
        '',
        '  salida.txt             el informe ENTERO (7 capitulos), como siempre',
        '  historial\             una copia con fecha de cada save (AAAA-MM-DD_HH-MM.txt)',
        '  CAIDA.TXT, CAIDAnnn.TXT, CABINA.LOG   lo que el kernel guarda de un arranque caido',
        '  gpu\, estado\         los volcados de la 3060 y las banderas de save mode / smp siempre',
        '  INDICE.TXT             la cabecera y que hoja es que',
        '  SESION.TXT             1. lo que se tecleo y lo que contesto',
        '  MAQUINA.TXT            2. cpu, caches medidas, extensiones',
        '  MEMORIA.TXT            3. marcos, entregas, cache de disco',
        '  CONSUMO.TXT            4. escritorio, RAM, tareas, DMA, usb, prestamos, avisos',
        '  PROGRAMA.TXT           5. memoria pedida y la ficha BEF2 de cada programa',
        '  DISCO.TXT              6. aparato, particiones, ESTRATOS',
        '  AUTOPSIA.TXT           7. el ultimo fallo de Ring 3',
        '  DATOS.TXT              los numeros de las siete, una linea por dato (capitulo.clave = valor unidad), para una maquina',
        '',
        '  save cpu|mem|consumo|apps|disco|autopsia   un tema suelto, aqui mismo (cpu.txt, ...)',
        '',
        'Cada `save` REESCRIBE estas hojas; la copia con fecha queda en historial\.',
        'TODO lo que BMO-X escribe viene aqui. Lo que se LEE esta en ..\datos-componentes\.'
    )
    Set-Content -LiteralPath (Join-Path $dataBase 'informe\LEEME.TXT') -Value $leeme -Encoding ascii
    Write-Host '    [informe] LEEME.TXT (la carpeta de las hojas de save)' -ForegroundColor DarkGray
    $leemeCap = @(
        'CAPTURAS -- lo que guarda Impr Pant desde el escritorio',
        '',
        '  Impr Pant          la pantalla entera',
        '  Alt + Impr Pant    solo la ventana de delante',
        '  Ctrl+Shift+S       RECORTE: arrastra un rectangulo con el raton (ESC cancela)',
        '  captura            lo mismo, escrito en Ejecutar (captura ventana: la de delante)',
        '',
        '  capNNNNN.png       PNG hecho por BMO-X: lo abren Windows y el visor de BMO-X',
        '',
        'El despliegue NO borra esta carpeta: las capturas se quedan en el disco de datos.'
    )
    Set-Content -LiteralPath (Join-Path $dataBase 'capturas\LEEME.TXT') -Value $leemeCap -Encoding ascii
    Write-Host '    [capturas] LEEME.TXT (la carpeta de Impr Pant)' -ForegroundColor DarkGray
    $datosSrc = Join-Path $repo 'toolchain\lang\cobol\examples\datos'
    $datosDst = Join-Path $dataBase 'datos-componentes'
    foreach ($d in (Get-ChildItem -LiteralPath $datosSrc -Filter '*.txt')) {
        Copy-Item -LiteralPath $d.FullName -Destination (Join-Path $datosDst $d.Name) -Force
        Write-Host ('    [datos-componentes] ' + $d.Name + ' (' + $d.Length + ' B)') -ForegroundColor DarkGray
    }
} finally { Pop-Location }
