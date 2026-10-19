param(
    [Parameter(Mandatory = $true)][string]$Crate,
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$HostKey,
    [string]$ExtraFilesJson = '[]',
    [string]$OutputDir = 'release-assets'
)
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root
if ($Crate -ne 'tinymemes-module') { throw "Unsupported module crate: $Crate" }
if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$') { throw "Invalid module version: $Version" }
if ($HostKey -notmatch '^[A-Za-z0-9]([A-Za-z0-9._-]*[A-Za-z0-9])?$') { throw "Invalid release host: $HostKey" }
if (@($ExtraFilesJson | ConvertFrom-Json).Count -ne 0) { throw 'This release profile does not package additional files' }
$extension = switch -Regex ($Target) {
    '^(x86_64|aarch64)-pc-windows-msvc$' { 'dll'; break }
    '^(x86_64|aarch64)-apple-darwin$' { 'dylib'; break }
    '^(x86_64|aarch64)-unknown-linux-gnu$' { 'so'; break }
    default { throw "Unsupported module target: $Target" }
}
$libraryName = if ($extension -eq 'dll') { 'tinymemes_module.dll' } elseif ($extension -eq 'dylib') { 'libtinymemes_module.dylib' } else { 'libtinymemes_module.so' }
$pathSeparators = [char[]]@([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
if ([IO.Path]::IsPathRooted($OutputDir) -or ($OutputDir.Split($pathSeparators) -contains '..')) { throw 'Output directory must be repository-relative' }
$outputPath = [IO.Path]::GetFullPath((Join-Path $root $OutputDir))
$rootPrefix = $root.TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
if (-not $outputPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output directory must stay inside the repository' }
$cursor = $root
foreach ($component in $OutputDir.Split($pathSeparators, [StringSplitOptions]::RemoveEmptyEntries)) {
    $cursor = Join-Path $cursor $component
    if ((Test-Path -LiteralPath $cursor -PathType Container) -and ((Get-Item -LiteralPath $cursor).Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Output path contains a reparse point: $OutputDir"
    }
}
New-Item -ItemType Directory -Force $outputPath | Out-Null
$outputPath = (Resolve-Path -LiteralPath $outputPath).Path
if (-not $outputPath.StartsWith($rootPrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Output directory escaped the repository' }

cargo build --locked --release --manifest-path Cargo.toml --package $Crate --target $Target
if ($LASTEXITCODE -ne 0) { throw 'Failed to build tinymemes-module' }
$stage = Join-Path $root 'target/module-package'
Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
$library = Join-Path $root "target/$Target/release/$libraryName"
Copy-Item -LiteralPath $library -Destination $stage
$hash = (Get-FileHash -LiteralPath (Join-Path $stage $libraryName) -Algorithm SHA256).Hash.ToLowerInvariant()
'"{0}" = "{1}"' -f $libraryName, $hash | Set-Content -Path (Join-Path $stage 'modules.toml') -Encoding utf8NoBOM
Copy-Item -LiteralPath 'LICENSE', 'README.md' -Destination $stage
$archive = Join-Path $outputPath "$Crate-$Version-$HostKey.zip"
Push-Location $stage
try {
    Compress-Archive -Path @($libraryName, 'LICENSE', 'README.md', 'modules.toml') -DestinationPath $archive -Force
} finally {
    Pop-Location
}
Write-Output $archive
