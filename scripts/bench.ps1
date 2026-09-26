# Compares Secopy with robocopy on Windows (RFD NFR-1..NFR-3, milestone M0).
#
# Usage: scripts\bench.ps1 -Source D:\bench\src -Dest E:\bench [-Generate]
#   -Generate  first fill -Source with large\ (LargeGiB x 1 GiB) and small\ (SmallCount x 16 KiB)
# Reports the median of 3 runs to docs\benchmarks\<date>-<computer>.md.
param(
    [Parameter(Mandatory)] [string] $Source,
    [Parameter(Mandatory)] [string] $Dest,
    [switch] $Generate,
    [int] $LargeGiB = 4,
    [int] $SmallCount = 20000
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path "$PSScriptRoot\..").Path
cargo build --release -p secopy-cli --manifest-path "$repo\Cargo.toml"
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$secopy = "$repo\target\release\secopy-cli.exe"

if ($Generate) {
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    New-Item -ItemType Directory -Force "$Source\large" | Out-Null
    $chunk = New-Object byte[] (1MB)
    $rng.GetBytes($chunk)
    for ($i = 0; $i -lt $LargeGiB; $i++) {
        $f = [System.IO.File]::Create(("{0}\large\clip{1:D2}.bin" -f $Source, $i))
        for ($j = 0; $j -lt 1024; $j++) { $f.Write($chunk, 0, $chunk.Length) }
        $f.Dispose()
    }
    $data = New-Object byte[] (16KB)
    $rng.GetBytes($data)
    for ($i = 0; $i -lt $SmallCount; $i++) {
        $dir = "{0}\small\d{1:D3}" -f $Source, ($i % 100)
        if ($i -lt 100) { New-Item -ItemType Directory -Force $dir | Out-Null }
        [System.IO.File]::WriteAllBytes(("{0}\f{1:D5}.bin" -f $dir, $i), $data)
    }
}

# Runs $Run three times into a fresh target folder and returns the median seconds.
function Measure-Median([scriptblock] $Run) {
    $target = "$Dest\secopy-bench-run"
    $times = foreach ($n in 1..3) {
        if (Test-Path $target) { Remove-Item -Recurse -Force $target }
        New-Item -ItemType Directory $target | Out-Null
        (Measure-Command { & $Run $target }).TotalSeconds
    }
    Remove-Item -Recurse -Force $target
    ($times | Sort-Object)[1]
}

$lines = @(
    ("# Benchmark {0:yyyy-MM-dd HH:mm} on $env:COMPUTERNAME" -f (Get-Date)),
    '',
    "- OS: $([System.Environment]::OSVersion.VersionString)",
    "- Source: ``$Source``",
    "- Destination: ``$Dest``",
    "- Median of 3 runs; OS cache not purged",
    "- Secopy: $(& $secopy --version)"
)
foreach ($set in 'large', 'small') {
    if (-not (Test-Path "$Source\$set")) { continue }
    $results = [ordered]@{
        'robocopy /E /MT:8'  = Measure-Median { param($t) robocopy "$Source\$set" "$t\$set" /E /MT:8 /NFL /NDL /NJH /NJS /NP | Out-Null }
        'secopy copy'        = Measure-Median { param($t) & $secopy "$Source\$set" --to $t 2>&1 | Out-Null }
        'secopy copy+verify' = Measure-Median { param($t) & $secopy "$Source\$set" --to $t --verify 2>&1 | Out-Null }
    }
    $lines += @('', "## $set/", '', '| Command | Median [s] |', '|:---|---:|')
    foreach ($k in $results.Keys) { $lines += ('| `{0}` | {1:N2} |' -f $k, $results[$k]) }
}
$out = "$repo\docs\benchmarks"
New-Item -ItemType Directory -Force $out | Out-Null
$report = "$out\{0:yyyy-MM-dd}-$env:COMPUTERNAME.md" -f (Get-Date)
Set-Content -Path $report -Value $lines -Encoding utf8
Write-Output "Results: $report"
