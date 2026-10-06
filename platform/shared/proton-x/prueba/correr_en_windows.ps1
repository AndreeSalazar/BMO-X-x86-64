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

# nombre, cuantos `bien`, cuantas `nota` (-1: no se cuentan), y que mirar.
# Un `bien` de -1: el juez no tiene cuenta fija; vale ningun MAL y salir con 0.
$jueces = @(
    # -- PROTON-X por dentro (P1 a P5, 27-09 y 28-09)
    @('hola', -1, -1, 'imprime la frase y sale con 0'),
    @('teb', 6, -1, ''),
    @('hilos', 19, -1, ''),
    @('ficheros', 16, -1, 'deja pxtest.txt'),
    @('crt', 35, -1, ''),
    @('texto', 21, -1, ''),
    @('esperas', 24, -1, 'tarda unos 150 ms'),
    @('carpetas', 34, -1, 'deja pqa.txt, pqb.txt, pzc.txt'),
    @('sistema', 22, -1, 'sale por TerminateProcess'),
    @('ucrt', 19, -1, ''),
    @('stdio', 15, -1, 'uno de los bien va por stderr'),
    @('peek', 12, -1, ''),
    @('compila', -1, -1, 'en Windows compila de verdad (d3dcompiler_47)'),
    @('usadll', 8, -1, 'necesita saludo.dll al lado'),
    @('seh', 9, -1, ''),
    # -- Las TANDAS de Cyberpunk (29-09 a 02-10)
    @('tanda1', 55, -1, ''),
    @('tanda2', 14, -1, ''),
    @('tanda3', 25, -1, ''),
    @('tanda3b', 18, -1, ''),
    @('tanda3c', 16, -1, ''),
    @('tanda4', 11, -1, ''),
    @('tanda4m', 11, -1, 'la de MSVC'),
    @('tanda5', 22, -1, ''),
    @('tanda6', 29, -1, ''),
    @('tanda7', 29, -1, ''),
    @('tanda8', 25, -1, ''),
    @('tanda9', 30, -1, ''),
    @('tanda10', 25, -1, ''),
    @('tanda11', 35, -1, ''),
    @('tanda12', 24, -1, 'sin red tambien'),
    @('tanda13', 16, -1, ''),
    @('diario', 4, -1, ''),
    @('tanda14', 27, -1, ''),
    @('tanda14b', 12, -1, ''),
    @('tanda15', 11, -1, ''),
    @('tanda16', 9, -1, ''),
    @('tanda17', 11, -1, ''),
    @('tanda18', 16, -1, ''),
    @('tanda19', 19, -1, ''),
    @('tanda19m', 15, -1, 'la de MSVC'),
    @('tanda20', 15, -1, ''),
    @('tanda21', 17, -1, ''),
    @('tanda22', 9, -1, 'PREGUNTA ABIERTA: con 9 la hipotesis de HACER.txt era cierta; con 7 MAL, Windows sigue sin darle TLS a la DLL de clang'),
    @('tanda23', 16, -1, ''),
    @('tanda24', 12, -1, ''),
    @('tanda25', 9, -1, ''),
    @('tanda26', 6, -1, ''),
    @('tanda27', 8, -1, ''),
    @('tanda28', 6, -1, ''),
    @('tanda29', 18, -1, ''),
    @('tanda30', 14, -1, ''),
    @('tanda31', 30, -1, ''),
    @('tanda32', 10, -1, ''),
    @('tanda33', 8, -1, ''),
    @('tanda34', 9, -1, ''),
    @('tanda35', 8, -1, ''),
    @('tanda36', 6, -1, ''),
    @('tanda37', 9, -1, ''),
    @('tanda38', 13, -1, ''),
    @('tanda39', 8, -1, ''),
    @('tanda41', 10, -1, ''),
    @('tanda42', 13, -1, ''),
    @('tanda43', 12, -1, ''),
    @('tanda44', 10, -1, ''),
    @('tanda45', 15, -1, ''),
    @('tanda46', 9, 1, 'la nota dice el HRESULT de Windows'),
    @('tanda47', 21, -1, ''),
    @('tanda48', 14, -1, ''),
    @('vueltas', 4, -1, ''),
    # -- Los jueces de D3D12 (05-10 y 06-10): NINGUNO corrido aun en Windows
    @('computo', 4, -1, 'D3D12'),
    @('instancias', 3, -1, 'D3D12'),
    @('vistas', 7, -1, 'D3D12'),
    @('hdr', 4, -1, 'D3D12'),
    @('uavpixel', 4, -1, 'D3D12'),
    @('flotante1', 10, -1, 'D3D12'),
    @('stencil', 5, -1, 'D3D12'),
    @('olas', 15, -1, 'D3D12: el "hasta N" de B puede ser otro'),
    @('derivadas', 17, -1, 'D3D12'),
    @('restos', 18, -1, 'D3D12: si la GPU no tiene SV_StencilRef, E sale como nota (no es fallo)'),
    @('multihilo', 17, -1, 'D3D12'),
    @('volumen', 10, -1, 'D3D12'),
    @('firmas', 6, -1, 'D3D12'),
    @('postpro', 3, -1, 'D3D12')
)

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
    $nombre, $quiere, $notas, $que = $j
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
    if ($quiere -ge 0 -and $bien -ne $quiere) { $fallo += "$bien bien (pide $quiere)" }
    if ($notas -ge 0 -and $nota -ne $notas) { $fallo += "$nota nota (pide $notas)" }

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
