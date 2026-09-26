/* SDL_mixer.h -- UN ARMAZON VACIO, y esta vacio por un motivo comprobado.
 *
 * `i_sound.c` lo incluye cuando `FEATURE_SOUND` esta definido:
 *
 *     #if defined(FEATURE_SOUND) && !defined(__DJGPP__)
 *     #include <SDL_mixer.h>
 *     #endif
 *
 * ...y despues **no usa ni un simbolo suyo**: las tres unicas apariciones de
 * "SDL_mixer" en ese fichero son comentarios sobre versiones viejas de la
 * biblioteca. Lo comprobado con un `grep` antes de escribir esto.
 *
 * Asi que la eleccion era: tocar `i_sound.c` --y perder la propiedad de que
 * DOOM esta sin tocar una coma-- o poner aqui un fichero que existe y no dice
 * nada. Lo segundo, y dicho.
 *
 * [!] Si algun dia una version de doomgeneric SI llama a un `Mix_*`, esto
 * dejara de compilar con "no existe" en vez de enlazar algo que no esta. Que
 * es lo correcto: un armazon vacio falla en voz alta.
 */
#ifndef BMO_SDL_MIXER_ARMAZON_H
#define BMO_SDL_MIXER_ARMAZON_H
#endif
