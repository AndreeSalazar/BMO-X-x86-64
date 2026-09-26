@echo off
rem Una nota sola por instrumento, para medir su frecuencia con frecuencia.py.
rem Uso: notas.bat  (compila y toca LA=tecla 69 en unos instrumentos)
setlocal
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat" >nul 2>nul
cd /d "%~dp0"
if not exist out\musica mkdir out\musica
cl /nologo /O2 /W3 /D_CRT_SECURE_NO_WARNINGS /Fe:out\musica_host.exe /Fo:out\ musica_host.c >nul || exit /b 1
for %%i in (0 19 29 30 33 48 56 80) do out\musica_host.exe ..\doom\doom1.wad NOTA out\musica\nota_%%i.wav %%i 69 >nul
