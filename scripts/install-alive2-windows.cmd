@echo off
setlocal EnableExtensions

set "TOOLCHAIN_ROOT=C:\toolchains"
set "SRC_ROOT=%TOOLCHAIN_ROOT%\src"
set "BUILD_ROOT=%TOOLCHAIN_ROOT%\build"
set "INSTALL_ROOT=%TOOLCHAIN_ROOT%\install"

set "LLVM_VERSION=21.1.8"
set "LLVM_TAG=llvmorg-%LLVM_VERSION%"
set "LLVM_ARCHIVE=%SRC_ROOT%\%LLVM_TAG%.zip"
set "LLVM_SOURCE_URL=https://codeload.github.com/llvm/llvm-project/zip/refs/tags/%LLVM_TAG%"
set "LLVM_SRC=%SRC_ROOT%\llvm-project-%LLVM_VERSION%"
set "LLVM_EXTRACTED=%SRC_ROOT%\llvm-project-%LLVM_TAG%"
set "LLVM_BUILD=%BUILD_ROOT%\llvm-%LLVM_VERSION%"
set "LLVM_INSTALL=%INSTALL_ROOT%\llvm-%LLVM_VERSION%"

set "ALIVE2_VERSION=v21.0"
set "ALIVE2_ARCHIVE=%SRC_ROOT%\alive2-%ALIVE2_VERSION%.zip"
set "ALIVE2_SOURCE_URL=https://codeload.github.com/AliveToolkit/alive2/zip/refs/tags/%ALIVE2_VERSION%"
set "ALIVE2_SRC=%SRC_ROOT%\alive2-%ALIVE2_VERSION%"
set "ALIVE2_EXTRACTED=%SRC_ROOT%\alive2-21.0"
set "ALIVE2_BUILD=%BUILD_ROOT%\alive2-%ALIVE2_VERSION%"
set "ALIVE2_BIN=%ALIVE2_BUILD%\alive-tv.exe"

set "Z3_ROOT=C:\ProgramData\chocolatey\lib\z3\tools\bin"
set "Z3_INCLUDE_DIR=%Z3_ROOT%\include"
set "Z3_LIBRARY=%Z3_ROOT%\bin\libz3.lib"
set "Z3_DLL=%Z3_ROOT%\bin\libz3.dll"

call :banner "Locating required tools"
call :find_cmake || exit /b 1
call :find_vsdevcmd || exit /b 1
call :require_command git || exit /b 1
call :require_command curl.exe || exit /b 1
call :require_command powershell.exe || exit /b 1
call :require_command ninja.exe || exit /b 1
call :require_command re2c.exe || exit /b 1
call :require_command tar.exe || exit /b 1

if not exist "%Z3_INCLUDE_DIR%\z3++.h" (
  call :die "Missing Z3 headers at %Z3_INCLUDE_DIR%\z3++.h"
  exit /b 1
)
if not exist "%Z3_LIBRARY%" (
  call :die "Missing Z3 import library at %Z3_LIBRARY%"
  exit /b 1
)
if not exist "%Z3_DLL%" (
  call :die "Missing Z3 runtime DLL at %Z3_DLL%"
  exit /b 1
)

call :banner "Preparing directories"
if not exist "%TOOLCHAIN_ROOT%" mkdir "%TOOLCHAIN_ROOT%"
if not exist "%SRC_ROOT%" mkdir "%SRC_ROOT%"
if not exist "%BUILD_ROOT%" mkdir "%BUILD_ROOT%"
if not exist "%INSTALL_ROOT%" mkdir "%INSTALL_ROOT%"

call "%VSDEVCMD%" -host_arch=amd64 -arch=amd64 >nul
if errorlevel 1 (
  call :die "Failed to initialize the Visual Studio build environment"
  exit /b 1
)

where cl >nul 2>&1
if errorlevel 1 (
  call :die "MSVC compiler (cl.exe) is not available after calling VsDevCmd"
  exit /b 1
)

set "PATH=%Z3_ROOT%\bin;%PATH%"

call :banner "Resetting dedicated versioned directories"
call :remove_tree "%LLVM_SRC%" || exit /b 1
call :remove_tree "%LLVM_EXTRACTED%" || exit /b 1
call :remove_tree "%LLVM_BUILD%" || exit /b 1
call :remove_tree "%LLVM_INSTALL%" || exit /b 1
call :remove_tree "%ALIVE2_SRC%" || exit /b 1
call :remove_tree "%ALIVE2_EXTRACTED%" || exit /b 1
call :remove_tree "%ALIVE2_BUILD%" || exit /b 1

call :download_if_missing "%LLVM_ARCHIVE%" "%LLVM_SOURCE_URL%" || exit /b 1
call :download_if_missing "%ALIVE2_ARCHIVE%" "%ALIVE2_SOURCE_URL%" || exit /b 1

call :banner "Extracting LLVM %LLVM_VERSION%"
tar -xf "%LLVM_ARCHIVE%" -C "%SRC_ROOT%"
if errorlevel 1 (
  call :die "Failed to extract %LLVM_ARCHIVE%"
  exit /b 1
)
if not exist "%LLVM_EXTRACTED%" (
  call :die "Expected extracted LLVM directory %LLVM_EXTRACTED% was not created"
  exit /b 1
)
rename "%LLVM_EXTRACTED%" "llvm-project-%LLVM_VERSION%"
if errorlevel 1 (
  call :die "Failed to rename %LLVM_EXTRACTED% to %LLVM_SRC%"
  exit /b 1
)
if not exist "%LLVM_SRC%\llvm\CMakeLists.txt" (
  call :die "LLVM source tree is missing %LLVM_SRC%\llvm\CMakeLists.txt"
  exit /b 1
)

call :banner "Configuring LLVM %LLVM_VERSION%"
"%CMAKE_EXE%" -S "%LLVM_SRC%\llvm" ^
  -B "%LLVM_BUILD%" ^
  -G Ninja ^
  -DCMAKE_BUILD_TYPE=Release ^
  -DLLVM_ENABLE_RTTI=ON ^
  -DLLVM_ENABLE_EH=ON ^
  -DBUILD_SHARED_LIBS=ON ^
  -DLLVM_ENABLE_ASSERTIONS=ON ^
  -DLLVM_ENABLE_PROJECTS=clang ^
  -DCMAKE_INSTALL_PREFIX="%LLVM_INSTALL%"
if errorlevel 1 (
  call :die "LLVM configure failed"
  exit /b 1
)

call :banner "Building and installing LLVM %LLVM_VERSION%"
"%CMAKE_EXE%" --build "%LLVM_BUILD%" --config Release --target install
if errorlevel 1 (
  call :die "LLVM build/install failed"
  exit /b 1
)

if not exist "%LLVM_INSTALL%\bin\clang.exe" (
  call :die "clang.exe was not installed to %LLVM_INSTALL%\bin"
  exit /b 1
)
if not exist "%LLVM_INSTALL%\bin\llvm-config.exe" (
  call :die "llvm-config.exe was not installed to %LLVM_INSTALL%\bin"
  exit /b 1
)

call :banner "Extracting Alive2 %ALIVE2_VERSION%"
tar -xf "%ALIVE2_ARCHIVE%" -C "%SRC_ROOT%"
if errorlevel 1 (
  call :die "Failed to extract %ALIVE2_ARCHIVE%"
  exit /b 1
)
if not exist "%ALIVE2_EXTRACTED%\CMakeLists.txt" (
  call :die "Expected extracted Alive2 directory %ALIVE2_EXTRACTED% was not created"
  exit /b 1
)
rename "%ALIVE2_EXTRACTED%" "alive2-%ALIVE2_VERSION%"
if errorlevel 1 (
  call :die "Failed to rename %ALIVE2_EXTRACTED% to %ALIVE2_SRC%"
  exit /b 1
)
if not exist "%ALIVE2_SRC%\CMakeLists.txt" (
  call :die "Alive2 source tree is missing %ALIVE2_SRC%\CMakeLists.txt"
  exit /b 1
)

call :banner "Configuring Alive2 %ALIVE2_VERSION%"
"%CMAKE_EXE%" -S "%ALIVE2_SRC%" ^
  -B "%ALIVE2_BUILD%" ^
  -G Ninja ^
  -DCMAKE_BUILD_TYPE=Release ^
  -DBUILD_TV=1 ^
  -DCMAKE_PREFIX_PATH="%LLVM_INSTALL%" ^
  -DZ3_INCLUDE_DIR="%Z3_INCLUDE_DIR%" ^
  -DZ3_LIBRARIES="%Z3_LIBRARY%"
if errorlevel 1 (
  call :die "Alive2 configure failed"
  exit /b 1
)

call :banner "Building Alive2 %ALIVE2_VERSION%"
"%CMAKE_EXE%" --build "%ALIVE2_BUILD%" --config Release
if errorlevel 1 (
  call :die "Alive2 build failed"
  exit /b 1
)

if not exist "%ALIVE2_BIN%" (
  call :die "alive-tv.exe was not produced at %ALIVE2_BIN%"
  exit /b 1
)

call :banner "Copying runtime DLLs beside alive-tv.exe"
copy /Y "%Z3_DLL%" "%ALIVE2_BUILD%\" >nul
if errorlevel 1 (
  call :die "Failed to copy %Z3_DLL% beside alive-tv.exe"
  exit /b 1
)
copy /Y "%LLVM_INSTALL%\bin\*.dll" "%ALIVE2_BUILD%\" >nul
if errorlevel 1 (
  call :die "Failed to copy LLVM runtime DLLs beside alive-tv.exe"
  exit /b 1
)

call :banner "Verifying installed binaries"
"%LLVM_INSTALL%\bin\clang.exe" --version || (
  call :die "clang.exe verification failed"
  exit /b 1
)
"%LLVM_INSTALL%\bin\llvm-config.exe" --version || (
  call :die "llvm-config.exe verification failed"
  exit /b 1
)
"%ALIVE2_BIN%" --help || (
  call :die "alive-tv.exe verification failed"
  exit /b 1
)

echo.
echo [alive2-setup] Success.
echo [alive2-setup] LLVM install: %LLVM_INSTALL%
echo [alive2-setup] Alive2 binary: %ALIVE2_BIN%
echo [alive2-setup] RepoBrain flag:
echo   --theorem-alive2-path %ALIVE2_BIN%
exit /b 0

:banner
echo.
echo [alive2-setup] %~1
echo.
exit /b 0

:die
echo.
echo [alive2-setup] ERROR: %~1
exit /b 1

:find_cmake
set "CMAKE_EXE="
if exist "C:\Program Files\CMake\bin\cmake.exe" set "CMAKE_EXE=C:\Program Files\CMake\bin\cmake.exe"
if not defined CMAKE_EXE if exist "C:\Program Files (x86)\CMake\bin\cmake.exe" set "CMAKE_EXE=C:\Program Files (x86)\CMake\bin\cmake.exe"
if not defined CMAKE_EXE (
  for /f "delims=" %%I in ('where cmake 2^>nul') do if not defined CMAKE_EXE set "CMAKE_EXE=%%~fI"
)
if not defined CMAKE_EXE call :die "CMake was not found. Install CMake or update the script paths."
exit /b 0

:find_vsdevcmd
set "VSDEVCMD="
for %%I in (
  "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
  "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"
  "C:\Program Files (x86)\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat"
  "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat"
  "C:\Program Files (x86)\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat"
  "C:\Program Files\Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat"
  "C:\Program Files (x86)\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat"
  "C:\Program Files\Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat"
 ) do (
  if not defined VSDEVCMD if exist %%~I set "VSDEVCMD=%%~I"
 )
if not defined VSDEVCMD call :die "VsDevCmd.bat was not found. Install Visual Studio Build Tools 2022 with C++ support."
exit /b 0

:require_command
where %~1 >nul 2>&1
if errorlevel 1 call :die "Required command not found on PATH: %~1"
exit /b 0

:download_if_missing
if exist "%~1" (
  echo [alive2-setup] Reusing existing archive %~1
  exit /b 0
)
echo [alive2-setup] Downloading %~2
curl.exe -L --retry 5 --retry-all-errors -o "%~1" "%~2"
if errorlevel 1 call :die "Download failed for %~2"
exit /b 0

:remove_tree
set "TARGET=%~1"
if /I not "%TARGET:~0,13%"=="C:\toolchains" call :die "Refusing to remove non-toolchains path: %TARGET%"
if exist "%TARGET%" rmdir /s /q "%TARGET%"
if exist "%TARGET%" call :die "Failed to remove %TARGET%"
exit /b 0
