param([Parameter(Mandatory = $true)][string]$Archive)
$ErrorActionPreference = 'Stop'
$archivePath = (Resolve-Path -LiteralPath $Archive).Path
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root
$asset = if ($env:MODULE_ASSET) { $env:MODULE_ASSET } else { 'tinymemes-module' }
$version = $env:MODULE_VERSION
$hostKey = $env:MODULE_HOST
if (-not $version -or -not $hostKey) { throw 'MODULE_VERSION and MODULE_HOST are required' }
if ([IO.Path]::GetFileName($archivePath) -ne "$asset-$version-$hostKey.zip") { throw 'Unexpected module archive name' }
$priorModule = $env:TINYMEMES_MODULE
$work = Join-Path $root "target/verify-module-$([System.Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory $work | Out-Null
try {
    if ($IsWindows) {
        $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $security = [System.Security.AccessControl.DirectorySecurity]::new()
        $security.SetOwner($identity.User)
        $security.SetAccessRuleProtection($true, $false)
        foreach ($sidValue in @($identity.User.Value, 'S-1-5-18', 'S-1-5-32-544')) {
            $rule = [System.Security.AccessControl.FileSystemAccessRule]::new(
                [System.Security.Principal.SecurityIdentifier]::new($sidValue),
                [System.Security.AccessControl.FileSystemRights]::FullControl,
                [System.Security.AccessControl.InheritanceFlags]'ContainerInherit, ObjectInherit',
                [System.Security.AccessControl.PropagationFlags]::None,
                [System.Security.AccessControl.AccessControlType]::Allow)
            [void]$security.AddAccessRule($rule)
        }
        Set-Acl -LiteralPath $work -AclObject $security
    }
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($archivePath)
    try {
        $names = @()
        foreach ($entry in $zip.Entries) {
            $parts = $entry.FullName -split '[\\/]'
            $mode = ($entry.ExternalAttributes -shr 16) -band 0xF000
            if ([IO.Path]::IsPathRooted($entry.FullName) -or ($parts -contains '..') -or ($parts -contains '.') -or $mode -eq 0xA000) {
                throw "Archive member is unsafe: $($entry.FullName)"
            }
            if ($entry.FullName.EndsWith('/')) { throw "Archive has an unexpected directory: $($entry.FullName)" }
            $names += $entry.FullName
        }
        $expectedNames = @('LICENSE', 'README.md', 'modules.toml', 'tinymemes_module.dll')
        $actualNameList = @($names | Sort-Object) -join "`n"
        $expectedNameList = @($expectedNames | Sort-Object) -join "`n"
        if ($actualNameList -cne $expectedNameList) {
            throw 'Archive has an unexpected file set'
        }
    } finally {
        $zip.Dispose()
    }
    Expand-Archive -LiteralPath $archivePath -DestinationPath $work
    $library = Join-Path $work 'tinymemes_module.dll'
    $hash = (Get-FileHash -LiteralPath $library -Algorithm SHA256).Hash.ToLowerInvariant()
    $expected = '"tinymemes_module.dll" = "{0}"' -f $hash
    if ((Get-Content -LiteralPath (Join-Path $work 'modules.toml') -Raw).Trim() -ne $expected) {
        throw 'modules.toml does not pin the packaged library hash'
    }
    $env:TINYMEMES_MODULE = $library
    bash scripts/test-module-e2e.sh
    if ($LASTEXITCODE -ne 0) { throw 'extracted module failed the TinyBus loader and contract checks' }
} finally {
    if ($null -eq $priorModule) { Remove-Item Env:TINYMEMES_MODULE -ErrorAction SilentlyContinue }
    else { $env:TINYMEMES_MODULE = $priorModule }
    Remove-Item -LiteralPath $work -Recurse -Force
}
