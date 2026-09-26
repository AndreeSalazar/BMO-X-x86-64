/* sys/stat.h -- POSIX shim for the port. NOT part of BMO's own headers.
 *
 * `mkdir` is POSIX, not ISO C, so it does not belong in the repo's <stdlib.h>:
 * BMO-X has no directory-creation operation at all. It answers -1, which is
 * what "could not create it" looks like to the caller, and DOOM's
 * M_MakeDirectory does not check the result.
 */
#ifndef BMO_PROBE_SYS_STAT_H
#define BMO_PROBE_SYS_STAT_H

int mkdir(const char *ruta, int modo) {
    return -1;
}

#endif
