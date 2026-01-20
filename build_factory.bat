@echo off
echo 🛠️ Starting KORVEX Factory Build...

:: 1. Build Commercial Version
echo 💎 Building Commercial Edition...
cargo build --release --features commercial
mkdir "dist\commercial"
copy "target\release\korvex-video-engine.exe" "dist\commercial\korvex-factory-pro.exe"
copy "LICENSE_COMMERCIAL.txt" "dist\commercial\LICENSE.txt"

:: 2. Build Community Version
echo 🛡️ Building Community Edition...
cargo build --release --features community
mkdir "dist\community"
copy "target\release\korvex-video-engine.exe" "dist\community\korvex-factory-demo.exe"
copy "LICENSE_COMMUNITY.txt" "dist\community\LICENSE.txt"

echo ✅ Build Complete! Check the "dist" folder.
pause