# LOS JUECES DE PROTON-X, CORRIDOS EN WINDOWS (06-10)
#
# El propietario: "dime en lista larga para probar en Windows". Cada juez de
# esta carpeta se hizo en la nube y su `bien` se escribio a mano (o se saco
# de las reglas de D3D12): Windows es quien dice si el juez tiene razon. Si
# aqui sale lo que pide la tabla, el juez es bueno y lo que diga BMO-X se
# puede creer; si no, la linea MAL es lo que hace falta (el juez esta mal, o
# la regla que se escribio a mano).
#
# Se corre desde ESTA carpeta, en una PowerShell normal (no hace falta la de
# Visual Studio):
#
#     powershell -ExecutionPolicy Bypass -File .\correr_en_windows.ps1
#
# Solo los jueces de CONSOLA (los de ventana se miran a ojo: la lista larga
# esta en docs/metal/PRUEBAS_EN_WINDOWS.md). Cada uno con un tope de 60 s:
# si se cuelga, lo cierra y lo dice. Deja lo que dijo cada uno en
# informe_windows\<nombre>.txt y el resumen en informe_windows\resumen.txt:
# ese resumen (o la carpeta entera) es lo que hay que mandar.
#
# Algunos dejan ficheros en esta carpeta (pxtest.txt, pqa.txt, tanda3_a.txt,
# tanda14.txt...): es lo que hacen tambien en BMO-X, y se pueden correr otra vez.

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

# La tabla: `jueces.txt` (aqui al lado), la MISMA que lleva dentro
# `sys/jueces.bex` en BMO-X. Por juez: nombre, cuantos `bien` (-1: sin
# cuenta fija), cuantas `nota` (-1 si no se dice: no se cuentan), lo de
# detras de `#`, y OTRA cuenta de `bien` y `nota` que tambien vale.
$jueces = @()
foreach ($l in Get-Content -LiteralPath (Join-Path $PSScriptRoot 'jueces.txt')) {
    $t = $l.Trim()
    if ($t -eq '' -or $t.StartsWith('#') -or $t.StartsWith('[')) { continue }
    $que = ''
    $k = $t.IndexOf('#')
    if ($k -ge 0) { $que = $t.Substring($k + 1).Trim(); $t = $t.Substring(0, $k).Trim() }
    $c = $t -split '\s+'
    $notas = if ($c.Count -gt 2) { [int]$c[2] } else { -1 }
    $j = @($c[0], [int]$c[1], $notas, $que)
    if ($c.Count -gt 4) { $j += , @([int]$c[3], [int]$c[4]) }
    $jueces += , $j
}

$informe = Join-Path $PSScriptRoot 'informe_windows'
New-Item -ItemType Directory -Force -Path $informe | Out-Null
$resumen = New-Object System.Collections.Generic.List[string]
$resumen.Add("Los jueces de PROTON-X en Windows, " + (Get-Date -Format 'yyyy-MM-dd HH:mm'))
$resumen.Add([Environment]::OSVersion.VersionString)
$resumen.Add("La tabla (jueces.txt): $($jueces.Count) jueces")
try {
    $gpu = (Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name + ' (' + $_.DriverVersion + ')' }) -join '; '
    $resumen.Add("GPU: $gpu")
} catch { $resumen.Add('GPU: no se pudo preguntar') }
$resumen.Add('')

# A11 (06-10): la escena 3D DURA se compara con la imagen de ESTE Windows.
# Si todavia no esta (escena.ref), se hace aqui, una vez: `escena.exe
# guardar` la deja al lado (y escena.bmp, para mirarla). Esa escena.ref es
# la que hay que mandar: con ella se juzgan el banco y BMO-X.
$escenaRef = Join-Path $PSScriptRoot 'escena.ref'
$escenaNueva = $false
if (-not (Test-Path -LiteralPath $escenaRef) -and (Test-Path -LiteralPath (Join-Path $PSScriptRoot 'escena.exe'))) {
    Write-Host 'escena.ref no esta: la hago con escena.exe guardar (una vez)...'
    $p = Start-Process -FilePath (Join-Path $PSScriptRoot 'escena.exe') -ArgumentList 'guardar' -WorkingDirectory $PSScriptRoot -NoNewWindow -PassThru -Wait
    $escenaNueva = Test-Path -LiteralPath $escenaRef
}
if ($escenaNueva) { $resumen.Add('escena.ref HECHA en este Windows: mandala (esta en la carpeta prueba)'); $resumen.Add('') }

$buenos = 0; $malos = 0; $faltan = 0
foreach ($j in $jueces) {
    $nombre, $quiere, $notas, $que = $j[0..3]
    $otra = if ($j.Count -gt 4) { $j[4] } else { $null }
    $exe = Join-Path $PSScriptRoot "$nombre.exe"
    if (-not (Test-Path -LiteralPath $exe)) {
        $resumen.Add(('{0,-12} NO ESTA el .exe' -f $nombre)); $faltan++; continue
    }
    $sale = Join-Path $informe "$nombre.txt"
    $err = Join-Path $informe "$nombre.err.txt"
    # 06-10: con Process de .NET y no con Start-Process: este tiene el
    # proceso desde que nace, y su ExitCode no se pierde. Con Start-Process,
    # un .exe tan rapido como hola.exe acababa antes de que se pidiera su
    # Handle, y salia "salio con" y nada (lo vio el propietario).
    $psi = New-Object System.Diagnostics.ProcessStartInfo $exe
    $psi.WorkingDirectory = $PSScriptRoot
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $p = [System.Diagnostics.Process]::Start($psi)
    $leeSale = $p.StandardOutput.ReadToEndAsync()
    $leeErr = $p.StandardError.ReadToEndAsync()
    $colgado = -not $p.WaitForExit(60000)
    if ($colgado) { try { $p.Kill() } catch {} }
    $p.WaitForExit()
    Set-Content -LiteralPath $sale -Value $leeSale.Result -NoNewline
    if ($leeErr.Result.Length -gt 0) { Set-Content -LiteralPath $err -Value $leeErr.Result -NoNewline }
    $lineas = @(($leeSale.Result + "`n" + $leeErr.Result) -split "`r?`n")
    $bien = @($lineas | Where-Object { $_ -match '^\s*bien\b' }).Count
    $mal = @($lineas | Where-Object { $_ -match '^\s*MAL\b' }).Count
    $nota = @($lineas | Where-Object { $_ -match '^\s*nota\b' }).Count
    $codigo = if ($colgado) { 'COLGADO' } else { $p.ExitCode }

    $fallo = @()
    if ($colgado) { $fallo += 'se colgo (60 s)' }
    elseif ($codigo -ne 0) { $fallo += "salio con $codigo" }
    if ($mal -gt 0) { $fallo += "$mal MAL" }
    $vale_otra = $otra -and $bien -eq $otra[0] -and $nota -eq $otra[1]
    if (-not $vale_otra) {
        if ($quiere -ge 0 -and $bien -ne $quiere) { $fallo += "$bien bien (pide $quiere)" }
        if ($notas -ge 0 -and $nota -ne $notas) { $fallo += "$nota nota (pide $notas)" }
    }

    if ($fallo.Count -eq 0) {
        $buenos++
        $linea = '{0,-12} bien   {1} bien, sale con 0' -f $nombre, $bien
    } else {
        $malos++
        $linea = '{0,-12} DISTINTO  {1}' -f $nombre, ($fallo -join ', ')
    }
    if ($que) { $linea += "   [$que]" }
    $resumen.Add($linea)
    Write-Host $linea
    # Las MAL y las nota, enteras: es lo que hace falta para saber por que.
    foreach ($l in ($lineas | Where-Object { $_ -match '^\s*(MAL|nota)\b' })) {
        $resumen.Add("               $l")
    }
}

$resumen.Add('')
$resumen.Add("$buenos dicen lo que pide la tabla, $malos distintos, $faltan sin .exe")
$resumen | Set-Content -LiteralPath (Join-Path $informe 'resumen.txt') -Encoding UTF8
Write-Host ''
Write-Host "$buenos dicen lo que pide la tabla, $malos distintos, $faltan sin .exe"
Write-Host "El resumen: $informe\resumen.txt"
if ($escenaNueva) {
    Write-Host ''
    Write-Host 'IMPORTANTE: se hizo escena.ref (la imagen de la escena 3D de este Windows).' -ForegroundColor Yellow
    Write-Host "Mandala junto con el resumen: $escenaRef" -ForegroundColor Yellow
}
