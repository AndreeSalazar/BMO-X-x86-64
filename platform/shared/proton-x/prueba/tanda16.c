/* tanda16.c -- el .exe de la TANDA 16 de Cyberpunk (30-09): los aparatos
 * (DURAS del censo). HID (el GUID de la clase, y un aparato que no es),
 * SETUPAPI (una clase de aparatos que no existe: la lista vacia), CFGMGR32
 * (el nodo raiz, la lista de aparatos, una propiedad que no hay, los
 * errores) y WLDAP32: sus 18 ordinales, los del juego, se CARGAN (no se
 * llaman: libcurl solo los usa con ldap://).
 *
 * Tus aparatos no importan: la clase que se busca no existe en ningun
 * Windows. Sale con el numero de fallos. En Windows dice lo mismo. */
typedef void *HANDLE;
typedef unsigned long DWORD;
typedef unsigned short WCHAR;
#define IMPORTA __declspec(dllimport)
#define W __stdcall
IMPORTA HANDLE W GetStdHandle(DWORD n);
IMPORTA int W WriteFile(HANDLE h, const void *b, DWORD n, DWORD *e, void *o);
IMPORTA void W ExitProcess(unsigned c);
IMPORTA DWORD W GetLastError(void);
IMPORTA void W HidD_GetHidGuid(void *g);
IMPORTA int W HidD_GetAttributes(HANDLE h, void *a);
IMPORTA long W HidP_GetCaps(void *pp, void *caps);
IMPORTA HANDLE W SetupDiGetClassDevsW(const void *g, const WCHAR *e, HANDLE h, DWORD f);
IMPORTA int W SetupDiEnumDeviceInterfaces(HANDLE h, void *di, const void *g, DWORD i, void *d);
IMPORTA int W SetupDiDestroyDeviceInfoList(HANDLE h);
IMPORTA DWORD W CM_Locate_DevNodeW(DWORD *dn, const WCHAR *id, DWORD f);
IMPORTA DWORD W CM_Get_Device_ID_List_SizeW(DWORD *n, const WCHAR *filtro, DWORD f);
IMPORTA DWORD W CM_Get_Device_ID_ListW(const WCHAR *filtro, WCHAR *b, DWORD n, DWORD f);
IMPORTA DWORD W CM_Get_DevNode_PropertyW(DWORD dn, const void *clave, DWORD *tipo, void *b, DWORD *n, DWORD f);
IMPORTA DWORD W CM_MapCrToWin32Err(DWORD cr, DWORD por_defecto);
/* WLDAP32: los ordinales del juego (los nombres, en tanda16_wldap32.def). */
IMPORTA void *W ldap_22(void);
IMPORTA void *W ldap_26(void);
IMPORTA void *W ldap_27(void);
IMPORTA void *W ldap_30(void);
IMPORTA void *W ldap_32(void);
IMPORTA void *W ldap_33(void);
IMPORTA void *W ldap_35(void);
IMPORTA void *W ldap_41(void);
IMPORTA void *W ldap_45(void);
IMPORTA void *W ldap_46(void);
IMPORTA void *W ldap_50(void);
IMPORTA void *W ldap_60(void);
IMPORTA void *W ldap_79(void);
IMPORTA void *W ldap_143(void);
IMPORTA void *W ldap_200(void);
IMPORTA void *W ldap_211(void);
IMPORTA void *W ldap_217(void);
IMPORTA void *W ldap_301(void);

static unsigned fallos;

static void di(const char *t) {
    DWORD n = 0, k = 0;
    while (t[n])
        n++;
    WriteFile(GetStdHandle((DWORD)-11), t, n, &k, 0);
}

static void mira(int bien, const char *que) {
    if (!bien)
        fallos++;
    di(bien ? "  bien  " : "  MAL   ");
    di(que);
    di("\r\n");
}

static const unsigned char HID[16] = {0xB2, 0x55, 0x1E, 0x4D, 0x6F, 0xF1, 0xCF, 0x11, 0x88, 0xCB, 0x00, 0x11, 0x11, 0x00, 0x00, 0x30};
static const unsigned char NO_EXISTE[16] = {0x42, 0x4D, 0x4F, 0x58, 0x16, 0x16, 0x16, 0x16, 0xB0, 0x0B, 0x5A, 0x1E, 0x16, 0x16, 0x16, 0x16};
/* Una clave de propiedad (DEVPROPKEY: GUID y pid) que no existe. */
static const unsigned char CLAVE[20] = {0x42, 0x4D, 0x4F, 0x58, 0x16, 0x16, 0x16, 0x16, 0xB0, 0x0B, 0x5A, 0x1E, 0x16, 0x16, 0x16, 0x16, 2, 0, 0, 0};

/* Para que las 18 de WLDAP32 se importen (y el enlazador no las quite). */
static void *volatile ldap[18];

static WCHAR lista[1 << 18];

void inicio(void) {
    unsigned char g[16];
    int k, igual = 1;
    HidD_GetHidGuid(g);
    for (k = 0; k < 16; k++)
        igual &= g[k] == HID[k];
    mira(igual, "HidD_GetHidGuid: {4D1E55B2-F16F-11CF-88CB-001111000030}");
    {
        unsigned char a[12];
        static unsigned char pp[256], caps[64];
        mira(!HidD_GetAttributes((HANDLE)(long long)-1, a) && HidP_GetCaps(pp, caps) == (long)0xC0110001, "HidD_GetAttributes sin aparato: FALSE; HidP_GetCaps de algo que no es: HIDP_STATUS_INVALID_PREPARSED_DATA");
    }
    {
        HANDLE h = SetupDiGetClassDevsW(NO_EXISTE, 0, 0, 0x12);
        DWORD d[8] = {32};
        mira(h != (HANDLE)(long long)-1 && !SetupDiEnumDeviceInterfaces(h, 0, NO_EXISTE, 0, d) && GetLastError() == 259 && SetupDiDestroyDeviceInfoList(h), "SetupDiGetClassDevsW de una clase que no hay: la lista vacia (ERROR_NO_MORE_ITEMS)");
    }
    {
        DWORD dn = 0, n = 0, tipo = 0, t = 64;
        unsigned char b[64];
        mira(CM_Locate_DevNodeW(&dn, 0, 0) == 0 && dn != 0, "CM_Locate_DevNodeW: el nodo raiz");
        mira(CM_Get_Device_ID_List_SizeW(&n, 0, 0) == 0 && n >= 1 && n <= (1 << 18) && CM_Get_Device_ID_ListW(0, lista, n, 0) == 0, "CM_Get_Device_ID_List_SizeW y CM_Get_Device_ID_ListW");
        mira(CM_Get_DevNode_PropertyW(dn, CLAVE, &tipo, b, &t, 0) == 0x25, "CM_Get_DevNode_PropertyW de una propiedad que no hay: CR_NO_SUCH_VALUE");
    }
    mira(CM_MapCrToWin32Err(0, 5) == 0 && CM_MapCrToWin32Err(0x1A, 5) == 122 && CM_MapCrToWin32Err(0x25, 5) == 1168 && CM_MapCrToWin32Err(0x99, 5) == 5, "CM_MapCrToWin32Err: exito, bufer chico, no hay, y el de por defecto");
    ldap[0] = (void *)ldap_22, ldap[1] = (void *)ldap_26, ldap[2] = (void *)ldap_27, ldap[3] = (void *)ldap_30;
    ldap[4] = (void *)ldap_32, ldap[5] = (void *)ldap_33, ldap[6] = (void *)ldap_35, ldap[7] = (void *)ldap_41;
    ldap[8] = (void *)ldap_45, ldap[9] = (void *)ldap_46, ldap[10] = (void *)ldap_50, ldap[11] = (void *)ldap_60;
    ldap[12] = (void *)ldap_79, ldap[13] = (void *)ldap_143, ldap[14] = (void *)ldap_200, ldap[15] = (void *)ldap_211;
    ldap[16] = (void *)ldap_217, ldap[17] = (void *)ldap_301;
    {
        int todos = 1;
        for (k = 0; k < 18; k++)
            todos &= ldap[k] != 0;
        mira(todos, "WLDAP32: sus 18 ordinales (los del juego) se cargan");
    }
    di("tanda16.exe: los aparatos que no hay dicen lo de Windows\r\n");
    ExitProcess(fallos);
}
