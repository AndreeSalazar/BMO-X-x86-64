# EL FIRMWARE DEL GSP DE LA 3060 (L0c), AL VOLUMEN DE DATOS.
#
# ** Por que esto es un fichero (2026-09-24)
#
# Porque es lo unico del build que NO sale del repo ni de un compilador: son
# cuatro binarios FIRMADOS por NVIDIA que la VBIOS no trae. FWSEC-FRTS (L0b)
# si venia en la ROM de la tarjeta; el booter y el GSP-RM no, y sin ellos la
# WPR2 que monto FWSEC se queda vacia.
#
#     boot_ld.bin   booter_load    corre en el SEC2, sube el GSP-RM a la WPR2
#     boot_ul.bin   booter_unload  el camino de vuelta, al apagar
#     bootldr.bin   bootloader     el primer codigo RISC-V del GSP
#     gsp.bin       gsp            el GSP-RM: ELF RISC-V de 63 MB, con la
#                                  firma `.fwsignature_ga10x` que pide la 3060
#
# ** De donde: linux-firmware, version 570.144 -- la que nova-core ARRANCA
# (sus mensajes con el GSP son `gsp/fw/r570_144`). Se empezo con la 535.113.01
# (la `FIRMWARE_VERSION` de Linux 6.17), pero esa solo servia hasta FWSEC: la
# WPR meta, el heap y las colas cambian con la version, y la unica que tiene
# un camino publicado entero es la 570.144 (cambiado el 24-09, en L0c1).
# La GA106 no tiene carpeta propia: el WHENCE dice
# `nvidia/ga106/gsp -> ../ga102/gsp`, asi que se bajan los de GA102.
#
# ** Por que no van en el repo: el GSP-RM son 63 MB (el `.git` entero mide 24)
# y la licencia es de NVIDIA (LICENSES/LICENCE.nvidia de linux-firmware), no
# la de BMO-X. Viven FUERA, en `BMO-externo\firmware\`, como DOOM: se bajan
# UNA vez y el build los reutiliza.
#
# ** Por que el SHA-256 va escrito aqui: un firmware firmado a medio bajar no
# falla al copiarlo; falla en el BROM del falcon, y ese error manda a buscar el
# bug en el parche del booter en vez de en el cable. El hash de aqui es el de
# linux-firmware el 24-09; uno distinto no se copia al disco.
#
# [!] Y NO ES FATAL. Sin red el build sigue: sin estos cuatro solo L0c queda
# sin hacer, y el resto de BMO-X no los necesita. Lo que falte se dice con la
# ruta donde dejarlo a mano.
#
# [!] Se carga con punto: corre en el ambito de `build.ps1` y usa sus `Step`,
# `Hash256`, `$root` y el `$dataBase` de `ejemplos.ps1`.
#
# ** Y tambien se puede correr SOLO (2026-09-24): la primera vez se corrio
# asi, sin `build.ps1` detras, y el guion solto treinta errores de ruta nula y
# acabo diciendo "los 4 del GSP, SHA-256 OK" sin haber bajado nada. Ahora,
# si falta lo de `build.ps1`, se lo pone el mismo; y el OK se CUENTA con los
# que llegaron, no con los que fallaron -- un error que no lanza no se cuenta.
if (-not (Get-Command Step -ErrorAction SilentlyContinue)) {
    function Step { param($m) Write-Host ('  => ' + $m) -ForegroundColor Cyan }
}
if (-not (Get-Command Hash256 -ErrorAction SilentlyContinue)) {
    function Hash256 { param($p) (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant() }
}
if (-not $root) { $root = Split-Path -Parent $PSScriptRoot }
if (-not $dataBase) { $dataBase = Join-Path $root 'staging\BMO-DATA' }

# ** LA VERSION EMPAREJADA (20-10, PLAN_GPU_COMO_LINUX F2a)
#
# Ahora hay dos: la 615.78.08 -- la que arranca la 3060 en ULTRA LinuX -- y la
# 570.144 de reserva hasta que la 615 pase sus diez arranques en frio (F4).
# Los cuatro ficheros van SIEMPRE de la misma version: un booter de una con
# el GSP-RM de otra es un ROJO sin explicacion. Se prepara la primera version
# cuyos cuatro esten y cuadren; `$env:BMO_GSP_VERSION` fuerza una.
#
# [!] La 615 no tiene de donde bajarse: linux-firmware no la publica. Sus
# cuatro se dejan a mano en `BMO-externo\firmware\nvidia\ga102\gsp\` (F0 del
# plan). El kernel la reconoce por el BLAKE3 de su `.fwimage`
# (`HASH_615` de `ring0/dev/gpu_gsp.rs`, sacado con `fw-huella`).
Step 'Staging firmware del GSP (L0c, 3060)...'
$fwCache   = Join-Path (Split-Path -Parent (Split-Path -Parent $root)) 'BMO-externo\firmware\nvidia\ga102\gsp'
$fwDestino = Join-Path $dataBase 'fw\gsp'
# por version: de donde se baja (o $null), y nombre, nombre 8.3 en BMO-DATA, SHA-256
$fwVersiones = @(
    @{ Version = '615.78.08'; Origen = $null; Lista = @(
        @('booter_load',   'boot_ld.bin', 'ad36e2044f76df6d0715031acb665c20cfcc70b0611748e41391b733a0d1db50'),
        @('booter_unload', 'boot_ul.bin', '8f68eb5bc034009f5b874aa0aa5da81ecc1fdad8e892c16afd862d78cf34fdc2'),
        @('bootloader',    'bootldr.bin', '82428f532240727e95bb3083fbaaba9b2cc7b937314323f2d546ce7245f27fad'),
        @('gsp',           'gsp.bin',     'a20100cafd5a85e22b3e13188339730c6ced94bd5f654984dd2ed274ed58d584')
    ) },
    @{ Version = '570.144'; Origen = 'https://gitlab.com/kernel-firmware/linux-firmware/-/raw/main/nvidia/ga102/gsp/'; Lista = @(
        @('booter_load',   'boot_ld.bin', '4497e3eff7e95c774b8a569d17b27c08c9650158d10b229d2be81cdcad9a085b'),
        @('booter_unload', 'boot_ul.bin', '8e63db5b78d7d3e349f20a2d11099c3d7109081393cb09ffc0a28133324ae009'),
        @('bootloader',    'bootldr.bin', '82428f532240727e95bb3083fbaaba9b2cc7b937314323f2d546ce7245f27fad'),
        @('gsp',           'gsp.bin',     'a8c3ebeed280323aedb51c061f321e73379cce7a9ae643a33dd03915df027f7f')
    ) }
)
if ($env:BMO_GSP_VERSION) {
    $fwVersiones = @($fwVersiones | Where-Object { $_.Version -eq $env:BMO_GSP_VERSION })
    if ($fwVersiones.Count -eq 0) {
        Write-Host ('    [fw] BMO_GSP_VERSION=' + $env:BMO_GSP_VERSION + ' no es una version conocida (615.78.08, 570.144)') -ForegroundColor Yellow
    }
}
New-Item -ItemType Directory -Path $fwCache -Force | Out-Null
New-Item -ItemType Directory -Path $fwDestino -Force | Out-Null
$fwElegida = $null
foreach ($fv in $fwVersiones) {
    $fwVersion = $fv.Version
    $fwListos = 0
    foreach ($fw in $fv.Lista) {
        $nombre = $fw[0] + '-' + $fwVersion + '.bin'
        $local  = Join-Path $fwCache $nombre
        if ((Test-Path $local) -and ((Hash256 $local) -ne $fw[2])) {
            Write-Host ('    [fw] ' + $nombre + ': el SHA-256 no cuadra, no se usa') -ForegroundColor Yellow
            if ($fv.Origen) { Remove-Item -LiteralPath $local -Force }
            continue
        }
        if (-not (Test-Path $local) -and $fv.Origen) {
            Write-Host ('    [fw] bajando ' + $nombre + ' de linux-firmware...') -ForegroundColor DarkGray
            # PowerShell 5.1 no ofrece TLS 1.2 si no se le pide, y sin barra de
            # progreso los 63 MB bajan en segundos en vez de minutos.
            $progresoPrevio = $ProgressPreference
            try {
                [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
                $ProgressPreference = 'SilentlyContinue'
                Invoke-WebRequest -Uri ($fv.Origen + $nombre) -OutFile $local -UseBasicParsing
            } catch {
                Write-Host ('    [fw] no se pudo bajar ' + $nombre + ': ' + $_.Exception.Message) -ForegroundColor Yellow
                Remove-Item -LiteralPath $local -Force -ErrorAction SilentlyContinue
            } finally {
                $ProgressPreference = $progresoPrevio
            }
        }
        if (-not (Test-Path $local)) { continue }
        if ((Hash256 $local) -ne $fw[2]) {
            Write-Host ('    [fw] ' + $nombre + ' bajo con OTRO SHA-256: no se usa') -ForegroundColor Yellow
            Remove-Item -LiteralPath $local -Force
            continue
        }
        $fwListos++
    }
    if ($fwListos -eq $fv.Lista.Count) { $fwElegida = $fv; break }
    Write-Host ('    [fw] ' + $fwVersion + ': ' + $fwListos + ' de ' + $fv.Lista.Count + ' en ' + $fwCache) -ForegroundColor Yellow
}
if ($fwElegida) {
    # Los cuatro de UNA version, juntos; y la version escrita al lado.
    $fwCopiados = 0
    foreach ($fw in $fwElegida.Lista) {
        $local = Join-Path $fwCache ($fw[0] + '-' + $fwElegida.Version + '.bin')
        $copia = Join-Path $fwDestino $fw[1]
        Copy-Item -LiteralPath $local -Destination $copia -Force
        if ((Hash256 $copia) -ne $fw[2]) {
            Write-Host ('    [fw] ' + $fw[1] + ': la copia a staging salio distinta') -ForegroundColor Yellow
            continue
        }
        $fwCopiados++
        Write-Host ('    [fw] ' + $fw[1] + ' (' + (Get-Item $local).Length + ' B) <- ' + $fw[0] + '-' + $fwElegida.Version + '.bin') -ForegroundColor DarkGray
    }
    Set-Content -LiteralPath (Join-Path $fwDestino 'version.txt') -Value $fwElegida.Version -Encoding ascii
    if ($fwCopiados -eq $fwElegida.Lista.Count) {
        Write-Host ('    [fw] los 4 del GSP ' + $fwElegida.Version + ', SHA-256 OK -> ' + $fwDestino) -ForegroundColor Green
    }
} else {
    Write-Host '    [fw] ninguna version tiene sus 4: el build sigue, pero L0c no tendra con que arrancar el GSP' -ForegroundColor Yellow
    Write-Host ('    [fw]   se pueden dejar a mano en ' + $fwCache) -ForegroundColor Yellow
    Write-Host '    [fw]   (la 615.78.08: F0 de docs/plan/PLAN_GPU_COMO_LINUX.md)' -ForegroundColor Yellow
}
