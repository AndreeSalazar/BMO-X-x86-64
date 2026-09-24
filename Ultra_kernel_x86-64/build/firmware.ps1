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
#     gsp.bin       gsp            el GSP-RM: ELF RISC-V de 38 MB, con la
#                                  firma `.fwsignature_ga10x` que pide la 3060
#
# ** De donde: linux-firmware, version 535.113.01 -- la misma que pide
# nova-core (`FIRMWARE_VERSION`). La GA106 no tiene carpeta propia: el WHENCE
# dice `nvidia/ga106/gsp -> ../ga102/gsp`, asi que se bajan los de GA102.
#
# ** Por que no van en el repo: el GSP-RM son 38 MB (el `.git` entero mide 24)
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

Step 'Staging firmware del GSP (L0c, 3060)...'
$fwVersion = '535.113.01'
$fwOrigen  = 'https://gitlab.com/kernel-firmware/linux-firmware/-/raw/main/nvidia/ga102/gsp/'
$fwCache   = Join-Path (Split-Path -Parent (Split-Path -Parent $root)) 'BMO-externo\firmware\nvidia\ga102\gsp'
$fwDestino = Join-Path $dataBase 'fw\gsp'
# nombre en linux-firmware, nombre 8.3 en BMO-DATA, SHA-256
$fwLista = @(
    @('booter_load',   'boot_ld.bin', '3ab46bf4c70d72e8fc89d98e6bdc9b83a61954a586629d26b8b5653a620237e0'),
    @('booter_unload', 'boot_ul.bin', '09240c83595856a0fe70d2ab3ec7325455cd8adfc48ec7b47f6fced1e5431beb'),
    @('bootloader',    'bootldr.bin', '076d2a0675131ad8a8de215c611ce4271289adcc03e2cf39995f9b29079e2136'),
    @('gsp',           'gsp.bin',     'e30231ec0d317021770e16c81ae2c0553f728d0344248827582f78132c9447d4')
)
New-Item -ItemType Directory -Path $fwCache -Force | Out-Null
New-Item -ItemType Directory -Path $fwDestino -Force | Out-Null
$fwFaltan = 0
foreach ($fw in $fwLista) {
    $nombre = $fw[0] + '-' + $fwVersion + '.bin'
    $local  = Join-Path $fwCache $nombre
    if ((Test-Path $local) -and ((Hash256 $local) -ne $fw[2])) {
        Write-Host ('    [fw] ' + $nombre + ': el SHA-256 no cuadra, se baja otra vez') -ForegroundColor Yellow
        Remove-Item -LiteralPath $local -Force
    }
    if (-not (Test-Path $local)) {
        Write-Host ('    [fw] bajando ' + $nombre + ' de linux-firmware...') -ForegroundColor DarkGray
        # PowerShell 5.1 no ofrece TLS 1.2 si no se le pide, y sin barra de
        # progreso los 38 MB bajan en segundos en vez de minutos.
        $progresoPrevio = $ProgressPreference
        try {
            [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
            $ProgressPreference = 'SilentlyContinue'
            Invoke-WebRequest -Uri ($fwOrigen + $nombre) -OutFile $local -UseBasicParsing
        } catch {
            Write-Host ('    [fw] no se pudo bajar ' + $nombre + ': ' + $_.Exception.Message) -ForegroundColor Yellow
            Remove-Item -LiteralPath $local -Force -ErrorAction SilentlyContinue
        } finally {
            $ProgressPreference = $progresoPrevio
        }
    }
    if (-not (Test-Path $local)) { $fwFaltan++; continue }
    if ((Hash256 $local) -ne $fw[2]) {
        Write-Host ('    [fw] ' + $nombre + ' bajo con OTRO SHA-256: no se copia') -ForegroundColor Yellow
        Remove-Item -LiteralPath $local -Force
        $fwFaltan++
        continue
    }
    Copy-Item -LiteralPath $local -Destination (Join-Path $fwDestino $fw[1]) -Force
    Write-Host ('    [fw] ' + $fw[1] + ' (' + (Get-Item $local).Length + ' B) <- ' + $nombre) -ForegroundColor DarkGray
}
if ($fwFaltan -gt 0) {
    Write-Host ('    [fw] faltan ' + $fwFaltan + ' de 4: el build sigue, pero L0c no tendra con que arrancar el GSP') -ForegroundColor Yellow
    Write-Host ('    [fw]   se pueden dejar a mano en ' + $fwCache) -ForegroundColor Yellow
    Write-Host '    [fw]   (de /lib/firmware/nvidia/ga102/gsp/ de un Linux, descomprimidos)' -ForegroundColor Yellow
} else {
    Write-Host '    [fw] los 4 del GSP, SHA-256 OK -> fw\gsp\' -ForegroundColor DarkGray
}
