# Windscribe Pro Hub

**Version:** 0.0.1  
**Platform:** Windows 10 / Windows 11  
**Stack:** Python 3.10+, CustomTkinter, Windows DPAPI, WinGet

A Windows desktop utility focused specifically on Windscribe.

## Features

- Detects Windscribe installation using WinGet package inventory.
- Checks whether a WinGet upgrade is available.
- Shows the correct action: Install, Update, or Open.
- Checks whether the Windscribe desktop process is running.
- Saves multiple Windscribe username/password pairs in a local DPAPI-encrypted vault.
- Shows or hides each saved password independently.
- Loads a selected account into the editor, copies username/password, and deletes accounts.
- Generates a username suggestion and a strong password. A suggested username is not registered automatically.
- Runs the two requested registry cleanup commands separately, exporting a .reg backup before deletion.
- Displays operation output in the activity log.

## Run from source

    py -m pip install -r requirements.txt
    py WindscribeHub.py

## Build EXE

    Set-ExecutionPolicy -Scope Process Bypass
    .\Build.ps1

The executable is written to dist\WindscribePro.exe.

## Credential vault

Stored at %LOCALAPPDATA%\WindscribeHub\accounts.vault. Passwords are encrypted with Windows DPAPI for the current Windows user. The vault is not portable to another Windows account or machine. Never commit real credentials or the vault file to Git.

## Registry commands

1. reg delete "HKCU\Software\Microsoft\Windows\CurrentVersion\NetControlPort" /f
2. reg delete "HKCU\Software\Windscribe\Windscribe2" /f

The application asks for confirmation and exports the key before deletion.

## Limitations

- WinGet is required for reliable package install/update detection.
- Username generation creates a suggestion only; it does not create an account.
- Validate on your Windows installation and report any errors with the Activity Log output.

## License

MIT.
