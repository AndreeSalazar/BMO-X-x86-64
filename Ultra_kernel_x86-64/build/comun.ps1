# comun.ps1 -- lo que comparten las ventanas del build (2026-10-03): el reloj
# por etapa, `Step`, `Tiempos`, `Fail`, `Hash256` y `Guardian`.
#
# ** Por que es un fichero: el propietario pidio TRES ventanas en paralelo --
# una que COMPILA, otra que VERIFICA y la principal que va al FAT32-- y las tres
# hablan igual. Antes estas funciones vivian en `build.ps1` y solo el las
# tenia; copiarlas a la ventana de verificar seria tener dos sitios donde
# arreglar cada una.
#
# [!] Se carga con punto: corre en el ambito de quien lo carga y usa su
# `$root` (la carpeta `Ultra_kernel_x86-64`). El texto es el de antes, tal cual.

# ** EL RELOJ POR ETAPA (2026-10-01). `Step` ya decia el tiempo ACUMULADO, y
# con eso no se ve donde se va: hay que restar a mano cuarenta lineas. Ahora
# cada `Step` apunta cuando empieza, una etapa dura hasta que empieza la
# siguiente, y `Tiempos` imprime la tabla al final (y tambien si el build
# muere: ahi es donde mas importa saber cuanto llevaba). Solo mide: no cambia
# nada de lo que se compila.
$script:reloj = [Diagnostics.Stopwatch]::StartNew()
$script:etapas = New-Object System.Collections.ArrayList
function Step {
    param($m)
    [void]$script:etapas.Add([pscustomobject]@{ Nombre = "$m"; Desde = $script:reloj.Elapsed.TotalSeconds })
    Write-Host ('  => ' + $m + ('   [{0,5:N1} s]' -f $script:reloj.Elapsed.TotalSeconds)) -ForegroundColor Cyan
}
function Tiempos {
    $fin = $script:reloj.Elapsed.TotalSeconds
    $filas = @()
    $primera = if ($script:etapas.Count -gt 0) { $script:etapas[0].Desde } else { $fin }
    if ($primera -gt 0.05) { $filas += [pscustomobject]@{ Nombre = '(antes del primer paso)'; Segundos = $primera } }
    for ($i = 0; $i -lt $script:etapas.Count; $i++) {
        $hasta = if ($i + 1 -lt $script:etapas.Count) { $script:etapas[$i + 1].Desde } else { $fin }
        $filas += [pscustomobject]@{ Nombre = $script:etapas[$i].Nombre; Segundos = $hasta - $script:etapas[$i].Desde }
    }
    Write-Host ''
    Write-Host '  etapa                                                         segundos' -ForegroundColor White
    Write-Host '  ------------------------------------------------------------  --------' -ForegroundColor DarkGray
    foreach ($f in ($filas | Sort-Object Segundos -Descending)) {
        $n = $f.Nombre; if ($n.Length -gt 60) { $n = $n.Substring(0, 57) + '...' }
        Write-Host ('  {0,-60}  {1,8:N1}' -f $n, $f.Segundos)
    }
    Write-Host '  ------------------------------------------------------------  --------' -ForegroundColor DarkGray
    Write-Host ('  {0,-60}  {1,8:N1}' -f 'TOTAL', $fin) -ForegroundColor White
    Write-Host ''
}
# *** `exit` DENTRO DE UN FICHERO CARGADO CON PUNTO SOLO SALE DE ESE FICHERO
# (2026-10-05). `. contrato.ps1` corre en el ambito de quien lo carga, pero un
# `exit` en su nivel de arriba termina `contrato.ps1` y NADA MAS: quien lo cargo
# sigue con la linea siguiente. Desde que esto se partio en ficheros (03-10)
# cada `Fail` de `comprobar`, `guardianes`, `contrato`, `ejemplos` o `discos`
# paraba su fichero y el resto seguia: VERIFICAR dijo "TODO VERIFICADO" con
# tres [X] encima. Probado en PowerShell 7.4.
#
# Asi que `Fail` (y `Muere`, y el "Abortado" de `discos.ps1`) APUNTA el codigo
# en `$script:salida` antes de salir, y quien carga un fichero con punto mira
# al volver: `if ($null -ne $script:salida) { exit $script:salida }`. Un `.`
# nuevo sin esa linea detras vuelve a tener el agujero.
function Fail { param($m) Write-Host ('  [X] ' + $m) -ForegroundColor Red; Tiempos; $script:salida = 1; exit 1 }
function Hash256 { param($p) (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant() }

# ** LOS GUARDIANES DE PYTHON, en un sitio: habia DOS bloques identicos y el
# tercero (L6a) seria la tercera. Sin Python se avisa; si falla, el build para.
function Guardian {
    param($paso, $script, $queMide, $siFalla)
    Step $paso
    $ruta = Join-Path (Split-Path -Parent $root) $script
    $py = (Get-Command python -ErrorAction SilentlyContinue)
    # ** FALTA EL SCRIPT y NO HAY PYTHON no son lo mismo, y hasta el 20-08 los
    # dos avisaban y seguian. Un path mal escrito dejo un guardian MUERTO y el
    # build dijo COMPLETE igual -- el fallo que este fichero ya describe:
    # avisar de nada, y con tono tranquilizador. El script es del REPO: para.
    if (-not (Test-Path $ruta)) { Fail ('guardian MUERTO: falta ' + $script) }
    if (-not $py) {
        Write-Host ('  [!] python no encontrado: no se comprueba ' + $queMide) -ForegroundColor Yellow
        return
    }
    $env:PYTHONIOENCODING = 'utf-8'
    $salida = & $py.Source $ruta --check
    if ($LASTEXITCODE -ne 0) {
        $salida | ForEach-Object { Write-Host ('    ' + $_) -ForegroundColor Red }
        Fail $siFalla
    }
    $salida | Where-Object { $_ -match 'clean:' } | ForEach-Object {
        Write-Host ('    ' + $_.Trim()) -ForegroundColor DarkGray
    }
}
