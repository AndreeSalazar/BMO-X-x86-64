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
# Dentro: el reparto del volumen por categorias (`sys cobol c ada inti datos`),
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
#     datos\   lo que los programas LEEN y ESCRIBEN
#
# * Y se teclea MENOS que antes: `cobol/banco.bex` es mas corto que
#   `apps/banco.bex`. Ordenar no ha costado tecleo, lo ha ahorrado.
#
# Los nombres de carpeta tambien son 8.3: el driver FAT32 del kernel se NIEGA a
# recortar, y una carpeta recortada manda a otro sitio igual que un fichero.
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
foreach ($d in @('sys', 'cobol', 'c', 'ada', 'inti', 'titan', 'datos', 'apps', 'informe', 'capturas', 'window')) {
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
    function Enlazar-App([string]$app) {
        $elf = Join-Path $usDir ('target\x86_64-unknown-none\release\' + $app)
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
    foreach ($app in @('taller', 'ludoteca', 'hermes', 'bankcat')) { Enlazar-App $app }
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
    foreach ($exe in @('hola.exe', 'teb.exe', 'ventana.exe', 'limpia.exe', 'cubo.exe', 'hilos.exe', 'ficheros.exe', 'crt.exe', 'texto.exe', 'esperas.exe', 'carpetas.exe', 'sistema.exe', 'ucrt.exe', 'stdio.exe', 'peek.exe', 'compila.exe', 'usadll.exe', 'saludo.dll', 'cubo12.exe', 'seh.exe', 'bmox12.exe', 'tanda1.exe', 'tanda2.exe', 'tanda3.exe', 'tanda3b.exe', 'tanda3c.exe', 'tanda4.exe', 'tanda5.exe', 'tanda6.exe', 'tanda7.exe', 'tanda8.exe', 'tanda9.exe', 'tanda10.exe', 'tanda11.exe', 'tanda12.exe', 'tanda13.exe', 'diario.exe', 'tanda14.exe', 'tanda4m.exe', 'tanda14b.exe', 'tanda15.exe', 'tanda16.exe', 'tanda17.exe', 'tanda18.exe', 'tanda19.exe', 'tanda19m.exe', 'tanda20.exe', 'tanda21.exe', 'tanda22.exe', 'tanda22d.dll', 'tanda23.exe', 'tanda24.exe', 'tanda25.exe', 'tanda26.exe', 'tanda27.exe', 'tanda28.exe', 'tanda29.exe', 'tanda30.exe', 'tanda31.exe', 'tanda32.exe', 'tanda33.exe', 'tanda34.exe', 'tanda35.exe', 'tanda36.exe', 'tanda37.exe', 'tanda38.exe', 'tanda39.exe', 'tanda41.exe', 'tanda42.exe', 'tanda43.exe', 'tanda44.exe', 'tanda45.exe', 'tanda46.exe', 'tanda47.exe', 'tanda48.exe', 'vueltas.exe', 'hwindow.exe', 'computo.exe', 'instancias.exe', 'vistas.exe', 'hdr.exe', 'uavpixel.exe', 'flotante1.exe', 'stencil.exe', 'olas.exe')) {
        Copy-Item (Join-Path (Get-Location) ('platform\shared\proton-x\prueba\' + $exe)) (Join-Path $dataBase ('window\' + $exe)) -Force
    }
    $leemeWin = @(
        'WINDOW -- los .exe de Windows (x86-64) que corre PROTON-X',
        '',
        '  run sys/proton-x.bex window/hola.exe           uno de aqui',
        '  run sys/proton-x.bex window/crt.exe -nivel 3   lo de detras es su linea de ordenes',
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

# -- Programas COBOL de ejemplo -----------------------------------
#
# Se compilan AQUI y salen al mismo staging que el compositor. Antes se
# generaban a mano dentro de toolchain/lang/cobol/examples/ y nunca llegaban al
# disco: `run apps/extracto.bex` contestaba "no esta: revisa la ruta" y parecia
# un fallo del cargador cuando el archivo sencillamente no se habia copiado.
#
# El nombre destino se recorta a 8.3 a proposito y de forma explicita: el
# driver FAT32 del kernel se NIEGA a recortar (un nombre recortado abre otro
# archivo), asi que el que no quepa se dice aqui y no en el arranque.
Step 'Building COBOL example programs...'
# Las rutas llevan el NIVEL delante: los ejemplos estan en escalera (1-basico,
# 2-decimal, 3-presentacion, 4-ficheros, 5-tablas), ordenados por cuanto COBOL
# hace falta que el compilador sepa. Ver examples\README.md.
# * POR NIVELES, y no en un monton. Los ejemplos estan en ESCALERA -cada uno
# pide una cosa mas que el anterior- y esa escalera se pierde si en el disco
# caen todos revueltos. Con niveles se puede VERIFICAR de uno en uno:
#
#     run cobol/1/hola.bex     y si eso va, subir
#     run cobol/2/banco.bex    y si eso va, subir
#     ...
#
# Y cuando algo se rompa, el orden dice por donde empezar a mirar: si falla el
# 10, comprobar primero que el 1 sigue vivo.
#
# [!] La carpeta es el NUMERO a secas y no el nombre largo. No es pereza: el
# driver FAT32 del kernel se NIEGA a recortar, y `3-presentacion` son trece
# letras. Un `n3presen` seria feo y no diria mas que un `3`; el nombre del nivel
# vive en examples\README.md, que es donde se lee.
$cobolEjemplos = @(
    @{ src = 'toolchain\lang\cobol\examples\1-basico\hola.cob';           out = 'hola.bex'     ; dir = 'cobol\1' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\banco.cob';         out = 'banco.bex'    ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\calc.cob';          out = 'calc.bex'     ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\calcgui.cob';       out = 'calcgui.bex'  ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\3-presentacion\extracto.cob'; out = 'extracto.bex' ; dir = 'cobol\3' },
    @{ src = 'toolchain\lang\cobol\examples\4-ficheros\batch.cob';        out = 'batch.bex'    ; dir = 'cobol\4' },
    # `conceptos` son nueve letras y el driver FAT32 se NIEGA a recortar, asi
    # que el destino es `concep`. La comprobacion de 8.3 de abajo lo cazaria
    # igual, pero mejor no llegar a que la cace.
    @{ src = 'toolchain\lang\cobol\examples\5-tablas\conceptos.cob';      out = 'concep.bex'   ; dir = 'cobol\5' },
    @{ src = 'toolchain\lang\cobol\examples\6-condiciones\cartera.cob';   out = 'carter.bex'   ; dir = 'cobol\6' },
    @{ src = 'toolchain\lang\cobol\examples\7-empaquetado\cuentas.cob';   out = 'cuentas.bex'  ; dir = 'cobol\7' },
    @{ src = 'toolchain\lang\cobol\examples\8-parrafos\cierre.cob';       out = 'cierre.bex'   ; dir = 'cobol\8' },
    @{ src = 'toolchain\lang\cobol\examples\9-decision\comision.cob';     out = 'comisio.bex'  ; dir = 'cobol\9' },
    @{ src = 'toolchain\lang\cobol\examples\10-binario\maestro.cob';      out = 'maestro.bex'  ; dir = 'cobol\10' },
    # BANK CAT (03-10): el motor del libro, con la LIBRERIA de copybooks (`COPY`).
    @{ src = 'toolchain\lang\cobol\examples\11-bankcat\libro.cob';      out = 'libro.bex'    ; dir = 'cobol\11' }
)
# -- Programas ADA de ejemplo -------------------------------------
#
# Crates PROPIOS, sin dependencia de los otros frontends: `bmo-ada-front` analiza
# y `bmo-ada-x86-64` (su `emisor-x86_64/`, partido el 2026-09-18) emite. El
# decimal exacto es el mismo que el de COBOL y no por copia: el Annex F de Ada
# copio las reglas de COBOL, asi que dos lenguajes que dicen lo mismo acaban en
# la misma aritmetica de enteros escalados.
$adaEjemplos = @(
    @{ src = 'toolchain\lang\ada\examples\1-basico\cierre.adb'; out = 'cierre.bex' ; dir = 'ada' }
)
# -- Programas C++ de ejemplo -------------------------------------
#
# ** 2026-09-17: el primer .bex de C++ que llega al disco. C++ estuvo APARCADO
# desde el 12-08 con dos filas rojas que resultaron ser del ARNES de pruebas y
# no del compilador, y su linea de ordenes tomaba `-o` como nombre de fichero.
# Este ejemplo encontro ademas que un derivado no llamaba al destructor de su
# base. Ver toolchain/lang/cpp/APARCADO.md, seccion 7.
$cppEjemplos = @(
    @{ src = 'toolchain\lang\cpp\examples\1-clases\cuentas.cpp'; out = 'cuentas.bex' ; dir = 'cpp' }
)
# -- Programas C de ejemplo ---------------------------------------
#
# * Este paso NO EXISTIA. COBOL y Ada llegaban al disco y C no, asi que los
# ejemplos de C solo se podian ejecutar si alguien los embebia a mano en el
# kernel -- y `scroll_C.bex` no llegaba al Kingston por eso, no por un fallo del
# compilador. Un lenguaje que compila y cuyo binario no se despliega esta a
# medias.
#
# `hola_C.c` prueba lo basico (bucles, %d, resta con signo, switch, %s).
# `scroll_C.c` usa las cabeceras `<bmo/...>`: la puerta de syscalls desde C, la
# capability de entrada y el modelo de scroll.
# `memoria_C.c` ESTRENA `KIND_MEMORIA`: pide, escribe, relee y agota el tope de
# cuatro peticiones. Es el unico programa que ejerce la capability de memoria.
$cEjemplos = @(
    @{ src = 'toolchain\lang\c\examples\hola_C.c';   out = 'holac.bex'  ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\scroll_C.c'; out = 'scrollc.bex' ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\pregunta_C.c'; out = 'pregc.bex'  ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\memoria_C.c'; out = 'memc.bex'   ; dir = 'c' },
    # ** EL SELLO (W^X, 2026-09-23): escribe `mov eax,42; ret`, lo sella, lo
    # llama (42) y al final escribe en el codigo sellado A PROPOSITO: lo
    # correcto es que muera ahi con un fallo de pagina de Ring 3.
    @{ src = 'toolchain\lang\c\examples\sello_C.c';  out = 'sello.bex'  ; dir = 'c' },
    # ** LA SONDA: el unico programa que usa la superficie MAL a proposito.
    # Handles inventados, operaciones que no existen, el renglon de ruta
    # inundado, el tope de memoria forzado, medidas imposibles. Cada empujon
    # tiene UNA respuesta correcta --el kernel dice que no y sigue vivo-- y que
    # el programa llegue a imprimir su recuento ya es media prueba.
    @{ src = 'toolchain\lang\c\examples\sonda_C.c';  out = 'sonda.bex'  ; dir = 'c' },
    # El ensayo general de DOOM: 2.5D en punto fijo sobre la pantalla real.
    @{ src = 'toolchain\lang\c\examples\raycaster_C.c'; out = 'ray.bex'    ; dir = 'c' },
    # ** EL BLOC DE NOTAS, y lo que estrena: ESCRIBIR dentro de una ventana.
    # Hasta el 11-09 una app recibia SCANCODES y nunca la LETRA que producian,
    # asi que no se podia teclear en una superficie sin copiarle la
    # distribucion castellana entera. Ahora el DIRECTOR reenvia el caracter ya
    # cocido --bit 62 del evento-- y esto solo lo lee. Abre `datos\notas.txt`,
    # se escribe encima y se guarda con el boton de su barra.
    # ** Y SE QUEDA EN `c\`, AUNQUE NO SEA UN EJEMPLO QUE SE CIERRA.
    #
    # Primero fue a `apps\`, porque el lanzador del escritorio lista `apps\` y
    # nada mas: ahi tendria icono, y un bloc de notas al que hay que llamar
    # escribiendo su ruta es un bloc de notas que no se usa. Y se deshizo
    # **porque `apps\` ya significa algo** y lo dice esta misma cabecera: lo
    # que alguien trae de FUERA del repo. El criterio es la procedencia, no si
    # el programa es util, y meter aqui una excepcion a la primera incomodidad
    # es como se deshacen las carpetas que se ordenaron una vez.
    #
    # Asi que hoy se lanza escribiendo `run c/texto.bex` en la caja del
    # escritorio, y lo que falta --que el lanzador mire algo mas que `apps\`--
    # esta escrito como casilla en `docs/plan/PLAN_DIRECTOR.md`. El icono va
    # dentro del `.bex` igual: es su cara, la lleve quien la lea o no.
    @{ src = 'toolchain\lang\c\examples\texto_C.c';     out = 'texto.bex'  ; dir = 'c' },
    # ** UNA APP SACA SU PROPIA IMAGEN DE DENTRO DE SI MISMA. Junta tres
    # piezas que existian por separado y nunca se habian usado juntas:
    # `paquete.h` (mi imagen, sin escribir ninguna ruta), `imagen.h` (el BICO,
    # a escala entera) y `superficie.h`. El icono que lee es EL MISMO recurso
    # que el escritorio pinta en la rejilla: el dato es uno, no hay copia.
    # Y se pinta sobre un TABLERO de cuadros a proposito -- el alfa es un bit,
    # y un fondo liso esconde que no se respete.
    @{ src = 'toolchain\lang\c\examples\imagen_C.c';    out = 'imagen.bex' ; dir = 'c' },
    # ** LA GUIA, y el texto NO esta en el programa: viaja como recurso.
    # Es la idea del `.datex` del propietario hecha con lo que el sistema ya
    # soporta -- para cambiar la guia se edita `guia.txt` y se reempaca, y el
    # programa no se toca ni se recompila.
    @{ src = 'toolchain\lang\c\examples\guia_C.c';      out = 'guia.bex'   ; dir = 'c' },
    # ** EL CUBO, Y SIN UN SOLO TRIANGULO. Lo pidio el propietario: *\"el triangulo
    # esta muy quemado\"*. Aqui el elemento que se dibuja es LA CAJA entera,
    # trazada por el metodo de las laminas: exacta a cualquier zoom, sin
    # vertices, sin recorte y con la normal saliendo de la interseccion.
    # Trae su propio metro (`[cubo] ... us`) porque cuesta por PIXEL.
    @{ src = 'toolchain\lang\c\examples\cubo_C.c';      out = 'cubo.bex'   ; dir = 'c' },
    # La prueba de fopen/fread/fseek. Lee `datos\salida.txt` DOS veces y
    # compara: si las dos lecturas coinciden, la cadena de ficheros funciona.
    @{ src = 'toolchain\lang\c\examples\leer_C.c';      out = 'leer.bex'   ; dir = 'c' },
    # ESTRENA `KIND_AUDIO`. Comprueba el CONTRATO y no el oido: que hay handle,
    # que el tope de duracion se cumple, que es exclusivo y --la que importa--
    # que el handle soltado ya NO pita. Puede que no se oiga nada y este todo
    # bien: el puerto del altavoz existe en todo x86, el zumbador no.
    @{ src = 'toolchain\lang\c\examples\sonido_C.c';    out = 'sonido.bex' ; dir = 'c' },
    # `<bmo/musica.h>`: notas por nombre, figuras y tempo. Se DIBUJA mientras
    # suena, porque puede que no suene -- si la placa no trae zumbador, la
    # pantalla es la unica prueba de que la cadena entera funciono.
    @{ src = 'toolchain\lang\c\examples\musica_C.c';    out = 'musica.bex' ; dir = 'c' },
    # ** EL PAQUETE: este `.bex` viaja con datos DENTRO y los lee sin escribir
    # ninguna ruta -- le pide al kernel su propia imagen. Se empaqueta justo
    # despues de compilarlo, ver `$cRecursos`.
    @{ src = 'toolchain\lang\c\examples\caja_C.c';      out = 'caja.bex'   ; dir = 'c' },
    # ** UNA PIEZA DE VERDAD sobre `KIND_AUDIO`: el ritornello de "La primavera"
    # de Vivaldi (1725, dominio publico). `musica.bex` prueba la libreria nota a
    # nota; esta prueba la PIEZA -- que ocho compases seguidos no deriven, y que
    # el eco forte/piano que Vivaldi escribio salga por `BMO_SONIDO_VOLUMEN`.
    @{ src = 'toolchain\lang\c\examples\vivaldi_C.c';   out = 'vivaldi.bex'; dir = 'c' },
    # ** EL NUMERO QUE NO EXISTIA: cuantos ciclos vale una puerta.
    # Compara bucle vacio, llamada normal, `INVOKE` pelado sobre la tarea
    # actual, e `INVOKE` sobre un handle de verdad. Se queda con el MINIMO,
    # porque el temporizador expropia y una media se puede inflar. Decide si
    # algo puede pasar por la superficie o tiene que ser codigo enlazado --
    # empezando por el runtime de Python. Ver `docs/maestro/PYTHON_MAESTRO.md`.
    @{ src = 'toolchain\lang\c\examples\coste_C.c';     out = 'coste.bex'  ; dir = 'c' },
    # *** DE QUE ESTA HECHA UNA PUERTA: parte los ticks en FIJO --lo que
    # cuesta cruzar y volver-- y TRABAJO --lo que se pidio--, midiendo una
    # puerta que el kernel RECHAZA. Un rechazo recorre la maquina entera y no
    # hace nada, asi que ES el fijo, medido y no estimado.
    #
    # ** Y con eso proyecta la tabla del LOTE: si una puerta llevara N
    # operaciones, el fijo se divide entre N y el trabajo no. Es lo unico que
    # puede bajar la fila `puerta` de `presupuesto.rs` a su meta de 300 sin
    # quitarle nada al trabajo. Ver `docs/plan/PLAN_LA_PUERTA_SE_PARTE.md`.
    @{ src = 'toolchain\lang\c\examples\ciclos_C.c';    out = 'ciclos.bex' ; dir = 'c' },
    # LA MEDIDA DEL BLIT: memcpy a RAM contra memcpy al framebuffer (WC), y
    # un bucle de 8 bytes como tercera fila. Ver su cabecera.
    @{ src = 'toolchain\lang\c\examples\blit_C.c';      out = 'blit.bex'   ; dir = 'c' }
)

# * LOS RECURSOS QUE VAN DENTRO DE UN `.bex`.
#
# Se meten DESPUES de compilar, y por eso es un paso aparte y no una opcion del
# compilador: el codigo lo emite el frontend, los datos llegan de quien monta la
# app. Ver `toolchain	oolsmo-pack`.
#
# ** Los bytes van escritos AQUI y no en un fichero suelto a proposito: el
# programa comprueba su contenido (`1..8`), asi que si esta lista y el ejemplo
# se separan, la prueba lo dice en vez de pasar por casualidad.
$cRecursos = @(
    @{ bex = 'c\caja.bex'; recursos = @(
        @{ nombre = 'saludo.txt'; texto = 'hola desde dentro de la caja' },
        @{ nombre = 'cuenta.bin'; bytes  = @(1,2,3,4,5,6,7,8) }
    ) }
    # ** LA CARA DEL BLOC DE NOTAS. Por el mismo camino que la de DOOM --un
    # recurso del paquete-- asi que el escritorio no necesita ni un acceso
    # directo ni una cache de iconos: el `.bex` va con su cara dentro.
    #
    # Una hoja con su barra azul arriba, que es lo que el programa muestra.
    # ** EL ICONO DE `imagen.bex` TIENE AGUJEROS A PROPOSITO. Un rombo deja
    # las cuatro esquinas transparentes, y eso es lo que hace que la prueba
    # del alfa sea una prueba: sobre el tablero de cuadros, las esquinas
    # tienen que dejar ver los cuadros.
    # ** LA GUIA: su TEXTO y su CARA, los dos dentro del mismo fichero.
    # ** Y LA TEXTURA DEL CUBO ES SU PROPIO ICONO. El mismo recurso que el
    # escritorio pintaria en la rejilla se ve pegado en sus caras: el dato es
    # uno, y un cubo con un cubo dibujado encima es exactamente lo que es.
    @{ bex = 'c\cubo.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '................',
            '....oooooooo....',
            '...oWWWWWWWWo...',
            '..oWWWWWWWWWWo..',
            '.oooooooooooooo.',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.obbbbbboggggggo',
            '.oooooooooooooo.',
            '................',
            '................'
        ) }
    ) }
    @{ bex = 'c\guia.bex'; recursos = @(
        @{ nombre = 'guia.txt'; desde = 'toolchain\lang\c\examples\guia.txt' },
        @{ nombre = 'icono'; icono = @(
            '................',
            '..oooooooooooo..',
            '..oWWWWWWWWWWo..',
            '..oWggggggggWo..',
            '..oWWWWWWWWWWo..',
            '..oWggggggWWWo..',
            '..oWWWWWWWWWWo..',
            '..oWggggggggWo..',
            '..oWWWWWWWWWWo..',
            '..oWgggggWWWWo..',
            '..oWWWWWWWWWWo..',
            '..oWWWbbbbWWWo..',
            '..oWWWbWWbWWWo..',
            '..oWWWWWbbWWWo..',
            '..oWWWWWbWWWWo..',
            '..oooooooooooo..'
        ) }
    ) }
    @{ bex = 'c\imagen.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '.......oo.......',
            '......obbo......',
            '.....obbbbo.....',
            '....obbbbbbo....',
            '...obbbWWbbbo...',
            '..obbbWWWWbbbo..',
            '.obbbWWWWWWbbbo.',
            'obbbWWWWWWWWbbbo',
            'obbbWWWWWWWWbbbo',
            '.obbbWWWWWWbbbo.',
            '..obbbWWWWbbbo..',
            '...obbbWWbbbo...',
            '....obbbbbbo....',
            '.....obbbbo.....',
            '......obbo......',
            '.......oo.......'
        ) }
    ) }
    @{ bex = 'c\texto.bex'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '................',
            '...oooooooooo...',
            '...obbbbbbbbo...',
            '...oWWWWWWWWo...',
            '...oWggggggWo...',
            '...oWWWWWWWWo...',
            '...oWgggggWWo...',
            '...oWWWWWWWWo...',
            '...oWgggggggo...',
            '...oWWWWWWWWo...',
            '...oWggggWWWo...',
            '...oWWWWWWWWo...',
            '...oWgggggWWo...',
            '...oWWWWWWWWo...',
            '...oooooooooo...',
            '................'
        ) }
    ) }
    # ** LA CARA DE NAVEGAR: una antena (el mastil y sus ondas) sobre la
    # pantalla que la muestra. Es un `.ibx` y se empaqueta por el mismo camino:
    # el mismo formato, el mismo cargador, la misma rejilla del escritorio.
    @{ bex = 'apps\navegar.ibx'; recursos = @(
        @{ nombre = 'icono'; icono = @(
            '.......bb.......',
            '.....bb..bb.....',
            '....b..bb..b....',
            '....b.b..b.b....',
            '.......oo.......',
            '.......oo.......',
            '..oooooooooooo..',
            '..oWWWWWWWWWWo..',
            '..oWggggggggWo..',
            '..oWWWWWWWWWWo..',
            '..oWgggggWWWWo..',
            '..oWWWWWWWWWWo..',
            '..oooooooooooo..',
            '.....oooooo.....',
            '....oooooooo....',
            '................'
        ) }
    ) }
)

# * EL FORMATO `BICO`, escrito aqui porque aqui es donde nace un icono.
#
#     0..4   "BICO"      4..6  ancho (u16)     6..8  alto (u16)
#     8..    ancho*alto pixeles BGRA, u32 little-endian
#
# 16x16 y el escritorio lo pinta al doble. Se guarda chico a proposito: la
# gracia de meter el icono en el paquete es que **no cueste nada llevarlo**, y
# un icono que engorda la app es un icono que alguien acabara quitando. 16x16
# son 1032 bytes; a 32x32 serian 4104.
#
# Los iconos se escriben como DIBUJO y no como una lista de numeros. Una rejilla
# de dieciseis lineas se lee, se corrige y se ve mal cuando esta mal; un array
# de 256 enteros no. Es el mismo criterio que `ring0\core\gato.rs`.
#
#   .  transparente (alfa 0: el escritorio se ve a traves)
#   o  contorno oscuro   R  rojo   d  rojo oscuro   W  blanco
#
# [!] Los colores van como **cuatro bytes en el orden del fichero (B, G, R, A)**
# y no como un `0xAARRGGBB`, por dos razones y la segunda escuece:
#
#   1. Asi la tabla dice el orden de bytes que se escribe, en vez de obligar a
#      recordar que un `u32` little-endian se guarda al reves de como se lee.
#   2. **PowerShell 5.1 lee `0xFFB4342A` como un `Int32` NEGATIVO** (-4967382) y
#      el cast a `uint32` revienta con "valor demasiado grande o demasiado
#      chico". Sin suffijo `u` en esta version, cualquier color con el alfa a
#      `FF` cae en la trampa -- o sea todos los opacos.
$BICO_PALETA = @{
    '.' = @(0x00, 0x00, 0x00, 0x00)
    'o' = @(0x10, 0x10, 0x1B, 0xFF)
    'R' = @(0x2A, 0x34, 0xB4, 0xFF)
    'd' = @(0x16, 0x1C, 0x6B, 0xFF)
    'W' = @(0xE0, 0xE6, 0xF0, 0xFF)
    # Los dos del bloc de notas: el azul de su barra y el gris de sus renglones.
    'b' = @(0xFF, 0xA6, 0x58, 0xFF)
    'g' = @(0x90, 0x83, 0x76, 0xFF)
}

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
    # ** `run inti/bico.ibx` (2026-09-12): la primera HERRAMIENTA en INTI.
    # Convierte datos/foto.bmp y datos/foto.qoi a BICO, y se generan aqui abajo.
    Compilar-Ejemplos @(
        @{ src = 'toolchain\lang\inti\sondas\cpu.inti'; out = 'cpu.ibx'; dir = 'inti' },
        @{ src = 'toolchain\lang\inti\sondas\pulso.inti'; out = 'pulso.ibx'; dir = 'inti' },
        # ** `run inti/ventana.ibx` (2026-09-17): la sonda de la VENTANA. Pinta una
        # superficie y escribe por consola lo que LEE de vuelta (pixeles, y el
        # buzon que el DIRECTOR le escribe): separa "INTI escribe en otro sitio"
        # de "el DIRECTOR lee otra memoria". Nacio del blanco de NAVEGAR (N1).
        @{ src = 'toolchain\lang\inti\sondas\ventana.inti'; out = 'ventana.ibx'; dir = 'inti' },
        @{ src = 'toolchain\lang\inti\ejemplos\bico.inti'; out = 'bico.ibx'; dir = 'inti' },
        # ** `run inti/musica.ibx [datos/x.mus]` (2026-09-13): el REPRODUCTOR.
        # Suena por el audifono USB (el altavoz de esta placa no suena).
        @{ src = 'toolchain\lang\inti\ejemplos\musica.inti'; out = 'musica.ibx'; dir = 'inti' },
        # ** VERRANO (26-09): el cubo contado por INTI en la CPU, publicado en
        # una LAMINA que el escritorio lee sin esperar. `run inti/cubo.ibx` y
        # despues `gpu verrano banco inti`.
        @{ src = 'toolchain\lang\inti\ejemplos\cubo.inti'; out = 'cubo.ibx'; dir = 'inti' },
        # ** NAVEGAR v0 (2026-09-16): la cara de la LAMINA, en INTI y con icono
        # en el escritorio. Hoy solo el mensaje --hace falta una ANTENA-- porque
        # INTI aun no abre ventana (N0 de docs/plan/PLAN_NAVEGAR.md). Va a
        # `apps/`, con DOOM, porque es una APP y no una sonda.
        @{ src = 'Ultra_userspace\apps\navegar\navegar.inti'; out = 'navegar.ibx'; dir = 'apps' }
    ) 'bmo-inti-x86-64' 'inti' 'ok:|error|aviso' $dataBase $repo

    Step 'Building TITAN++ programs...'
    # ** `run titan/hola.bex` (T3, 2026-10-04): el PRIMER .bex de TITAN++, y
    # escribe en la consola (decidido el 30-09; la ventana, despues). Su propio
    # emisor --no el de INTI--, la puerta de `bmo-lower` y el gate con
    # manifiesto. `.bex` y no `.ibx`: el `.ibx` es el compromiso de INTI.
    Compilar-Ejemplos @(
        @{ src = 'toolchain\lang\titan\ejemplos\nivel0\hola.titan'; out = 'hola.bex'; dir = 'titan' },
        @{ src = 'toolchain\lang\titan\ejemplos\nivel0\dos_saludos.titan'; out = 'dos.bex'; dir = 'titan' }
    ) 'bmo-titan-x86-64' 'titan' 'ok:|T00|no se ha' $dataBase $repo -Orden 'build'

    # -- Las dos imagenes que `bico.ibx` convierte ------------------------
    #
    # Se GENERAN y no se copian: un binario en el repo es un fichero que nadie
    # puede leer en un diff. 16x16 las dos, con un degradado que se reconoce a
    # simple vista -- si el conversor invirtiera las filas, se veria.
    $imgDst = Join-Path $dataBase 'datos'
    New-Item -ItemType Directory -Force $imgDst | Out-Null
    [System.IO.File]::WriteAllBytes((Join-Path $imgDst 'foto.bmp'), (Nuevo-Bmp))
    [System.IO.File]::WriteAllBytes((Join-Path $imgDst 'foto.qoi'), (Nuevo-Qoi))
    Write-Host '    [datos] foto.bmp y foto.qoi (16x16, para inti/bico.ibx)' -ForegroundColor DarkGray
    # ** Y un PNG y un JPEG de verdad (2026-09-20), para el visor: son los
    # ficheros con los que `bmo-imagen` se prueba en el anfitrion contra
    # Pillow, asi que lo que se ve en el Ryzen es lo que el banco ya juzgo.
    # Binarios chicos que viven en `pruebas/` de la crate, no aqui.
    $imgPruebas = Join-Path $repo 'platform\shared\bmo-imagen\pruebas'
    Copy-Item (Join-Path $imgPruebas 'inti256.png') (Join-Path $imgDst 'inti.png') -Force
    Copy-Item (Join-Path $imgPruebas 'foto.jpg') (Join-Path $imgDst 'arranque.jpg') -Force
    Write-Host '    [datos] inti.png (256x256 RGBA) y arranque.jpg (640x362 4:2:0), para el visor' -ForegroundColor DarkGray
    # La melodia que `musica.ibx` toca si no le dan otra. Es TEXTO: esta en el
    # repo y se lee en un diff.
    Copy-Item (Join-Path $repo 'toolchain\lang\inti\ejemplos\tema.mus') (Join-Path $imgDst 'tema.mus') -Force
    Write-Host '    [datos] tema.mus (Vivaldi, para inti/musica.ibx)' -ForegroundColor DarkGray
    # La lamina de example.com, la misma que sirve la antena: NAVEGAR la pinta
    # si esta (N2 de PLAN_NAVEGAR). Texto, en el repo, y se lee en un diff.
    # [!] 8.3: el FAT32 de BMO-X busca por nombre corto y se salta las
    # entradas de nombre largo, asi que `ejemplo.lamina` no lo encontraria.
    Copy-Item (Join-Path $repo 'toolchain\tools\antena\ejemplo.lamina') (Join-Path $imgDst 'ejemplo.lam') -Force
    Write-Host '    [datos] ejemplo.lam (example.com, para apps/navegar.ibx)' -ForegroundColor DarkGray
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
        '  ..\datos\salida.txt   el informe ENTERO (7 capitulos), como siempre',
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
        'Cada `save` REESCRIBE estas hojas. Lo que haya que conservar, se copia fuera.'
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
    $datosDst = Join-Path $dataBase 'datos'
    foreach ($d in (Get-ChildItem -LiteralPath $datosSrc -Filter '*.txt')) {
        Copy-Item -LiteralPath $d.FullName -Destination (Join-Path $datosDst $d.Name) -Force
        Write-Host ('    [datos] ' + $d.Name + ' (' + $d.Length + ' B)') -ForegroundColor DarkGray
    }
} finally { Pop-Location }
