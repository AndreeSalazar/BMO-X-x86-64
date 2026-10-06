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
try {
    $gpu = (Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name + ' (' + $_.DriverVersion + ')' }) -join '; '
    $resumen.Add("GPU: $gpu")
} catch { $resumen.Add('GPU: no se pudo preguntar') }
$resumen.Add('')

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
    $p = Start-Process -FilePath $exe -WorkingDirectory $PSScriptRoot -NoNewWindow -PassThru `
        -RedirectStandardOutput $sale -RedirectStandardError $err
    # Sin tocar Handle antes de que acabe, ExitCode sale vacio (PowerShell 5.1).
    $null = $p.Handle
    $colgado = -not $p.WaitForExit(60000)
    if ($colgado) { try { $p.Kill() } catch {} }
    $lineas = @(Get-Content -LiteralPath $sale -ErrorAction SilentlyContinue) +
              @(Get-Content -LiteralPath $err -ErrorAction SilentlyContinue)
    if ((Get-Item -LiteralPath $err).Length -eq 0) { Remove-Item -LiteralPath $err }
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
