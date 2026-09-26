@echo off
rem Compila musica_host.exe con el cl de Visual Studio y renderiza las canciones
rem de doom1.wad a out\musica\*.wav. Uso: musica_host.bat [D_E1M1 ...]
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul
cd /d "%~dp0"
if not exist out\musica mkdir out\musica
cl /nologo /O2 /W3 /Fe:out\musica_host.exe /Fo:out\ musica_host.c || exit /b 1
set CANCIONES=%*
if "%CANCIONES%"=="" set CANCIONES=D_E1M1
for %%c in (%CANCIONES%) do out\musica_host.exe ..\doom\doom1.wad %%c out\musica\%%c.wav || exit /b 1
