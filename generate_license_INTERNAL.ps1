# ============================================================
# KORVEX VIDEO ENGINE - PRIVATE ADMIN LICENSE GENERATOR
# ============================================================
Clear-Host
Write-Host "--- KORVEX Internal License Factory v1.1 ---" -ForegroundColor Gold

# 1. Input de la client
$CustomerHWID = Read-Host "Introduceți Machine ID (HWID) primit de la client"

if ([string]::IsNullOrWhiteSpace($CustomerHWID)) {
    Write-Host " Eroare: HWID-ul nu poate fi gol!" -ForegroundColor Red
    pause
    return
}

# 2. Securitate (Trebuie să coincidă cu ce avem în Rust)
$Salt = "KORVEX_GOLD_SALT"
$FullString = $CustomerHWID + $Salt

# 3. Generare Cheie SHA256
$Hasher = [System.Security.Cryptography.HashAlgorithm]::Create("SHA256")
$Bytes = [System.Text.Encoding]::UTF8.GetBytes($FullString)
$HashBytes = $Hasher.ComputeHash($Bytes)

# Convertim în format compatibil cu motorul (16 caractere hex)
$LicenseKey = [System.BitConverter]::ToString($HashBytes).Replace("-", "").ToLower().Substring(0,16)

# 4. Export fișier licență
$LicenseKey | Out-File "license.key" -Encoding ascii

Write-Host "
============================================================" -ForegroundColor White
Write-Host " GENERARE REUȘITĂ!" -ForegroundColor Green
Write-Host " LICENSE KEY: $LicenseKey" -ForegroundColor Cyan
Write-Host " FIȘIER CREAT: license.key" -ForegroundColor Yellow
Write-Host "============================================================" -ForegroundColor White
Write-Host "Instrucțiuni: Trimiteți fișierul 'license.key' clientului." -ForegroundColor Gray
pause
