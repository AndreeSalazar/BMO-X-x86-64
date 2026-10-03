# paralelo.ps1 -- LAS TRES VENTANAS de `bmo.ps1 -Paralelo` (2026-10-03).
#
#   COMPILAR    una ventana nueva: `build.ps1 -BuildOnly -SinGuardianes`
#               (toolchain, Ring 3, kernel, el .efi y los .bex a `staging`)
#   VERIFICAR   otra ventana nueva: `verificar.ps1` (el contrato, el banco,
#               los guardianes)
#   PRINCIPAL   la de siempre: espera a las dos, dice como van, y SOLO si las
#               dos acaban bien lleva `staging` al disco (`build.ps1
#               -SoloDiscos`, con las comprobaciones de `discos.ps1` intactas)
#
# ** Por que: el propietario lo pidio -- *"uno para compilar, el otro para
# verificar y el otro es el principal para ir al FAT32"*. En fila, compilar
# esperaba al banco y a veintidos guardianes que no dependen de nada de lo que
# se compila; a la vez, el tiempo es el del mas lento, no la suma.
#
# ** Y la regla que no cambia: NADA llega al disco si una de las dos fallo. La
# principal no escribe hasta tener los DOS "bien"; si una falla, se para ahi,
# dice cual, y su ventana se queda abierta con el motivo.
#
# Como se hablan: cada ventana, al acabar, escribe su codigo de salida en
# `target\ventanas\<NOMBRE>.fin`. Si se cierra antes de acabar (la cerro el
# propietario), no hay fichero y su proceso ya no esta: se dice asi, y no se
# espera para siempre.
#
# [!] Las dos ventanas usan cargo a la vez. Comparten `target\` del workspace
# (el banco y los obreros del build): cargo pone un cerrojo y una espera a la
# otra ahi ("Blocking waiting for file lock"). No es un fallo; es el precio de
# no compilar dos veces lo mismo.
#
# [!] Se carga con punto desde `bmo.ps1` y usa sus `$raiz`, `Titulo`, `Bien` y
# `Muere`.

$script:carpetaVentanas = Join-Path $raiz 'Ultra_kernel_x86-64\target\ventanas'

# **Abrir una ventana** que corre `$orden` (texto de PowerShell) en la raiz del
# repo, con su nombre en el titulo, y que al acabar deja su codigo en su
# fichero. Va con `-EncodedCommand`: sin comillas que escapar, ni rutas con
# espacios que partir.
function Abrir-Ventana {
    param([string]$nombre, [string]$orden)
    New-Item -ItemType Directory -Force -Path $script:carpetaVentanas | Out-Null
    $fin = Join-Path $script:carpetaVentanas ($nombre + '.fin')
    Remove-Item -LiteralPath $fin -ErrorAction SilentlyContinue
    $aqui = $raiz -replace "'", "''"
    $finQ = $fin -replace "'", "''"
    $texto = @"
`$Host.UI.RawUI.WindowTitle = 'BMO-X  $nombre'
Set-Location -LiteralPath '$aqui'
`$codigo = 99
try {
    $orden
    `$codigo = if (`$null -eq `$LASTEXITCODE) { 0 } else { `$LASTEXITCODE }
} catch {
    Write-Host (`$_ | Out-String) -ForegroundColor Red
}
Set-Content -LiteralPath '$finQ' -Value `$codigo -Encoding Ascii
Write-Host ''
if (`$codigo -eq 0) {
    Write-Host '  $nombre -- BIEN. Esta ventana ya se puede cerrar.' -ForegroundColor Green
} else {
    Write-Host ('  $nombre -- FALLO (codigo ' + `$codigo + '): lo de arriba dice por que.') -ForegroundColor Red
}
"@
    $b64 = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($texto))
    # La misma consola que corre esto (Windows PowerShell o pwsh), en otra ventana.
    $exe = (Get-Process -Id $PID).Path
    $p = Start-Process -FilePath $exe -ArgumentList ('-NoProfile -NoExit -ExecutionPolicy Bypass -EncodedCommand ' + $b64) -PassThru
    [pscustomobject]@{ Nombre = $nombre; Proceso = $p; Fin = $fin; Desde = Get-Date; Codigo = $null; Segundos = 0 }
}

# Como va una ventana, en una palabra.
function Estado-Ventana($v) {
    $s = [int]((Get-Date) - $v.Desde).TotalSeconds
    if ($null -eq $v.Codigo) { return ('{0}: corriendo ({1} s)' -f $v.Nombre, $s) }
    if ($v.Codigo -eq 0) { return ('{0}: BIEN en {1} s' -f $v.Nombre, $v.Segundos) }
    if ($v.Codigo -eq -1) { return ('{0}: CERRADA antes de acabar' -f $v.Nombre) }
    return ('{0}: FALLO (codigo {1})' -f $v.Nombre, $v.Codigo)
}

# **Esperar a las ventanas**: hasta que todas acaben, o hasta que una falle (no
# se espera a la otra: ya no se va a desplegar). Dice como van al cambiar algo,
# y cada medio minuto aunque no cambie. Devuelve la primera que fallo, o nada.
function Esperar-Ventanas {
    param($ventanas)
    $dicho = ''
    $ultimoLatido = Get-Date
    while ($true) {
        foreach ($v in $ventanas) {
            if ($null -ne $v.Codigo) { continue }
            if (Test-Path -LiteralPath $v.Fin) {
                $n = 0
                # Recien escrito puede estar a medias: si no se lee, la vuelta siguiente.
                if ([int]::TryParse(("" + (Get-Content -LiteralPath $v.Fin -Raw -ErrorAction SilentlyContinue)).Trim(), [ref]$n)) {
                    $v.Codigo = $n
                    $v.Segundos = [int]((Get-Date) - $v.Desde).TotalSeconds
                }
            } elseif ($v.Proceso.HasExited) {
                $v.Codigo = -1
            }
        }
        $linea = (@($ventanas | ForEach-Object { Estado-Ventana $_ }) -join '   |   ')
        $cambio = ($linea -replace '\(\d+ s\)', '') -ne ($dicho -replace '\(\d+ s\)', '')
        if ($cambio -or ((Get-Date) - $ultimoLatido).TotalSeconds -ge 30) {
            Write-Host ('   ' + $linea) -ForegroundColor DarkGray
            $dicho = $linea
            $ultimoLatido = Get-Date
        }
        $mala = @($ventanas | Where-Object { $null -ne $_.Codigo -and $_.Codigo -ne 0 }) | Select-Object -First 1
        if ($mala) { return $mala }
        if (@($ventanas | Where-Object { $null -eq $_.Codigo }).Count -eq 0) { return $null }
        Start-Sleep -Milliseconds 500
    }
}
