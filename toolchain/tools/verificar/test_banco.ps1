# Casos negativos del portico PowerShell: codigo nativo, cobertura y resultados.
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$entrada = Join-Path $repo 'Ultra_kernel_x86-64\build\comprobar.ps1'
# La salida de un nativo no puede compartir nombre con el codigo de fallo.
# Tambien se miran funciones: otra asignacion local reintroduciria la ambiguedad.
$archivosPortico = @(Get-ChildItem (Join-Path $repo 'Ultra_kernel_x86-64\build') -Filter '*.ps1') +
    @(Get-Item (Join-Path $repo 'bmo.ps1'), (Join-Path $repo 'Ultra_kernel_x86-64\build.ps1'))
foreach ($file in $archivosPortico) {
    $syntaxErrors=$null
    $ast=[System.Management.Automation.Language.Parser]::ParseFile($file.FullName,[ref]$null,[ref]$syntaxErrors)
    if ($syntaxErrors) { throw ('sintaxis rota: ' + $file.FullName) }
    $collisions=$ast.FindAll({ param($node)
        $node -is [System.Management.Automation.Language.AssignmentStatementAst] -and
        $node.Left -is [System.Management.Automation.Language.VariableExpressionAst] -and
        $node.Left.VariablePath.UserPath -eq 'salida'
    },$true)
    if ($collisions.Count) { throw ('colision con script:salida en ' + $file.Name) }
}
Write-Host 'PASS: ningun texto nativo pisa el codigo de fallo'
& {
    $root = Join-Path $repo 'Ultra_kernel_x86-64'
    . (Join-Path $root 'build\comun.ps1')
    function Get-Command { param($Name,$ErrorAction) }
    function Fail { param($m) throw "RECHAZADO: $m" }
    $rechazo=''
    try { Guardian 'fixture' 'toolchain\tools\la-3060\la_3060.py' 'NVIDIA' 'fixture' -Obligatorio }
    catch { $rechazo=$_.Exception.Message }
    if ($rechazo -notlike '*obligatorio sin Python*') { throw 'NVIDIA debe rechazar falta de Python' }
    Guardian 'fixture opcional' 'toolchain\tools\la-3060\la_3060.py' 'fixture' 'fixture'
    Write-Host 'PASS: guardian obligatorio sin Python rechaza; opcional conserva su contrato'
}

$casos = @(
    @{ Nombre='bien'; Codigo=0; Meta=0; Error=''; Espera='' },
    @{ Nombre='salida-nativa'; Codigo=7; Meta=0; Error=''; Espera='cargo=7' },
    @{ Nombre='metadata'; Codigo=0; Meta=1; Error=''; Espera='metadata' },
    @{ Nombre='error-compilador'; Codigo=0; Meta=0; Error='error: compilacion rota'; Espera='compilo=False' },
    @{ Nombre='sin-resultado'; Codigo=0; Meta=0; Error=''; Espera='resultados'; SinResultado=$true },
    @{ Nombre='sin-propietario'; Codigo=0; Meta=0; Error=''; Espera='sin crate conocido'; Ajeno=$true }
)
foreach ($caso in $casos) {
    & {
        $raiz = Join-Path $repo ("target\verificacion\prueba-$PID-" + $caso.Nombre)
        foreach ($rel in @('toolchain\tools\contrato', 'toolchain\tools\proton-x')) {
            New-Item -ItemType Directory -Force -Path (Join-Path $raiz $rel) | Out-Null
        }
        Set-Content (Join-Path $raiz 'toolchain\tools\contrato\contrato.py') ''
        Set-Content (Join-Path $raiz 'toolchain\tools\proton-x\proton_x.py') ''
        $Rapido = $false
        function Titulo { param($m) }
        function Bien { param($m) }
        function Muere { param($m) throw "RECHAZADO: $m" }
        function Get-Command { param($Name, $ErrorAction) if ($Name -ne 'python') { throw 'solo se esperaba buscar Python' } }
        function cargo {
            $global:LASTEXITCODE = $caso.Meta
            '{"packages":[{"name":"fixture"}]}'
        }
        function cmd {
            $rutaError = [regex]::Match($args[1], '2>"([^"]+)"').Groups[1].Value
            $nombre = if ($caso.Ajeno) { 'ajeno' } else { 'fixture' }
            @(" Running unittests src/lib.rs (target/debug/deps/$nombre-1234567890abcdef.exe)", $caso.Error) |
                Set-Content -LiteralPath $rutaError
            $global:LASTEXITCODE = $caso.Codigo
            if (-not $caso.SinResultado) { 'test result: ok. 1 passed; 0 failed; 0 ignored; finished in 0.25s' }
        }
        $rechazo = ''
        try { . $entrada } catch { $rechazo = $_.Exception.Message }
        if ($caso.Espera) {
            if ($rechazo -notlike ('*' + $caso.Espera + '*')) { throw ("caso " + $caso.Nombre + ': rechazo incorrecto: ' + $rechazo) }
        } elseif ($rechazo) { throw $rechazo }
        Write-Host ('PASS: ' + $caso.Nombre)
    }
}
Write-Host 'clean: seis casos del portico banco, incluidos fallos sin texto error'
