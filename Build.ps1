$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
py -m pip install --upgrade pip
py -m pip install -r requirements.txt pyinstaller
py tools\make_icon.py
py -m PyInstaller --noconfirm --clean --onefile --windowed --name "WindscribePro" --icon "assets\WindscribeHub.ico" --add-data "assets\WindscribeHub.ico;assets" WindscribeHub.py
Write-Host "Build complete: $PSScriptRoot\dist\WindscribePro.exe" -ForegroundColor Green
