/* Minimal <string.h> for probing BMO C against DOOM. Declarations only. */
#ifndef BMO_PROBE_STRING_H
#define BMO_PROBE_STRING_H

#ifndef NULL
#define NULL 0
#endif

void *memcpy(void *dst, const void *src, unsigned long n);
void *memmove(void *dst, const void *src, unsigned long n);
void *memset(void *dst, int v, unsigned long n);
int memcmp(const void *a, const void *b, unsigned long n);

unsigned long strlen(const char *s);
char *strcpy(char *dst, const char *src);
char *strncpy(char *dst, const char *src, unsigned long n);
char *strcat(char *dst, const char *src);
char *strncat(char *dst, const char *src, unsigned long n);
int strcmp(const char *a, const char *b);
int strncmp(const char *a, const char *b, unsigned long n);
int strcasecmp(const char *a, const char *b);
int strncasecmp(const char *a, const char *b, unsigned long n);
char *strchr(const char *s, int c);
char *strrchr(const char *s, int c);
char *strstr(const char *hay, const char *needle);
char *strdup(const char *s);

#endif
