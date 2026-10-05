// pix3.h -- los marcadores de PIX (WinPixEventRuntime de Microsoft), VACIOS.
// Las muestras de E2 los ponen alrededor de cada fase del fotograma; sin
// USE_PIX, el pix3.h de Microsoft los deja en nada, y este hace lo mismo:
// el .exe no importa WinPixEventRuntime.dll ni llama a nada por ellos.
#pragma once
#define PIXBeginEvent(...) ((void)0)
#define PIXEndEvent(...) ((void)0)
#define PIXSetMarker(...) ((void)0)
