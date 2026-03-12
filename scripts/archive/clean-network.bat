@echo off
echo ========================================
echo   COINjecture Network Cleanup
echo ========================================
echo.
echo This will DELETE all node data folders:
echo   - node1-data (Full/Bootstrap)
echo   - node2-data (Archive)
echo   - node3-data (Validator)
echo   - node4-data (Bounty)
echo   - node5-data (Oracle)
echo   - node6-data (Light)
echo.
pause

cd /d c:\Users\LEET\COINjecture1337-NETB

echo Removing node1-data...
if exist node1-data rmdir /s /q node1-data
echo Removing node2-data...
if exist node2-data rmdir /s /q node2-data
echo Removing node3-data...
if exist node3-data rmdir /s /q node3-data
echo Removing node4-data...
if exist node4-data rmdir /s /q node4-data
echo Removing node5-data...
if exist node5-data rmdir /s /q node5-data
echo Removing node6-data...
if exist node6-data rmdir /s /q node6-data

echo.
echo ========================================
echo   Cleanup Complete!
echo ========================================
echo.
echo You can now run: run-all-6-nodes.bat
echo.
pause

