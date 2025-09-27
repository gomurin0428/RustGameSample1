using namespace System.Text.Json

param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')),
    [string]$OutputDirName = 'metrics/complexity'
)

if (-not (Test-Path $Root)) {
    throw "指定されたルートが存在しません: $Root"
}

$workspace = Resolve-Path $Root
$complexityDir = Join-Path $workspace.Path $OutputDirName
if (Test-Path $complexityDir) {
    Remove-Item $complexityDir -Recurse -Force
}
New-Item -ItemType Directory -Path $complexityDir -Force | Out-Null

$rustFiles = Get-ChildItem -Path $workspace.Path -Recurse -Filter *.rs |
    Where-Object { $_.FullName -notmatch "\\target\\" }

if (-not $rustFiles) {
    Write-Host "Rust ソースファイルが見つかりませんでした。"
    exit 0
}

$cli = Join-Path $env:USERPROFILE '.cargo/bin/rust-code-analysis-cli.exe'
if (-not (Test-Path $cli)) {
    throw "rust-code-analysis-cli.exe が見つかりません。先に 'cargo install rust-code-analysis-cli --locked' を実行してください。"
}

Push-Location $workspace.Path
try {
    $relativeOutput = $OutputDirName -replace '/', [System.IO.Path]::DirectorySeparatorChar
    $args = @('--metrics','-l','rust','-O','json','-o',$relativeOutput)
    foreach ($file in $rustFiles) {
        $relativePath = [System.IO.Path]::GetRelativePath($workspace.Path, $file.FullName)
        $args += @('-p', $relativePath)
    }

    & $cli @args | Out-Null
}
finally {
    Pop-Location
}

function Get-GroupDouble {
    param(
        [JsonElement]$Metrics,
        [string]$Group,
        [string]$Name
    )
    try {
        $groupElem = $Metrics.GetProperty($Group)
        $valueElem = $groupElem.GetProperty($Name)
        if ($valueElem.ValueKind -eq [JsonValueKind]::Number) {
            return $valueElem.GetDouble()
        }
    } catch {
        return 0.0
    }
    return 0.0
}

$reports = Get-ChildItem -Path $complexityDir -Recurse -Filter *.json
if (-not $reports) {
    Write-Warning "複雑度レポートが生成されませんでした。"
    exit 1
}

$summary = @()
foreach ($report in $reports) {
    $jsonText = Get-Content -Path $report.FullName -Raw
    $doc = [JsonDocument]::Parse($jsonText)
    try {
        try {
            $metricsElem = $doc.RootElement.GetProperty('metrics')
        } catch {
            continue
        }

        try {
            $relative = $doc.RootElement.GetProperty('name').GetString()
        } catch {
            $relative = $report.FullName.Substring($workspace.Path.Length).TrimStart([char[]]('/','\'))
        }

        $summary += [pscustomobject]@{
            RelativePath        = $relative
            CyclomaticSum       = [Math]::Round((Get-GroupDouble $metricsElem 'cyclomatic' 'sum'), 4)
            CyclomaticAverage   = [Math]::Round((Get-GroupDouble $metricsElem 'cyclomatic' 'average'), 4)
            CognitiveSum        = [Math]::Round((Get-GroupDouble $metricsElem 'cognitive' 'sum'), 4)
            CognitiveAverage    = [Math]::Round((Get-GroupDouble $metricsElem 'cognitive' 'average'), 4)
            HalsteadVolume      = [Math]::Round((Get-GroupDouble $metricsElem 'halstead' 'volume'), 4)
            HalsteadDifficulty  = [Math]::Round((Get-GroupDouble $metricsElem 'halstead' 'difficulty'), 4)
            Maintainability     = [Math]::Round((Get-GroupDouble $metricsElem 'mi' 'mi_visual_studio'), 4)
            MaintainabilitySEI  = [Math]::Round((Get-GroupDouble $metricsElem 'mi' 'mi_sei'), 4)
            MaintainabilityOrig = [Math]::Round((Get-GroupDouble $metricsElem 'mi' 'mi_original'), 4)
            SLOC                = [Math]::Round((Get-GroupDouble $metricsElem 'loc' 'sloc'), 4)
            Functions           = [Math]::Round((Get-GroupDouble $metricsElem 'nom' 'total'), 4)
        }
    }
    finally {
        $doc.Dispose()
    }
}

$summaryPath = Join-Path $complexityDir 'summary.json'
$summary | ConvertTo-Json -Depth 5 | Set-Content -Path $summaryPath -Encoding UTF8

Write-Host "== Complexity Summary =="
$totalCyclomatic = ($summary | Measure-Object -Property CyclomaticSum -Sum).Sum
$totalCognitive = ($summary | Measure-Object -Property CognitiveSum -Sum).Sum
$totalFunctions = ($summary | Measure-Object -Property Functions -Sum).Sum
$totalSloc = ($summary | Measure-Object -Property SLOC -Sum).Sum
$avgMI = if ($summary.Count -gt 0) { [Math]::Round((($summary | Measure-Object -Property Maintainability -Average).Average), 2) } else { 0 }
Write-Host ("Cyclomatic Sum: {0}" -f [Math]::Round($totalCyclomatic,2))
Write-Host ("Cognitive Sum : {0}" -f [Math]::Round($totalCognitive,2))
Write-Host ("Functions     : {0}" -f $totalFunctions)
Write-Host ("Total SLOC    : {0}" -f $totalSloc)
Write-Host ("Avg MI (VS)   : {0}" -f $avgMI)

if ($summary.Count -gt 0) {
    Write-Host "`n== High Cyclomatic (Top 10) =="
    $summary |
        Sort-Object -Property CyclomaticSum -Descending |
        Select-Object -First 10 -Property RelativePath, CyclomaticSum, CognitiveSum, Maintainability |
        Format-Table -AutoSize

    Write-Host "`n== Lowest Maintainability (Top 10) =="
    $summary |
        Sort-Object -Property Maintainability |
        Select-Object -First 10 -Property RelativePath, Maintainability, CyclomaticSum, HalsteadVolume |
        Format-Table -AutoSize
}

Write-Host "`n詳細レポート: $summaryPath"
