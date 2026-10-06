param(
    [switch]$Clean,
    [switch]$Flash,
    [switch]$Verify,
    [switch]$BuildOnly,
    [string]$Drive = 'D',
    # Letra del volumen de DATOS (BMO-DATA) al que copiar los programas de
    # Ring 3. Vacio = no se toca ningun disco, que es el valor por defecto y la
    # postura de este build: escribir en discos esta cerrado salvo que se pida.
    [string]$Data = '',
    # * Las DOS mitades del despliegue a la misma unidad, que es el caso normal
    # en esta maquina. Equivale a `-Flash -Data <la misma letra de -Drive>`.
    #
    # Existe porque `-Flash` y `-Data` separados tienen una trampa silenciosa:
    # `-Flash` actualiza el ARRANQUE y `-Data` los PROGRAMAS, y quien olvida el
    # segundo arranca un kernel nuevo con un `sys\d.bex` viejo. No falla nada:
    # simplemente estas probando el build de antes y no lo sabes. Paso una tarde
    # el 2026-08-04.
    #
    # Las dos banderas SIGUEN existiendo por separado a proposito -son dos
    # discos logicos con dos riesgos distintos-, pero el camino corto es el
    # correcto y por eso tiene nombre.
    [switch]$Todo,
    # No compilar DOOM aunque `BMO-externo\` este puesto.
    #
    # El paso ya se salta solo cuando el port no esta --es GPL y vive fuera del
    # repo-- asi que esta bandera es para el caso contrario: el port esta y no
    # apetece pagar las 56.465 lineas en cada vuelta del build.
    [switch]$SinDoom,
    # ** EL KERNEL DE MEDIDA: `--features metro_puerta`.
    #
    # Devuelve los dos `rdtsc` a `dispatch`, o sea el REPARTO de una puerta
    # entre su mitad Rust y el resto. Es lo unico que puede contestar donde se
    # van los ~945 ciclos, y lo que cuesta esta medido: **112 ciclos en CADA
    # puerta de CADA programa**, un 11%.
    #
    # [!] NO ES EL KERNEL QUE SE DEJA PUESTO. Se flashea, se corre
    # `sys/precio.bex`, se apunta el reparto y se vuelve al build normal. Un
    # instrumento que se queda puesto deja de ser una medida y pasa a ser un
    # peaje -- que es exactamente por lo que se retiro el 16-08.
    [switch]$Metro,
    # ** LAS TRES VENTANAS (2026-10-03, `bmo.ps1 -Paralelo`). La ventana
    # COMPILAR corre este build con `-SinGuardianes`: los guardianes y el
    # contrato los corre a la vez la ventana VERIFICAR. A secas no cambia nada.
    [switch]$SinGuardianes,
    # Y la principal, cuando las dos acaban bien, corre `-SoloDiscos`: lleva al
    # disco lo que la ventana COMPILAR dejo en `staging`, SIN compilar otra
    # vez. Las comprobaciones de `build\discos.ps1` (la letra, el FAT32, el
    # techo, el hash de cada copia) son las mismas: no se salta ninguna.
    [switch]$SoloDiscos,
    [switch]$Yes
)

# `-Todo` se resuelve a las dos banderas de siempre ANTES de nada, para que
# todo lo de abajo no tenga que saber que existe.
if ($Todo) {
    $Flash = $true
    if (-not $Data) { $Data = $Drive }
}

$root = $PSScriptRoot
if (-not $root) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }
# La hora de arranque, para poder distinguir despues lo que este build produjo
# de lo que se quedo de otro. Ver el guardian de fantasmas del final.
$buildStart = Get-Date
# Lo que dejo un `exit` de un fichero cargado con punto (ver `Fail` en
# build\comun.ps1: alli esta el por que).
$script:salida = $null

# El reloj por etapa, `Step`, `Tiempos`, `Fail`, `Hash256` y `Guardian`: en
# `build\comun.ps1` desde el 2026-10-03, porque las tres ventanas de
# `bmo.ps1 -Paralelo` hablan igual.
. (Join-Path $PSScriptRoot 'build\comun.ps1')

# ** UN OBRERO SE CONSTRUYE UNA VEZ Y SE LLAMA MUCHAS (2026-09-21).
#
# El build llamaba `cargo run -p <crate> -- ...` por CADA programa de ejemplo:
# 20 de C por dos (compilar + enlazar), 12 de COBOL, los de INTI, los `pack`...
# unas 50 veces. Cada `cargo run` resuelve el workspace entero antes de decidir
# que no hay nada que compilar: 0,3-0,4 s por llamada, 15 s del build sin que
# nada se compile. `Obrero` construye el crate UNA vez (`cargo build`, que
# recompila si el fuente cambio, igual que `run`) y devuelve el ejecutable que
# cargo dice haber producido -- del propio mensaje de cargo, no de una lista de
# nombres de binarios.
$script:obreros = @{}
function Obrero {
    param([string]$crate)
    if ($script:obreros.ContainsKey($crate)) { return $script:obreros[$crate] }
    $antes = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $lineas = cargo build -p $crate --message-format=json-render-diagnostics 2>&1
    $codigo = $LASTEXITCODE
    $ErrorActionPreference = $antes
    if ($codigo -ne 0) {
        $lineas | Where-Object { "$_" -notmatch '^\{' } | ForEach-Object { Write-Host ('    ' + $_) -ForegroundColor Red }
        Fail ('no compilo el obrero ' + $crate)
    }
    $exe = $null
    foreach ($l in $lineas) {
        $t = "$l"
        if (-not $t.StartsWith('{')) { continue }
        try { $m = $t | ConvertFrom-Json } catch { continue }
        if ($m.reason -eq 'compiler-artifact' -and $m.executable -and ($m.target.kind -contains 'bin') -and $m.package_id -match ('(^|[ /#])' + [regex]::Escape($crate) + '[@# ]')) {
            $exe = $m.executable
        }
    }
    if (-not $exe) { Fail ('cargo construyo ' + $crate + ' y no dijo donde esta su ejecutable') }
    $script:obreros[$crate] = $exe
    return $exe
}


# ===========================================================================
#  EL ESPEJO: lo que hay EN EL DISCO contra lo que acaba de salir del build
# ===========================================================================
#
# ** Esto existe por un fallo concreto y caro: el 2026-08-04 se desplego con
# `-Flash` y sin `-Data`, o sea que se actualizo el ARRANQUE y no los
# PROGRAMAS. La maquina arranco un kernel nuevo con un `sys\d.bex` de dos
# commits antes. **No fallo nada** -- simplemente se estuvo probando el build de
# ayer, y las conclusiones de esa tarde eran sobre codigo que ya no existia.
#
# Un deploy incompleto que no dice nada es peor que uno que revienta: el que
# revienta se arregla en un minuto; este manda a depurar un fantasma.
#
# Solo LEE el disco. No copia, no borra y no puede fallar el build por lo que
# encuentre: informa. Si la unidad no esta puesta, se calla.
function Espejo {
    param([string]$letra)
    if (-not $letra) { return }
    if ($letra -and (Test-Path ($letra + ':\'))) {
        $espejoSrc = Join-Path $root 'staging\BMO-DATA'
        if (Test-Path $espejoSrc) {
            Write-Host ('  === ESPEJO: ' + $letra + ':\ contra este build ===') -ForegroundColor Cyan
            $viejos = 0
            $faltan = 0
            $aldia  = 0
            foreach ($f in Get-ChildItem -Path $espejoSrc -Recurse -File) {
                $rel = $f.FullName.Substring($espejoSrc.Length).TrimStart([char]'\')
                $enDisco = Join-Path ($letra + ':\') $rel
                if (-not (Test-Path -LiteralPath $enDisco)) {
                    Write-Host ('    FALTA    ' + $rel) -ForegroundColor Red
                    $faltan++
                } elseif ((Hash256 $enDisco) -ne (Hash256 $f.FullName)) {
                    # El medida se muestra porque es lo que se compara a ojo cuando
                    # uno mira el disco desde fuera.
                    $dl = (Get-Item -LiteralPath $enDisco).Length
                    $sl = $f.Length
                    Write-Host ('    VIEJO    {0}   disco {1} B / build {2} B' -f $rel, $dl, $sl) -ForegroundColor Red
                    $viejos++
                } else {
                    $aldia++
                }
            }
            Write-Host ''
            if ($viejos -eq 0 -and $faltan -eq 0) {
                Write-Host ('  EL DISCO ESTA AL DIA (' + $aldia + ' archivos)') -ForegroundColor Green
            } else {
                # En rojo y con el comando dentro. Un aviso que no dice como
                # arreglarlo obliga a recordar la bandera que uno acaba de olvidar.
                Write-Host '  *********************************************************' -ForegroundColor Red
                Write-Host ('  *  EL DISCO NO TIENE ESTE BUILD: {0} viejos, {1} sin copiar' -f $viejos, $faltan) -ForegroundColor Red
                Write-Host '  *' -ForegroundColor Red
                Write-Host '  *  Lo que arranques NO es lo que acabas de compilar.' -ForegroundColor Red
                Write-Host ('  *  Arreglo:  .\Ultra_kernel_x86-64\build.ps1 -Todo -Drive ' + $letra + ' -Yes') -ForegroundColor Yellow
                Write-Host '  *********************************************************' -ForegroundColor Red
            }
            Write-Host ''
        }
    }
}


$target = Join-Path $root 'target'
$stage  = Join-Path $root (Join-Path 'staging' 'EFI\BOOT')

# ** `-SoloDiscos`: lo compilado ya esta en `staging` (lo dejo la ventana
# COMPILAR); se comprueba que es de ESTA revision y se va derecho a los discos.
if ($SoloDiscos) {
    if ($Clean) { Fail '-SoloDiscos con -Clean borraria lo que se va a copiar' }
    if (-not ($Flash -or $Data)) { Fail '-SoloDiscos sin -Todo, -Flash ni -Data: no hay disco al que ir' }
    $manifiesto = Join-Path $stage 'BMO-MANIFEST.TXT'
    if (-not (Test-Path -LiteralPath $manifiesto) -or -not (Test-Path -LiteralPath (Join-Path $stage 'BOOTX64.EFI'))) {
        Fail 'no hay nada compilado en staging: la ventana COMPILAR no acabo (o no se corrio)'
    }
    # Un staging de otra revision es el fantasma del 04-08 al reves: copiar
    # al disco un build que no es el de ahora.
    $revision = (& git -C $root rev-parse --short=12 HEAD 2>$null)
    $enStaging = (Get-Content -LiteralPath $manifiesto | Where-Object { $_ -match '^revision=' }) -replace '^revision=', ''
    if ($revision -and $enStaging -ne $revision) {
        Fail ('staging es de la revision ' + $enStaging + ' y el arbol esta en ' + $revision + ': compila otra vez')
    }
    $deployFiles = @('BOOTX64.EFI', 'BMO-MANIFEST.TXT')
    . (Join-Path $PSScriptRoot 'build\discos.ps1')
    if ($null -ne $script:salida) { exit $script:salida }
    $espejoLetra = if ($Data) { $Data.TrimEnd([char]':', [char]'\').ToUpper() } else { $Drive.TrimEnd([char]':', [char]'\').ToUpper() }
    Espejo $espejoLetra
    Tiempos
    exit 0
}

if ($Clean) {
    Step 'Cleaning'
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    if (Test-Path $stage)  { Remove-Item $stage  -Recurse -Force }
}

Write-Host ''
Write-Host '  === BMO Ultra Kernel x86-64 Build (UEFI 5 layers + 2 stages + kernel) ===' -ForegroundColor Magenta
Write-Host ''

# ** LOS GUARDIANES Y EL CONTRATO, antes de compilar: en `build\guardianes.ps1`
# y `build\contrato.ps1`. Con `-SinGuardianes` no se corren AQUI porque los
# corre la ventana VERIFICAR a la vez (`bmo.ps1 -Paralelo`); a secas, todos.
if ($SinGuardianes) {
    Write-Host '  (los guardianes y el contrato los corre la ventana VERIFICAR, a la vez)' -ForegroundColor DarkGray
} else {
    . (Join-Path $PSScriptRoot 'build\guardianes.ps1')
    if ($null -ne $script:salida) { exit $script:salida }
    # -- CONTRATO (L6a: `build.ps1` se partio el 2026-08-28) --------
    . (Join-Path $PSScriptRoot 'build\contrato.ps1')
    if ($null -ne $script:salida) { exit $script:salida }
}
# NOTE: uefi_chain is now the UNIFIED shim -- it embeds the flat binaries
# of s1_cpu, s2_mem and the kernel via include_bytes!, so it is built
# AFTER them (see 'Building unified uefi_chain' below). Rationale: some
# firmwares (MSI A320M AMI fast path) never bind SimpleFileSystem, so
# the boot chain cannot read the ESP through UEFI protocols at all.

# -- Build the 2 consolidated stages -------------------------------
$stages = @('s1_cpu', 's2_mem')
$idx = 0
foreach ($s in $stages) {
    Step (('[{0,2}/2] Building stage ' -f ($idx + 1)) + $s + '...')
    $stageDir = Join-Path (Join-Path $root 'faggin') $s
    Push-Location $stageDir
    try {
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
        $td = Join-Path $target (Join-Path 'faggin' $s)
        # Each faggin stage is a standalone crate, NOT part of any workspace.
        # We pass opt-level=z via --config so the resulting binary is
        # as small as possible. Each stage also has its own
        # .cargo/config.toml that points to its linker.ld and adds
        # -C relocation-model=static -C target-feature=+crt-static.
        # Cargo does not track linker.ld. Include its content hash in a harmless
        # linker symbol so script-only changes invalidate just the final crate.
        $linkerHash = (Get-FileHash (Join-Path $stageDir 'linker.ld') -Algorithm SHA256).Hash.Substring(0, 16)
        $out = cargo +nightly rustc --release --target x86_64-unknown-none --target-dir $td -- `
            -C ("link-arg=--defsym=BMO_LINKER_REV_" + $linkerHash + '=0') 2>&1
        $out | ForEach-Object {
            if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [' + $s + '] ' + $_) -ForegroundColor DarkGray }
        }
        if ($LASTEXITCODE -ne 0) { Fail ($s + ' build failed') }
    } finally { Pop-Location }
    $idx++
}

# -- Build Ring 3 userspace (Ultra_userspace/) ---------------------
#
# * El kernel YA NO EMBEBE el compositor. Antes lo metia con `include_bytes!` y
# este paso tenia que ir antes del kernel para que el blob estuviera al dia;
# ademas dejaba un binario de 24 KiB dentro de kernel/src/ que se reescribia en
# cada build y ensuciaba el repositorio en cada commit.
#
# Ahora sale a staging\BMO-DATA\apps\ y se COPIA al volumen de datos, igual que
# cualquier otro programa. El kernel lo arranca con `launch::ruta` despues de
# montar el disco (ver `phase::arrancar_escritorio`). Cambiar el escritorio ya
# no obliga a recompilar Ring 0.
#
# Ultra_userspace/ es su PROPIO workspace: se compila a x86_64-unknown-none con
# su guion de enlazado, que fija la base en USER_IMAGE_BASE. `bex-link` traduce
# el ELF a un contenedor BEF y comprueba, seccion por seccion, que las
# direcciones que escribio el enlazador son las que el kernel va a mapear.
Step 'Building Ring 3 userspace (DIRECTOR)...'
$usDir = Join-Path (Split-Path -Parent $root) 'Ultra_userspace'
if (-not (Test-Path $usDir)) { Fail 'Ultra_userspace/ no existe' }
Push-Location $usDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    # == ** Y `coste` SE CONSTRUYE AQUI, NO A MANO (2026-09-07) ==============
    #
    # Este paso compilaba SOLO el director, y veinte lineas mas abajo el build
    # EXIGIA un ELF de `medida/coste` que nadie construia. O sea que
    # `sys/precio.bex` dependia de que alguien lo hubiera compilado a mano
    # alguna vez, y de que nada lo hubiera tocado desde entonces.
    #
    # ** Y algo lo tocaba: cualquier `cargo` en ese workspace con otros flags
    # --un `cargo test` del banco, por ejemplo-- puede dejar ese artefacto sin
    # revalidar. El sintoma era `[X] no salio el ELF de medida/coste` seguido de
    # "28 .bex que este build NO ha producido", y ha pasado ya tres veces.
    #
    # *** La regla es la de la casa: un build que EXIGE algo tiene que
    # CONSTRUIRLO. Exigir sin construir es un guardian que se queja del usuario
    # en vez de hacer su trabajo.
    # ** Y `sombra` (2026-09-23): S5 del SOMBREADOR, SPIR-V traducido y
    # SELLADO dentro de BMO-X. Mismo motivo: si el build la exige, la construye.
    # ** Y `proton-x` (2026-09-27): PROTON-X P1c, un .exe de Windows cargado,
    # SELLADO y ejecutado en Ring 3.
    # ** Y `estructura` (2026-09-29): F1, el taller como editor de nodos de
    # TITAN++ con el comprobador animado. El DIRECTOR lo LANZA con F1, asi que
    # si el build no lo construye, F1 no abre nada.
    # ** Y `ludoteca` (01-10): F4, tus juegos de todas las tiendas. Igual:
    # sin construirla, F4 no abre nada.
    # ** Y `hermes` (03-10): F3, H5 de PLAN_HERMES. Sin ella, F3 solo avisa.
    # ** Y `jueces` (06-10): los jueces de PROTON-X corridos SOLOS (`run sys/jueces.bex`).
    $out = cargo +nightly build -p bmo-service-director -p bmo-medida-coste -p bmo-medida-sombra -p bmo-app-proton-x -p bmo-app-taller -p bmo-app-ludoteca -p bmo-app-hermes -p bmo-app-bankcat -p bmo-app-jueces `
        --release --target x86_64-unknown-none 2>&1
    $out | ForEach-Object {
        if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [userspace] ' + $_) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'userspace build failed' }
} finally { Pop-Location }

$compositorElf = Join-Path $usDir 'target\x86_64-unknown-none\release\director'
if (-not (Test-Path $compositorElf)) { Fail 'no salio el ELF del DIRECTOR' }

# ** Y CUANTO BAJA CADA PROGRAMA POR SU PILA DE RING 3 (2026-09-28).
#
# El 28-09 `gpu verrano` tumbo la maquina: el `draw` del director pedia un
# marco de 395.432 B contra una pila de Ring 3 de 64 KiB. `pila.py --ring3`
# mide los cuatro programas de arriba como mide el kernel, y para el build si
# el camino desde `_start` no cabe o si un marco solo pasa de media pila.
Step 'Measuring how deep each Ring 3 program goes on its stack'
$pila3Py = Join-Path (Split-Path -Parent $root) 'toolchain/tools/pila/pila.py'
if (-not (Test-Path $pila3Py)) { Fail ('guardian MUERTO: falta ' + $pila3Py) }
$pila3Python = (Get-Command python -ErrorAction SilentlyContinue)
if ($pila3Python) {
    $env:PYTHONIOENCODING = 'utf-8'
    $pila3Salida = & $pila3Python.Source $pila3Py --check --ring3
    if ($LASTEXITCODE -ne 0) {
        $pila3Salida | ForEach-Object { Write-Host ('    ' + $_) -ForegroundColor Red }
        Fail 'a Ring 3 program does not fit its 64 KiB stack (see toolchain/tools/pila/pila.py --ring3)'
    }
    $pila3Salida | Where-Object { $_ -match 'clean:' } | ForEach-Object {
        Write-Host ('    ' + $_.Trim()) -ForegroundColor DarkGray
    }
} else {
    Write-Host '    [!] python no encontrado: no se mide la pila de Ring 3' -ForegroundColor Yellow
}
# El .bex sale a staging\BMO-DATA\apps\, que es el espejo de lo que hay que
# copiar al volumen de datos. La ruta de dentro (sys\d.bex) tiene que cuadrar
# con `RUTA_COMPOSITOR` de phase.rs: es el contrato entre el build y el arranque.
#
# ** `d.bex` y no `director.bex`: `director` son ocho exactos y CABRIA. Lo que
# decide es que con el escritorio muerto esto se teclea a mano; el por que
# entero, en `core/desktop.rs`. Y el 8.3 si manda en lo demas: el driver FAT32
# se NIEGA a recortar, porque un nombre recortado abre otro archivo.
# -- EJEMPLOS (L6a: `build.ps1` se partio el 2026-08-28) --------
. (Join-Path $PSScriptRoot 'build\ejemplos.ps1')
if ($null -ne $script:salida) { exit $script:salida }
# -- FIRMWARE DEL GSP (L0c): bajado una vez, verificado por SHA-256 ----
. (Join-Path $PSScriptRoot 'build\firmware.ps1')
if ($null -ne $script:salida) { exit $script:salida }

# -- Build kernel (Ring 0 base) ------------------------------------
Step 'Building kernel (Ring 0 base)...'
$stageDir = Join-Path $root 'kernel'
Push-Location $stageDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $kd = Join-Path $target 'kernel'
    $linkerHash = (Get-FileHash (Join-Path $stageDir 'linker.ld') -Algorithm SHA256).Hash.Substring(0, 16)
    # ** EL KERNEL DE MEDIDA VA A OTRO `--target-dir`, y no es un detalle.
    #
    # Con el mismo directorio, cargo reusa los objetos: se compila con el metro,
    # se vuelve a compilar sin el, y **el que queda es el ultimo que se pidio**
    # sin que nada en pantalla lo diga. Dos directorios significan que los dos
    # binarios existen a la vez y ninguno pisa al otro.
    $rasgos = @()
    if ($Metro) {
        $rasgos = @('--features', 'metro_puerta')
        $kd = Join-Path $target 'kernel-metro'
        Write-Host ''
        Write-Host '  ** KERNEL DE MEDIDA (--features metro_puerta) **' -ForegroundColor Yellow
        Write-Host '     Devuelve los dos rdtsc a `dispatch`: cada puerta cuesta ~112 ciclos MAS.' -ForegroundColor Yellow
        Write-Host '     Correr `sys/precio.bex`, apuntar el reparto, y VOLVER al build normal.' -ForegroundColor Yellow
        Write-Host ''
    }
    $out = cargo +nightly rustc --release @rasgos --target x86_64-unknown-none --target-dir $kd -- `
        -C ("link-arg=--defsym=BMO_LINKER_REV_" + $linkerHash + '=0') 2>&1
    $out | ForEach-Object {
        if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [kernel] ' + $_) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'kernel build failed' }
} finally { Pop-Location }

# -- ** CUANTO BAJA EL KERNEL POR CADA PILA DE TAREA (2026-09-21) ---
#
# Un syscall corre sobre la pila de kernel de 32 KiB de la tarea que lo pide,
# y por un syscall pasa el cargador entero. El 20-09 el camino de `LANZAR`
# bajaba 14.232 bytes de 16.384 y una interrupcion encima lo saco por el
# fondo: el PD del escritorio, en el marco vecino, aparecio a cero. Nadie lo
# vio porque el numero no existia. Ahora lo mide `toolchain/tools/pila/pila.py`
# sobre el ELF que se acaba de construir: el camino estatico mas hondo desde
# `syscall_entry` mas la puerta mas honda de la IDT tiene que caber con una
# pagina de margen -- y los topes los lee del fuente, no de una lista.
#
# [!] Va AQUI, sobre `$kd`, por lo mismo que el objcopy de abajo: con `-Metro`
# el kernel que se mide es el instrumentado, que es el que se va a flashear.
Step 'Measuring how deep the kernel goes on each task stack'
$pilaPy = Join-Path (Split-Path -Parent $root) 'toolchain/tools/pila/pila.py'
if (-not (Test-Path $pilaPy)) { Fail ('guardian MUERTO: falta ' + $pilaPy) }
$pilaPython = (Get-Command python -ErrorAction SilentlyContinue)
if ($pilaPython) {
    $pilaElf = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel'))
    $env:PYTHONIOENCODING = 'utf-8'
    $pilaSalida = & $pilaPython.Source $pilaPy --check --elf $pilaElf
    if ($LASTEXITCODE -ne 0) {
        $pilaSalida | ForEach-Object { Write-Host ('    ' + $_) -ForegroundColor Red }
        Fail 'a kernel stack does not hold its deepest path plus one interrupt (see toolchain/tools/pila/pila.py)'
    }
    $pilaSalida | Where-Object { $_ -match 'clean:' } | ForEach-Object {
        Write-Host ('    ' + $_.Trim()) -ForegroundColor DarkGray
    }
} else {
    Write-Host '    [!] python no encontrado: no se mide la pila del kernel' -ForegroundColor Yellow
}

# -- Embed payloads + build unified uefi_chain ---------------------
Step 'Preparing embedded payloads'
$embedDir = Join-Path $target 'embed'
New-Item -ItemType Directory -Path $embedDir -Force | Out-Null
$llvmObjcopyEmbed = Get-ChildItem -Path "$env:USERPROFILE\.rustup" -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName
if (-not $llvmObjcopyEmbed) { Fail 'llvm-objcopy not found for embedded payloads' }
$embedMap = @{}
foreach ($s in $stages) {
    $bn = $s -replace '_', '-'
    $td = Join-Path $target (Join-Path 'faggin' $s)
    $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' ($bn + '.exe')))
    if (-not (Test-Path $bin)) { $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' $bn)) }
    $outBin = Join-Path $embedDir ($s + '.bin')
    & $llvmObjcopyEmbed -O binary $bin $outBin
    if ($LASTEXITCODE -ne 0) { Fail ('objcopy (embed) failed for ' + $s) }
    $embedMap[$s] = $outBin
}
# ** SE LEE DE `$kd` Y NO DE 'kernel' A PELO, y eso es lo que hace utilizable el
# `-Metro`: con la ruta escrita a mano, un build de medida habria compilado el
# kernel instrumentado en `kernel-metro\` y **empaquetado el normal de al lado**,
# sin que una sola linea lo dijera. Se habria medido el build equivocado y el
# reparto habria salido en ceros -- con el disco ya flasheado.
$kernelElf = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel.exe'))
if (-not (Test-Path $kernelElf)) { $kernelElf = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel')) }
$kernelEmbed = Join-Path $embedDir 'kernel.bin'
& $llvmObjcopyEmbed -O binary $kernelElf $kernelEmbed
if ($LASTEXITCODE -ne 0) { Fail 'objcopy (embed) failed for kernel' }

Step 'Building unified uefi_chain (embedded stages)...'
$uefiDir = Join-Path $root 'uefi_chain'
Push-Location $uefiDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $env:BMO_S1_BIN = $embedMap['s1_cpu']
    $env:BMO_S2_BIN = $embedMap['s2_mem']
    $env:BMO_KERNEL_BIN = $kernelEmbed
    $uefiTarget = Join-Path $target 'uefi_chain'
    $out = cargo +nightly build --release --target x86_64-unknown-uefi --target-dir $uefiTarget 2>&1
    $out | ForEach-Object {
        if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [uefi_chain] ' + $_) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'uefi_chain build failed' }

    # ** EL EFI SE ENLAZA DOS VECES Y TIENE QUE SALIR IGUAL (2026-09-18). Hasta
    # ese dia dos builds del MISMO arbol daban un BOOTX64.EFI distinto en 10
    # bytes (hora y PDB), y nadie podia comprobar que el EFI publicado sale de
    # este fuente. `/Brepro` + `/DEBUG:NONE` en uefi_chain/.cargo/config.toml lo
    # arreglan; esto impide que vuelva sin ruido.
    $efiUno = Join-Path $uefiTarget 'x86_64-unknown-uefi\release\uefi_chain.efi'
    $huellaUno = Hash256 $efiUno
    # ** DOS trampas, las dos cazadas mutando (quitadas las dos opciones, esto
    # tiene que dar rojo -- y dio VERDE dos veces):
    #  1. Borrar el .efi NO re-enlaza: cargo guarda el enlazado en
    #     `build/uefi-chain/<hash>/out/` y solo lo vuelve a copiar. Se comparaba
    #     el mismo fichero consigo mismo. `cargo clean -p` borra ESE crate (es
    #     chico; los payloads ya estan hechos) y obliga a enlazar de nuevo.
    #  2. El sello del enlazador va en SEGUNDOS: sin la espera, dos enlaces en el
    #     mismo segundo coinciden aunque no sean reproducibles.
    Start-Sleep -Seconds 2
    $null = cargo +nightly clean --release --target x86_64-unknown-uefi --target-dir $uefiTarget -p uefi-chain 2>&1
    $null = cargo +nightly build --release --target x86_64-unknown-uefi --target-dir $uefiTarget 2>&1
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $efiUno)) { Fail 'uefi_chain: el segundo enlace fallo' }
    if ((Hash256 $efiUno) -ne $huellaUno) {
        Fail 'uefi_chain NO es reproducible: dos enlaces del mismo arbol dan un EFI distinto'
    }
    Write-Host ('    [uefi_chain] reproducible: dos enlaces, la misma huella ' + $huellaUno.Substring(0, 16)) -ForegroundColor DarkGray
} finally {
    Pop-Location
    Remove-Item Env:\BMO_S1_BIN, Env:\BMO_S2_BIN, Env:\BMO_KERNEL_BIN -ErrorAction SilentlyContinue
}

# -- ** LOS FANTASMAS DE `staging\` -------------------------------
#
# `staging\` NO SE LIMPIA entre builds, a proposito: rehacer el volumen entero
# cada vez cuesta minutos. El precio es que **un fichero que este build dejo de
# producir se queda ahi para siempre**, y no como basura inerte: TAPANDO al
# bueno.
#
# Paso de verdad el 2026-08-18. Los ejemplos de COBOL se reorganizaron de
# `cobol\` a `cobol\<escalon>\`, los planos del 3 de agosto se quedaron, y el
# escritorio --que lanzaba `cobol/calcgui.bex`-- siguio arrancando un motor de
# quince dias atras. Hablaba un protocolo anterior, la calculadora se quedo
# esperando una respuesta que no iba a llegar, y con las teclas suyas el Ryzen
# se quedo sin teclado.
#
# ** UN FANTASMA NO DA ERROR: CONTESTA. Por eso hace falta mirarlo aqui.
#
# La comprobacion es una RESTA, no una lista: cualquier `.bex` mas viejo que el
# arranque de este build es algo que ya no se produce. Un guardian con lista
# tendria el mismo fallo que vigila.
#
# ** Y "cualquier .bex" tambien era una lista, de UNA extension (2026-09-16):
# INTI paso de `.ibex` a `.ibx` el 13-09 y tres `.ibex` viejos se quedaron en
# staging tres dias, al lado de los nuevos, sin que este guardian los viera.
# Ahora mira todo lo que el escritorio puede LANZAR: .bex, .ibx y .ibex.
$fantasmas = @(Get-ChildItem -Path $dataBase -Recurse -File -ErrorAction SilentlyContinue |
    Where-Object { $_.Extension -in '.bex', '.ibx', '.ibex' -and $_.LastWriteTime -lt $buildStart })
if ($fantasmas.Count -gt 0) {
    Write-Host ('    [!] {0} ejecutable(s) en staging que este build NO ha producido:' -f $fantasmas.Count) -ForegroundColor Yellow
    foreach ($f in $fantasmas) {
        Write-Host ('        {0}' -f $f.FullName.Substring($dataBase.Length + 1)) -ForegroundColor Yellow
    }
    Write-Host '        Este build no los produce. Comprobar si alguno TAPA a uno bueno.' -ForegroundColor Yellow
} else {
    Write-Host '    staging: ningun ejecutable (.bex/.ibx/.ibex) sobrante de un build anterior' -ForegroundColor DarkGray
}

# -- ** QUE LE HIZO ESTE CAMBIO A LOS 25 PROGRAMAS ------------------
#
# Va AQUI y no arriba con los guardianes porque necesita los `.bex` ya escritos.
# Y no es un trinquete: un programa puede crecer con razon. REPORTA -- ver
# toolchain/tools/medida/medida.py, y el aviso de que el medida no es la
# velocidad esta en su cabecera.
# [!] NO va por `Guardian`: esa funcion solo muestra las lineas `clean:` y mata
# si el script devuelve distinto de cero. Este REPORTA, asi que se le deja
# hablar entero y nunca decide nada.
Step 'Comparing executable sizes with the baseline'
$medidaPy = Join-Path (Split-Path -Parent $root) 'toolchain/tools/medida/medida.py'
$medidaBin = (Get-Command python -ErrorAction SilentlyContinue)
if (-not (Test-Path $medidaPy)) { Fail ('guardian MUERTO: falta ' + $medidaPy) }
if ($medidaBin) {
    $env:PYTHONIOENCODING = 'utf-8'
    & $medidaBin.Source $medidaPy | ForEach-Object {
        Write-Host ('    ' + $_) -ForegroundColor DarkGray
    }
} else {
    Write-Host '    [!] python no encontrado: no se comparan las medidas' -ForegroundColor Yellow
}

# -- Validate outputs ----------------------------------------------
Step 'Validating outputs'
$uefi_chain = Join-Path $target (Join-Path 'uefi_chain' (Join-Path 'x86_64-unknown-uefi' (Join-Path 'release' 'uefi_chain.efi')))

$all_binaries = @($uefi_chain)
foreach ($s in $stages) {
    $binName = $s -replace '_', '-'
    $td = Join-Path $target (Join-Path 'faggin' $s)
    $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' ($binName + '.exe')))
    if (-not (Test-Path $bin)) { $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' $binName)) }
    $all_binaries += $bin
}
# El mismo `$kd` que se empaqueto: la tabla de medidas tiene que describir el
# binario que se acaba de meter en el `.efi`, no el que hubiera al lado.
$kernel = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel.exe'))
if (-not (Test-Path $kernel)) { $kernel = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel')) }
$all_binaries += $kernel

Write-Host ''
Write-Host '  bin                         linked size  load address' -ForegroundColor White
Write-Host '  --------------------------  -----------  ------------' -ForegroundColor DarkGray
$loadAddresses = @($null, [uint64]0x100000, [uint64]0x200000, [uint64]0x400000)
for ($binaryIndex = 0; $binaryIndex -lt $all_binaries.Count; $binaryIndex++) {
    $f = $all_binaries[$binaryIndex]
    if (-not (Test-Path $f)) { Fail ('Not found: ' + $f) }
    $raw = (Get-Item $f).Length
    $name = Split-Path $f -Leaf
    if ($name -eq 'uefi_chain.efi') {
        $line = ('  {0,-25} {1,9} B  firmware managed' -f $name, $raw)
    } else {
        $line = ('  {0,-25} {1,9} B  0x{2:X6}' -f $name, $raw, $loadAddresses[$binaryIndex])
    }
    Write-Host $line
}
Write-Host '  --------------------------  -----------  ------------' -ForegroundColor DarkGray
Write-Host ''

# -- Stage to ESP layout -------------------------------------------
# ESP layout -- UNA SOLA COSA:
#   $stage\BOOTX64.EFI        el shim unificado; lleva s1_cpu, s2_mem y el
#                             kernel EMBEBIDOS (include_bytes!), asi que es
#                             todo el sistema en un archivo
#   $stage\BMO-MANIFEST.TXT   revision + hash de lo desplegado
#
# Antes tambien se copiaban ring0\kernel.bin y ring0\faggin\s*.bin: los
# MISMOS binarios que el shim ya lleva dentro, ~575 KB de duplicado exacto.
# Eran el camino de respaldo por sistema de ficheros, que dejo de existir el
# dia del "EFI unificado" (esta placa no tiene driver FAT UEFI que conectar,
# por eso se embebio todo). Nadie los leia. Se fueron.
Step ('Staging to ' + $stage)
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null

Copy-Item $uefi_chain (Join-Path $stage 'BOOTX64.EFI')

$llvmObjcopy = Get-ChildItem -Path "$env:USERPROFILE\.rustup" -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName
if (-not $llvmObjcopy) { $llvmObjcopy = Get-ChildItem -Path "$env:USERPROFILE\.rustup\toolchains" -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName }
if ($llvmObjcopy) {
    # Flat binaries have no ELF entry metadata: the entry symbol itself must
    # be the first byte at the fixed load address.
    $llvmNm = Join-Path (Split-Path $llvmObjcopy -Parent) 'llvm-nm.exe'
    if (-not (Test-Path $llvmNm)) { Fail 'llvm-nm not found; cannot validate flat-binary entry points' }
    $entryChecks = @(
        @{ File = $all_binaries[1]; Symbol = 's1_entry'; Expected = [uint64]0x100000 },
        @{ File = $all_binaries[2]; Symbol = '_start';   Expected = [uint64]0x200000 },
        @{ File = $kernel;          Symbol = '_start';   Expected = [uint64]0x400000 }
    )
    foreach ($check in $entryChecks) {
        $pattern = '^[0-9A-Fa-f]+\s+\S\s+' + [regex]::Escape($check.Symbol) + '$'
        $line = & $llvmNm -n $check.File 2>&1 | Where-Object { $_ -match $pattern } | Select-Object -First 1
        if (-not $line) { Fail ('entry symbol not found: ' + $check.Symbol + ' in ' + $check.File) }
        $actual = [Convert]::ToUInt64(($line -split '\s+')[0], 16)
        if ($actual -ne $check.Expected) {
            Fail ('flat entry mismatch for ' + $check.Symbol + ': expected 0x{0:X}, got 0x{1:X}' -f $check.Expected, $actual)
        }
    }
    $stackChecks = @(
        @{ File = $all_binaries[2]; Symbol = 'S2_STACK_END' },
        @{ File = $kernel;          Symbol = 'KERNEL_STACK_END_MARKER' }
    )
    foreach ($check in $stackChecks) {
        $pattern = '^[0-9A-Fa-f]+\s+\S\s+' + [regex]::Escape($check.Symbol) + '$'
        $line = & $llvmNm -n $check.File 2>&1 | Where-Object { $_ -match $pattern } | Select-Object -First 1
        if (-not $line) { Fail ('stack symbol not found: ' + $check.Symbol) }
        $address = [Convert]::ToUInt64(($line -split '\s+')[0], 16)
        if (($address -band 0xF) -ne 0) { Fail ('stack is not 16-byte aligned: ' + $check.Symbol) }
    }
} else {
    Fail 'llvm-objcopy not found; the boot chain requires flat binaries and cannot load ELF files'
}

# Un solo archivo desplegado, un solo hash que verificar.
$deployFiles = @('BOOTX64.EFI')
$gitRevision = (& git -C $root rev-parse --short=12 HEAD 2>$null)
if (-not $gitRevision) { $gitRevision = 'unknown' }
$manifestLines = @(
    'BMO UEFI deployment manifest v1',
    ('revision=' + $gitRevision),
    'architecture=x86_64',
    'boot_chain=uefi_chain,s1_cpu,s2_mem,kernel'
)
foreach ($relativePath in $deployFiles) {
    $sourcePath = Join-Path $stage $relativePath
    $manifestLines += ('file={0}|bytes={1}|sha256={2}' -f `
        $relativePath, (Get-Item -LiteralPath $sourcePath).Length, (Hash256 $sourcePath))
}
$manifestPath = Join-Path $stage 'BMO-MANIFEST.TXT'
$manifestLines | Set-Content -LiteralPath $manifestPath -Encoding Ascii
$deployFiles += 'BMO-MANIFEST.TXT'

Write-Host ''
Write-Host '  === BUILD COMPLETE ===' -ForegroundColor Green
Write-Host ('  Staged to: ' + $stage) -ForegroundColor White
Write-Host ''
Write-Host '  2 stages + kernel (Ring 0)' -ForegroundColor Cyan
Write-Host '  s1_cpu@0x100000 s2_mem@0x200000 kernel@0x400000' -ForegroundColor Cyan
Write-Host ''


# ** LO QUE ESTE BUILD HIZO Y NO LLEGO A NINGUN DISCO (2026-09-27).
#
# Sin `-Data` ni `-Todo` el build no escribe en discos (es su postura, ver
# `-Data` arriba), y el `Espejo` solo habla si se le nombra una unidad. O sea
# que un `.\build.ps1` a secas compila un programa NUEVO, lo deja en `staging`
# y se calla. Paso el 27-09: `run sys/proton-x.bex` dijo "el archivo no esta"
# con el build bien hecho: el disco era el de antes, sin `apps\` ni `datos\`.
#
# Esto no copia nada: lista lo que este build ESCRIBIO en `staging\BMO-DATA`
# (por fecha, contra `$buildStart`) y da el comando.
function SinCopiar {
    if ($Data -or $espejoLetra) { return }
    $src = Join-Path $root 'staging\BMO-DATA'
    if (-not (Test-Path $src)) { return }
    $nuevos = @(Get-ChildItem -Path $src -Recurse -File | Where-Object { $_.LastWriteTime -ge $buildStart })
    if ($nuevos.Count -eq 0) { return }
    Write-Host ''
    Write-Host ('  [!] {0} archivo(s) de programas hechos por este build y SIN COPIAR a ningun disco (falta -Data o -Todo):' -f $nuevos.Count) -ForegroundColor Yellow
    $muestra = $nuevos | Sort-Object FullName | Select-Object -First 12
    foreach ($f in $muestra) {
        Write-Host ('      ' + $f.FullName.Substring($src.Length).TrimStart([char]'\')) -ForegroundColor Yellow
    }
    if ($nuevos.Count -gt 12) { Write-Host ('      ... y ' + ($nuevos.Count - 12) + ' mas') -ForegroundColor Yellow }
    Write-Host '      En BMO-X diran "el archivo no esta" hasta que se copien:' -ForegroundColor Yellow
    Write-Host '      .\Ultra_kernel_x86-64\build.ps1 -Todo -Drive <letra> -Yes' -ForegroundColor Yellow
    Write-Host ''
}

# A que unidad se le hace el espejo. `-Data` manda; luego `-Flash`; y si solo
# se compilo, la que se haya ESCRITO en `-Drive` -no vale el valor por defecto,
# o el espejo saldria contra una unidad que nadie nombro.
$espejoLetra = ''
if ($Data)                                       { $espejoLetra = $Data.TrimEnd([char]':',[char]'\').ToUpper() }
elseif ($Flash -or $Verify)                      { $espejoLetra = $Drive.TrimEnd([char]':',[char]'\').ToUpper() }
elseif ($PSBoundParameters.ContainsKey('Drive')) { $espejoLetra = $Drive.TrimEnd([char]':',[char]'\').ToUpper() }

if ($BuildOnly) {
    Espejo $espejoLetra
    SinCopiar
    Tiempos
    exit 0
}

# -- DISCOS (L6a: `build.ps1` se partio el 2026-08-28) --------
. (Join-Path $PSScriptRoot 'build\discos.ps1')
if ($null -ne $script:salida) { exit $script:salida }

Espejo $espejoLetra
SinCopiar
Tiempos
