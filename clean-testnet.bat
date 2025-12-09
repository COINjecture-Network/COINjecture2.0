@echo off
REM Clean up testnet data
echo Cleaning up testnet data...

if exist "testnet" (
    rmdir /s /q testnet
    echo Testnet data removed.
) else (
    echo No testnet data found.
)

pause
