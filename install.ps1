$ErrorActionPreference = "Stop"

$rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$cargoBinDir = if ($env:CARGO_HOME) {
    Join-Path $env:CARGO_HOME "bin"
} else {
    Join-Path $HOME ".cargo\bin"
}
$lectureBin = Join-Path $cargoBinDir "lecture.exe"
$whisperCppDir = if ($env:WHISPER_CPP_DIR) {
    $env:WHISPER_CPP_DIR
} else {
    Join-Path (Split-Path -Parent $rootDir) "whisper.cpp"
}
$whisperBuildDir = Join-Path $whisperCppDir "build"
$whisperModelDir = Join-Path $whisperCppDir "models"
$script:whisperModel = Join-Path $whisperModelDir "ggml-base.bin"
$projectModel = Join-Path $rootDir "ggml-base.bin"

# If the project bundles a ggml-base.bin, prefer that over the whisper.cpp models dir.
if ((Test-Path $projectModel) -and (Test-Path $whisperCppDir)) {
    $script:whisperModel = $projectModel
}
$launcherDir = Join-Path $HOME ".local\bin"
$launcherPath = Join-Path $launcherDir "lecture.cmd"

function Test-Command {
    param([string]$Name)

    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Resolve-WhisperBin {
    param([string]$BaseDir)

    $candidates = @(
        (Join-Path $BaseDir "build\bin\whisper-cli.exe"),
        (Join-Path $BaseDir "build\bin\Release\whisper-cli.exe"),
        (Join-Path $BaseDir "build\bin\Debug\whisper-cli.exe")
    )

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    return $candidates[0]
}

if (-not (Test-Command "cargo")) {
    throw "cargo is required but is not installed."
}

if (-not (Test-Command "cmake")) {
    throw "cmake is required but is not installed."
}

if (-not (Test-Command "ffmpeg")) {
    throw "ffmpeg is required but is not installed or not available in PATH."
}

if (-not (Test-Path $whisperCppDir)) {
    throw "whisper.cpp was not found at '$whisperCppDir'. Set WHISPER_CPP_DIR and run again."
}

Write-Host "Installing lecture..."
cargo install --path $rootDir --locked

$whisperBin = Resolve-WhisperBin -BaseDir $whisperCppDir
if (-not (Test-Path $whisperBin)) {
    Write-Host "Building whisper.cpp..."
    cmake -S $whisperCppDir -B $whisperBuildDir -DWHISPER_BUILD_TESTS=OFF
    cmake --build $whisperBuildDir --config Release
    $whisperBin = Resolve-WhisperBin -BaseDir $whisperCppDir
}

if (-not (Test-Path $whisperModel)) {
    if (-not (Test-Path $whisperModelDir)) {
        New-Item -ItemType Directory -Force -Path $whisperModelDir | Out-Null
    }
    $downloadScript = Join-Path $whisperModelDir "download-ggml-model.cmd"
    if (-not (Test-Path $downloadScript)) {
        throw "Whisper model is missing and no Windows download script was found at '$downloadScript'."
    }

    Write-Host "Downloading Whisper model..."
    Push-Location $whisperModelDir
    try {
        & $downloadScript base
    } finally {
        Pop-Location
    }
    $script:whisperModel = Join-Path $whisperModelDir "ggml-base.bin"
}

if (-not (Test-Path $whisperModel)) {
    throw "Whisper model still missing at '$whisperModel'. Download it manually and rerun."
}

New-Item -ItemType Directory -Force -Path $launcherDir | Out-Null
$launcherLines = @(
    '@echo off',
    'setlocal',
    "set ""LECTURE_WHISPER_CPP_DIR=$whisperCppDir""",
    "set ""LECTURE_WHISPER_BIN=$whisperBin""",
    "set ""LECTURE_WHISPER_MODEL_DIR=$whisperModelDir""",
    "set ""LECTURE_WHISPER_MODEL=$whisperModel""",
    """$lectureBin"" %*"
)
Set-Content -Path $launcherPath -Value $launcherLines -Encoding ASCII

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$pathsToAdd = @($launcherDir, $cargoBinDir)
foreach ($pathToAdd in $pathsToAdd) {
    if ([string]::IsNullOrWhiteSpace($userPath)) {
        $userPath = $pathToAdd
    } elseif (($userPath -split ';') -notcontains $pathToAdd) {
        $userPath = "$pathToAdd;$userPath"
    }
}
[Environment]::SetEnvironmentVariable("Path", $userPath, "User")

Write-Host ""
Write-Host "Installed."
Write-Host "Open a new PowerShell or CMD window, then use:"
Write-Host "  lecture devices"
Write-Host "  lecture start 3 clase-fisica2"
