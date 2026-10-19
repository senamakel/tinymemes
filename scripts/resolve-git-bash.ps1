function Resolve-GitBashExecutable {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$GitExecutable)

    $gitPath = [System.IO.Path]::GetFullPath($GitExecutable)
    $directory = Get-Item -LiteralPath (Split-Path -Parent $gitPath) -ErrorAction Stop
    $searched = [System.Collections.Generic.List[string]]::new()
    while ($null -ne $directory) {
        if ($directory.Name -ieq 'Git') {
            $candidate = Join-Path $directory.FullName 'bin/bash.exe'
            $searched.Add($candidate)
            if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                return $candidate
            }
        }
        $directory = $directory.Parent
    }
    $searchedText = if ($searched.Count -gt 0) { $searched -join ', ' } else { '<no Git directory ancestor>' }
    throw "Git for Windows Bash not found for git.exe '$gitPath'; searched candidates: $searchedText"
}
