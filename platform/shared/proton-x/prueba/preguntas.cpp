// preguntas.cpp -- el juez de las preguntas de CheckFeatureSupport que la
// casa no contestaba (06-10, A8 del contador de DX12). Un motor pregunta,
// al montar su D3D12, la cache de sombreadores, las prioridades de cola, la
// serializacion de montones... y una que no se contesta (E_INVALIDARG con
// la medida buena) la toma por un fallo del dispositivo o por "no hay",
// segun el motor. Lo que DICE cada una puede ser otro en la 3060 (la casa
// dice que no a lo que no hace); lo que se juzga es que conteste, con la
// medida exacta, y lo que tiene que ser igual en las dos.
//
//   A  SHADER_CACHE: S_OK.
//   B  COMMAND_QUEUE_PRIORITY: DIRECT a NORMAL y COMPUTE a HIGH, si.
//   C  EXISTING_HEAPS: S_OK.
//   D  SERIALIZATION del nodo 0: S_OK; del nodo 1 (no hay): E_INVALIDARG.
//   E  CROSS_NODE: S_OK.
//   F  DISPLAYABLE: S_OK.
//   G  PROTECTED_RESOURCE_SESSION_SUPPORT del nodo 0: S_OK.
//   H  Las siete, con la medida de mas (4 bytes): E_INVALIDARG.
//   nota  OPTIONS13 a OPTIONS21, PREDICATION y HARDWARE_COPY: lo que
//      contesta Windows (las mas nuevas, segun su version, no las sabe).
//
// Sale con el numero de fallos; en Windows dice lo mismo.
#include <windows.h>
#include <d3d12.h>
#include <dxguids/dxguids.h>
#include <cstdio>
#include <cstring>

static int fallos = 0;

static void decir(bool bien, const char *que) {
    printf("%s%s\n", bien ? "  bien  " : "  MAL   ", que);
    if (!bien)
        fallos++;
}

int main() {
    ID3D12Device *d = nullptr;
    HRESULT h = D3D12CreateDevice(nullptr, D3D_FEATURE_LEVEL_11_0, IID_PPV_ARGS(&d));
    if (FAILED(h)) {
        printf("  MAL   D3D12CreateDevice: HRESULT 0x%08lx\n", (unsigned long)h);
        return 1;
    }
    char msg[300];
    // Preguntar `que` con `n` bytes en `p`.
    auto pide = [&](D3D12_FEATURE que, void *p, UINT n) { return d->CheckFeatureSupport(que, p, n); };

    D3D12_FEATURE_DATA_SHADER_CACHE cache = {};
    h = pide(D3D12_FEATURE_SHADER_CACHE, &cache, sizeof cache);
    snprintf(msg, sizeof msg, "A, SHADER_CACHE: 0x%08lx (SupportFlags %#x)", (unsigned long)h, (unsigned)cache.SupportFlags);
    decir(h == S_OK, msg);

    D3D12_FEATURE_DATA_COMMAND_QUEUE_PRIORITY p1 = {D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_PRIORITY_NORMAL, FALSE};
    D3D12_FEATURE_DATA_COMMAND_QUEUE_PRIORITY p2 = {D3D12_COMMAND_LIST_TYPE_COMPUTE, D3D12_COMMAND_QUEUE_PRIORITY_HIGH, FALSE};
    HRESULT h1 = pide(D3D12_FEATURE_COMMAND_QUEUE_PRIORITY, &p1, sizeof p1), h2 = pide(D3D12_FEATURE_COMMAND_QUEUE_PRIORITY, &p2, sizeof p2);
    snprintf(msg, sizeof msg, "B, COMMAND_QUEUE_PRIORITY: DIRECT a NORMAL 0x%08lx (%d), COMPUTE a HIGH 0x%08lx (%d)", (unsigned long)h1, p1.PriorityForTypeIsSupported, (unsigned long)h2, p2.PriorityForTypeIsSupported);
    decir(h1 == S_OK && h2 == S_OK && p1.PriorityForTypeIsSupported && p2.PriorityForTypeIsSupported, msg);

    D3D12_FEATURE_DATA_EXISTING_HEAPS existentes = {};
    h = pide(D3D12_FEATURE_EXISTING_HEAPS, &existentes, sizeof existentes);
    snprintf(msg, sizeof msg, "C, EXISTING_HEAPS: 0x%08lx (Supported %d)", (unsigned long)h, existentes.Supported);
    decir(h == S_OK, msg);

    D3D12_FEATURE_DATA_SERIALIZATION s0 = {0, D3D12_HEAP_SERIALIZATION_TIER_0}, s1 = {1, D3D12_HEAP_SERIALIZATION_TIER_0};
    h1 = pide(D3D12_FEATURE_SERIALIZATION, &s0, sizeof s0);
    h2 = pide(D3D12_FEATURE_SERIALIZATION, &s1, sizeof s1);
    snprintf(msg, sizeof msg, "D, SERIALIZATION del nodo 0: 0x%08lx (tier %d); del nodo 1, que no hay: 0x%08lx", (unsigned long)h1, (int)s0.HeapSerializationTier, (unsigned long)h2);
    decir(h1 == S_OK && h2 == E_INVALIDARG, msg);

    D3D12_FEATURE_DATA_CROSS_NODE cruce = {};
    h = pide(D3D12_FEATURE_CROSS_NODE, &cruce, sizeof cruce);
    snprintf(msg, sizeof msg, "E, CROSS_NODE: 0x%08lx (tier %d, atomicos %d)", (unsigned long)h, (int)cruce.SharingTier, cruce.AtomicShaderInstructions);
    decir(h == S_OK, msg);

    D3D12_FEATURE_DATA_DISPLAYABLE pantalla = {};
    h = pide(D3D12_FEATURE_DISPLAYABLE, &pantalla, sizeof pantalla);
    snprintf(msg, sizeof msg, "F, DISPLAYABLE: 0x%08lx (%d, tier %d)", (unsigned long)h, pantalla.DisplayableTexture, (int)pantalla.SharedResourceCompatibilityTier);
    decir(h == S_OK, msg);

    D3D12_FEATURE_DATA_PROTECTED_RESOURCE_SESSION_SUPPORT protegida = {0, D3D12_PROTECTED_RESOURCE_SESSION_SUPPORT_FLAG_NONE};
    h = pide(D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_SUPPORT, &protegida, sizeof protegida);
    snprintf(msg, sizeof msg, "G, PROTECTED_RESOURCE_SESSION_SUPPORT del nodo 0: 0x%08lx (%#x)", (unsigned long)h, (unsigned)protegida.Support);
    decir(h == S_OK, msg);

    // H: con 4 bytes de mas (el nodo y lo que entra, en su sitio).
    unsigned char sobra[64] = {};
    const struct {
        D3D12_FEATURE que;
        UINT medida;
    } todas[7] = {{D3D12_FEATURE_SHADER_CACHE, sizeof cache}, {D3D12_FEATURE_COMMAND_QUEUE_PRIORITY, sizeof p1}, {D3D12_FEATURE_EXISTING_HEAPS, sizeof existentes}, {D3D12_FEATURE_SERIALIZATION, sizeof s0}, {D3D12_FEATURE_CROSS_NODE, sizeof cruce}, {D3D12_FEATURE_DISPLAYABLE, sizeof pantalla}, {D3D12_FEATURE_PROTECTED_RESOURCE_SESSION_SUPPORT, sizeof protegida}};
    UINT malas = 0;
    for (const auto &t : todas) {
        memset(sobra, 0, sizeof sobra);
        malas += pide(t.que, sobra, t.medida + 4) != E_INVALIDARG;
    }
    snprintf(msg, sizeof msg, "H, las siete con 4 bytes de mas: E_INVALIDARG (%u no)", malas);
    decir(malas == 0, msg);

    // La nota: las nuevas, lo que diga cada una.
    const struct {
        const char *nombre;
        int que;
        UINT medida;
    } nuevas[11] = {{"OPTIONS13", 42, 24}, {"OPTIONS14", 43, 12}, {"OPTIONS15", 44, 8}, {"OPTIONS16", 45, 8}, {"OPTIONS17", 46, 8}, {"OPTIONS18", 47, 4}, {"OPTIONS19", 48, 40}, {"OPTIONS20", 49, 8}, {"OPTIONS21", 53, 16}, {"PREDICATION", 50, 4}, {"HARDWARE_COPY", 52, 4}};
    int w = snprintf(msg, sizeof msg, "  nota  las nuevas (S_OK = s, otra = n):");
    for (const auto &n : nuevas) {
        memset(sobra, 0, sizeof sobra);
        HRESULT r = pide((D3D12_FEATURE)n.que, sobra, n.medida);
        w += snprintf(msg + w, sizeof msg - w, " %s %s", n.nombre, r == S_OK ? "s" : "n");
    }
    printf("%s\n", msg);

    printf("preguntas.exe: CheckFeatureSupport contesta lo que contesta Windows\n");
    return fallos;
}
