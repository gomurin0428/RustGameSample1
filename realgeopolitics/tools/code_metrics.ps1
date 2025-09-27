param(
    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..'))
)

if (-not (Test-Path $Root)) {
    throw "指定されたルートが存在しません: $Root"
}

$workspace = Resolve-Path $Root
$files = Get-ChildItem -Path $workspace -Recurse -Filter *.rs |
    Where-Object { $_.FullName -notmatch "\\target\\" }

if (-not $files) {
    Write-Host "Rust ソースファイルが見つかりませんでした。"
    exit 0
}

$metrics = foreach ($file in $files) {
    $lines = Get-Content -Path $file.FullName
    $totalLines = $lines.Count
    $codeLines = ($lines | Where-Object { $_.Trim() -ne '' -and -not $_.Trim().StartsWith('//') }).Count
    $testCount = ($lines | Where-Object { $_ -match '\[test\]' }).Count

    $relative = [System.IO.Path]::GetRelativePath($workspace.Path, $file.FullName)
    $crate = if ($relative) { ($relative -split '[\\/]')[0] } else { '(root)' }

    [pscustomobject]@{
        File      = $file.FullName
        Relative  = $relative
        Crate     = $crate
        Lines     = $totalLines
        CodeLines = $codeLines
        Tests     = $testCount
    }
}

$summary = [pscustomobject]@{
    TotalRustFiles  = $metrics.Count
    TotalLines      = ($metrics | Measure-Object -Property Lines -Sum).Sum
    CodeLines       = ($metrics | Measure-Object -Property CodeLines -Sum).Sum
    TotalTests      = ($metrics | Measure-Object -Property Tests -Sum).Sum
    AvgLinesPerFile = [Math]::Round((($metrics | Measure-Object -Property Lines -Average).Average), 2)
}

Write-Host "== Workspace Summary =="
$summary | Format-List

Write-Host "`n== Crate Breakdown =="
$crateStats = $metrics | Group-Object -Property Crate | ForEach-Object {
    [pscustomobject]@{
        Crate       = $_.Name
        Files       = $_.Count
        TotalLines  = ($_.Group | Measure-Object -Property Lines -Sum).Sum
        CodeLines   = ($_.Group | Measure-Object -Property CodeLines -Sum).Sum
        Tests       = ($_.Group | Measure-Object -Property Tests -Sum).Sum
        AvgLines    = [Math]::Round((($_.Group | Measure-Object -Property Lines -Average).Average), 2)
    }
}
$crateStats | Sort-Object -Property TotalLines -Descending | Format-Table -AutoSize

Write-Host "`n== Top 5 Files by Total Lines =="
$metrics |
    Sort-Object -Property Lines -Descending |
    Select-Object -First 5 -Property @{Name='RelativePath';Expression={ $_.Relative }}, Lines, CodeLines, Tests |
    Format-Table -AutoSize
