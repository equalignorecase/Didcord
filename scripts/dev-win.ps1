# Configure Windows MSVC + CMake + Ninja + NASM environment for building Didcord.
# Usage (dot-source in PowerShell): . .\scripts\dev-win.ps1

$vcvars = "D:\VSBuildTools\VC\Auxiliary\Build\vcvars64.bat"
if (-not (Test-Path $vcvars)) {
	$vcvars = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
}

if (Test-Path $vcvars) {
	cmd /c "`"$vcvars`" && set" | ForEach-Object {
		if ($_ -match '^([^=]+)=(.*)$') {
			[Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
		}
	}
}

$vsCmake = "D:\VSBuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
if (Test-Path $vsCmake) {
	$env:CMAKE = $vsCmake
}

$env:CMAKE_GENERATOR = "Ninja"

$extraPaths = @(
	"D:\tools\nasm",
	"D:\VSBuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja"
) | Where-Object { Test-Path $_ }

if ($extraPaths.Count -gt 0) {
	$env:PATH = ($extraPaths -join ";") + ";" + $env:PATH
}

Write-Host "Didcord Windows build environment ready (CMAKE_GENERATOR=$env:CMAKE_GENERATOR)."
