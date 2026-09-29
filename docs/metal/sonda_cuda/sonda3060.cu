// sonda3060.cu -- lo que CUDA dice y hace con la RTX 3060, medido desde dentro.
//
// Siete partes, cada una una linea con su etiqueta entre corchetes:
//   [dispositivo] [atributo]  los 159 atributos que el driver contesta
//   [modulo]                  las DLL que CUDA carga en el proceso
//   [asa]                     el valor de cudaTextureObject_t / cudaSurfaceObject_t
//   [especial]                los registros especiales de PTX (el SASS dice de
//                             que palabra del banco constante 0 sale cada uno)
//   [pitch]                   la misma textura en cudaArray y en PITCH, bit a bit
//   [tiempo] [reloj_sm] [globaltimer] [pcie] [vram]   lo que cuesta cada cosa
//
// atributos.h sale de cuda.h con atributos.py (los nombres de CU_DEVICE_ATTRIBUTE_*).
// Compilar: nvcc -arch=sm_86 -O2 -std=c++17 -o sonda3060.exe sonda3060.cu -lcuda -lpsapi
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cstdint>
#include <vector>
#include <algorithm>
#include <chrono>
#include <string>
#include <windows.h>
#include <psapi.h>
#include <cuda.h>
#include <cuda_runtime.h>
#include "atributos.h"

#define R(x) do { cudaError_t e_ = (x); if (e_ != cudaSuccess) { \
    printf("ERROR %s:%d %s -> %s\n", __FILE__, __LINE__, #x, cudaGetErrorString(e_)); exit(1);} } while (0)
#define D(x) do { CUresult e_ = (x); if (e_ != CUDA_SUCCESS) { const char *s_ = 0; \
    cuGetErrorName(e_, &s_); printf("ERROR %s:%d %s -> %s\n", __FILE__, __LINE__, #x, s_ ? s_ : "?"); exit(1);} } while (0)

typedef std::chrono::steady_clock Reloj;
static double us_desde(Reloj::time_point a) {
    return std::chrono::duration<double, std::micro>(Reloj::now() - a).count();
}

// ---------------------------------------------------------------- kernels
extern "C" __global__ void vacio() {}

#define ESP32(nombre, i) asm volatile("mov.u32 %0, %%" nombre ";" : "=r"(o[i]))
extern "C" __global__ void especiales(unsigned int *o, unsigned long long *o64)
{
    ESP32("ntid.x", 0);  ESP32("ntid.y", 1);  ESP32("ntid.z", 2);
    ESP32("nctaid.x", 3); ESP32("nctaid.y", 4); ESP32("nctaid.z", 5);
    ESP32("nsmid", 6); ESP32("smid", 7); ESP32("nwarpid", 8); ESP32("warpid", 9);
    ESP32("laneid", 10); ESP32("dynamic_smem_size", 11); ESP32("total_smem_size", 12);
    ESP32("envreg0", 16); ESP32("envreg1", 17); ESP32("envreg2", 18); ESP32("envreg3", 19);
    ESP32("envreg4", 20); ESP32("envreg5", 21); ESP32("envreg6", 22); ESP32("envreg7", 23);
    ESP32("envreg8", 24); ESP32("envreg9", 25); ESP32("envreg10", 26); ESP32("envreg11", 27);
    ESP32("envreg12", 28); ESP32("envreg13", 29); ESP32("envreg14", 30); ESP32("envreg15", 31);
    ESP32("envreg16", 32); ESP32("envreg17", 33); ESP32("envreg18", 34); ESP32("envreg19", 35);
    ESP32("envreg20", 36); ESP32("envreg21", 37); ESP32("envreg22", 38); ESP32("envreg23", 39);
    ESP32("envreg24", 40); ESP32("envreg25", 41); ESP32("envreg26", 42); ESP32("envreg27", 43);
    ESP32("envreg28", 44); ESP32("envreg29", 45); ESP32("envreg30", 46); ESP32("envreg31", 47);
    unsigned long long g;
    asm volatile("mov.u64 %0, %%gridid;" : "=l"(g)); o64[0] = g;
    asm volatile("mov.u64 %0, %%clock64;" : "=l"(g)); o64[1] = g;
    asm volatile("mov.u64 %0, %%globaltimer;" : "=l"(g)); o64[2] = g;
}

// Cuantos ns avanza %globaltimer de una lectura a la siguiente que cambia.
extern "C" __global__ void pasos_globaltimer(unsigned long long *o, int n)
{
    unsigned long long a, b;
    asm volatile("mov.u64 %0, %%globaltimer;" : "=l"(a));
    for (int i = 0; i < n; i++) {
        do { asm volatile("mov.u64 %0, %%globaltimer;" : "=l"(b)); } while (b == a);
        o[i] = b - a;
        a = b;
    }
}

// Ciclos del SM (clock64) contra ns del globaltimer: el reloj real del SM.
extern "C" __global__ void reloj_sm(unsigned long long *o, long long ciclos)
{
    unsigned long long c0, c1, t0, t1;
    asm volatile("mov.u64 %0, %%globaltimer;" : "=l"(t0));
    c0 = clock64();
    do { c1 = clock64(); } while ((long long)(c1 - c0) < ciclos);
    asm volatile("mov.u64 %0, %%globaltimer;" : "=l"(t1));
    o[0] = c1 - c0; o[1] = t1 - t0;
}

extern "C" __global__ void calentar(float *x, int n)
{
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) { float v = x[i]; for (int k = 0; k < 4096; k++) v = v * 1.0000001f + 0.5f; x[i] = v; }
}

// ---------------------------------------------------------------- partes
static void parte_dispositivo(CUdevice dev)
{
    char nombre[256];
    D(cuDeviceGetName(nombre, sizeof nombre, dev));
    size_t total = 0; D(cuDeviceTotalMem(&total, dev));
    int drv = 0; D(cuDriverGetVersion(&drv));
    int rt = 0; R(cudaRuntimeGetVersion(&rt));
    printf("[dispositivo] nombre=%s memoria=%zu B (%.1f MiB) driver_api=%d runtime=%d\n",
           nombre, total, total / 1048576.0, drv, rt);
    for (auto &a : ATRIBUTOS) {
        int v = 0;
        CUresult e = cuDeviceGetAttribute(&v, (CUdevice_attribute)a.id, dev);
        if (e == CUDA_SUCCESS) printf("[atributo] %3d %-52s %d\n", a.id, a.nombre, v);
        else { const char *s = 0; cuGetErrorName(e, &s); printf("[atributo] %3d %-52s ERROR %s\n", a.id, a.nombre, s); }
    }
}

static void parte_modulos()
{
    HMODULE m[1024]; DWORD n = 0;
    if (!EnumProcessModules(GetCurrentProcess(), m, sizeof m, &n)) { printf("[modulo] ERROR %lu\n", GetLastError()); return; }
    char perfil[MAX_PATH] = {0};
    GetEnvironmentVariableA("USERPROFILE", perfil, MAX_PATH);
    for (DWORD i = 0; i < n / sizeof(HMODULE); i++) {
        char ruta[MAX_PATH]; GetModuleFileNameExA(GetCurrentProcess(), m[i], ruta, MAX_PATH);
        MODULEINFO mi; GetModuleInformation(GetCurrentProcess(), m[i], &mi, sizeof mi);
        std::string r = ruta;
        size_t lp = strlen(perfil);
        // Lo que vive bajo el perfil del usuario (el propio .exe) sale solo con su nombre.
        if (lp && _strnicmp(r.c_str(), perfil, lp) == 0) r = "(usuario)\\" + r.substr(r.find_last_of('\\') + 1);
        printf("[modulo] %10lu B  %s\n", (unsigned long)mi.SizeOfImage, r.c_str());
    }
}

static cudaTextureObject_t crear_tex(cudaArray_t a, cudaTextureFilterMode f, cudaTextureAddressMode d)
{
    cudaResourceDesc r; memset(&r, 0, sizeof r);
    r.resType = cudaResourceTypeArray; r.res.array.array = a;
    cudaTextureDesc t; memset(&t, 0, sizeof t);
    t.addressMode[0] = t.addressMode[1] = d; t.filterMode = f;
    t.readMode = cudaReadModeNormalizedFloat; t.normalizedCoords = 1;
    t.borderColor[1] = 0.5f; t.borderColor[2] = 1.0f; t.borderColor[3] = 1.0f;
    cudaTextureObject_t o = 0;
    R(cudaCreateTextureObject(&o, &r, &t, nullptr));
    return o;
}

static void asa(const char *que, unsigned long long h)
{
    printf("[asa] %-34s 0x%016llx  bajos20=0x%05llx  20..39=0x%05llx  altos=0x%llx\n",
           que, h, h & 0xFFFFF, (h >> 20) & 0xFFFFF, h >> 40);
}

static void parte_asas()
{
    cudaChannelFormatDesc c = cudaCreateChannelDesc<uchar4>();
    cudaArray_t a, b, s;
    R(cudaMallocArray(&a, &c, 4, 4));
    R(cudaMallocArray(&b, &c, 4, 4));
    R(cudaMallocArray(&s, &c, 4, 4, cudaArraySurfaceLoadStore));
    const char *fn[] = {"Point", "Linear"};
    const char *dn[] = {"Wrap", "Mirror", "Clamp", "Border"};
    cudaTextureAddressMode dm[] = {cudaAddressModeWrap, cudaAddressModeMirror, cudaAddressModeClamp, cudaAddressModeBorder};
    std::vector<cudaTextureObject_t> hechas;
    for (int f = 0; f < 2; f++) for (int d = 0; d < 4; d++) {
        char q[64]; snprintf(q, sizeof q, "A %s %s", fn[f], dn[d]);
        cudaTextureObject_t o = crear_tex(a, f ? cudaFilterModeLinear : cudaFilterModePoint, dm[d]);
        asa(q, o); hechas.push_back(o);
    }
    cudaTextureObject_t ob = crear_tex(b, cudaFilterModePoint, cudaAddressModeWrap);
    asa("B Point Wrap (otra imagen)", ob);
    cudaTextureObject_t oa2 = crear_tex(a, cudaFilterModePoint, cudaAddressModeWrap);
    asa("A Point Wrap (repetida)", oa2);
    R(cudaDestroyTextureObject(hechas[0]));
    cudaTextureObject_t oa3 = crear_tex(a, cudaFilterModePoint, cudaAddressModeWrap);
    asa("A Point Wrap (tras destruir la 1a)", oa3);
    cudaResourceDesc r; memset(&r, 0, sizeof r);
    r.resType = cudaResourceTypeArray; r.res.array.array = s;
    cudaSurfaceObject_t so = 0; R(cudaCreateSurfaceObject(&so, &r));
    asa("superficie S", so);
    // cuanto cuesta crear una
    auto t0 = Reloj::now();
    const int N = 200; std::vector<cudaTextureObject_t> v(N);
    for (int i = 0; i < N; i++) v[i] = crear_tex(b, cudaFilterModeLinear, cudaAddressModeClamp);
    double us = us_desde(t0);
    asa("B Linear Clamp (la 1a de 200)", v[0]);
    asa("B Linear Clamp (la 200)", v[N - 1]);
    printf("[tiempo] cudaCreateTextureObject: %.2f us cada una (media de %d)\n", us / N, N);
    for (auto o : v) R(cudaDestroyTextureObject(o));
}

static void parte_especiales()
{
    unsigned int *d; unsigned long long *d64;
    R(cudaMalloc(&d, 64 * 4)); R(cudaMalloc(&d64, 8 * 8));
    R(cudaMemset(d, 0xEE, 64 * 4));
    for (int vuelta = 0; vuelta < 2; vuelta++) {
        especiales<<<dim3(3, 2, 1), dim3(5, 1, 1), 96>>>(d, d64);
        R(cudaGetLastError()); R(cudaDeviceSynchronize());
        unsigned int h[64]; unsigned long long h64[8];
        R(cudaMemcpy(h, d, sizeof h, cudaMemcpyDeviceToHost));
        R(cudaMemcpy(h64, d64, sizeof h64, cudaMemcpyDeviceToHost));
        printf("[especial] lanzamiento %d con <<<(3,2,1),(5,1,1),96 B>>>\n", vuelta + 1);
        const char *n[] = {"ntid.x","ntid.y","ntid.z","nctaid.x","nctaid.y","nctaid.z","nsmid","smid",
                           "nwarpid","warpid","laneid","dynamic_smem_size","total_smem_size"};
        for (int i = 0; i < 13; i++) printf("[especial]   %-18s %u\n", n[i], h[i]);
        for (int i = 0; i < 32; i++) printf("[especial]   envreg%-12d 0x%08x\n", i, h[16 + i]);
        printf("[especial]   gridid             %llu\n", h64[0]);
        printf("[especial]   clock64            %llu\n", h64[1]);
        printf("[especial]   globaltimer        %llu ns\n", h64[2]);
    }
    R(cudaFree(d)); R(cudaFree(d64));
}

static void mediana(const char *que, std::vector<double> v)
{
    std::sort(v.begin(), v.end());
    printf("[tiempo] %-44s min %.2f  mediana %.2f  p99 %.2f  max %.2f us (n=%zu)\n", que,
           v.front(), v[v.size() / 2], v[v.size() * 99 / 100], v.back(), v.size());
}

static void parte_tiempos()
{
    unsigned long long *d; R(cudaMalloc(&d, 256 * 8));
    // globaltimer
    pasos_globaltimer<<<1, 1>>>(d, 256); R(cudaDeviceSynchronize());
    unsigned long long h[256]; R(cudaMemcpy(h, d, sizeof h, cudaMemcpyDeviceToHost));
    std::vector<unsigned long long> p(h, h + 256); std::sort(p.begin(), p.end());
    printf("[globaltimer] paso entre cambios: min %llu  mediana %llu  max %llu ns (256 pasos)\n", p[0], p[128], p[255]);

    auto reloj = [&](const char *que) {
        reloj_sm<<<1, 1>>>(d, 20000000LL); R(cudaDeviceSynchronize());
        unsigned long long r[2]; R(cudaMemcpy(r, d, sizeof r, cudaMemcpyDeviceToHost));
        printf("[reloj_sm] %-24s %llu ciclos en %llu ns -> %.1f MHz\n", que, r[0], r[1], r[0] * 1000.0 / r[1]);
    };
    reloj("en frio");

    // lanzar + esperar, vacio
    std::vector<double> v;
    for (int i = 0; i < 20; i++) { vacio<<<1, 1>>>(); R(cudaDeviceSynchronize()); }
    for (int i = 0; i < 2000; i++) {
        auto t0 = Reloj::now(); vacio<<<1, 1>>>(); R(cudaStreamSynchronize(0)); v.push_back(us_desde(t0));
    }
    mediana("vacio: lanzar + esperar (ida y vuelta)", v);
    // solo encolar
    v.clear();
    for (int i = 0; i < 10000; i++) { auto t0 = Reloj::now(); vacio<<<1, 1>>>(); v.push_back(us_desde(t0)); }
    auto t0 = Reloj::now(); R(cudaDeviceSynchronize()); double resto = us_desde(t0);
    mediana("vacio: solo encolar (cudaLaunchKernel)", v);
    printf("[tiempo] tras 10000 encolados, esperar el resto: %.1f us\n", resto);
    // encolar 10000 y medir el total en la GPU con eventos
    cudaEvent_t e0, e1; R(cudaEventCreate(&e0)); R(cudaEventCreate(&e1));
    R(cudaEventRecord(e0)); for (int i = 0; i < 10000; i++) vacio<<<1, 1>>>(); R(cudaEventRecord(e1));
    R(cudaEventSynchronize(e1)); float ms = 0; R(cudaEventElapsedTime(&ms, e0, e1));
    printf("[tiempo] 10000 vacios seguidos en la GPU: %.3f ms -> %.2f us por lanzamiento\n", ms, ms * 1000 / 10000);

    // calentar y volver a medir el reloj
    float *x; int n = 1 << 20; R(cudaMalloc(&x, n * 4)); R(cudaMemset(x, 0, n * 4));
    auto tc = Reloj::now();
    while (us_desde(tc) < 1500000) { calentar<<<n / 256, 256>>>(x, n); R(cudaDeviceSynchronize()); }
    reloj("tras 1,5 s de carga");

    // PCIe
    const size_t B = 64u << 20;
    void *dev; R(cudaMalloc(&dev, B));
    void *pag = malloc(B); memset(pag, 1, B);
    void *fij; R(cudaMallocHost(&fij, B)); memset(fij, 1, B);
    auto banda = [&](const char *que, void *dst, const void *src, cudaMemcpyKind k) {
        R(cudaMemcpy(dst, src, B, k));
        double mejor = 1e30;
        for (int i = 0; i < 5; i++) { auto t = Reloj::now(); R(cudaMemcpy(dst, src, B, k)); mejor = std::min(mejor, us_desde(t)); }
        printf("[pcie] %-30s 64 MiB en %.0f us -> %.2f GB/s\n", que, mejor, B / (mejor * 1000.0));
    };
    banda("RAM paginable -> VRAM", dev, pag, cudaMemcpyHostToDevice);
    banda("VRAM -> RAM paginable", pag, dev, cudaMemcpyDeviceToHost);
    banda("RAM fijada -> VRAM", dev, fij, cudaMemcpyHostToDevice);
    banda("VRAM -> RAM fijada", fij, dev, cudaMemcpyDeviceToHost);
    void *dev2; R(cudaMalloc(&dev2, B));
    {   // VRAM -> VRAM es ASINCRONA para la CPU: se mide con eventos en la GPU
        cudaEvent_t a, z; R(cudaEventCreate(&a)); R(cudaEventCreate(&z));
        R(cudaMemcpy(dev2, dev, B, cudaMemcpyDeviceToDevice)); R(cudaDeviceSynchronize());
        float mejor = 1e30f;
        for (int i = 0; i < 5; i++) {
            R(cudaEventRecord(a)); R(cudaMemcpy(dev2, dev, B, cudaMemcpyDeviceToDevice)); R(cudaEventRecord(z));
            R(cudaEventSynchronize(z)); float ms; R(cudaEventElapsedTime(&ms, a, z)); mejor = std::min(mejor, ms);
        }
        printf("[vram] VRAM -> VRAM (eventos)        64 MiB en %.0f us -> %.1f GB/s copiados (%.1f GB/s de bus: lee + escribe)\n",
               mejor * 1000, B / (mejor * 1e6), 2.0 * B / (mejor * 1e6));
    }
    R(cudaFree(dev2)); R(cudaFree(dev)); R(cudaFreeHost(fij)); free(pag); R(cudaFree(x)); R(cudaFree(d));

    // reservar
    v.clear();
    for (int i = 0; i < 200; i++) { void *q; auto t = Reloj::now(); R(cudaMalloc(&q, 1 << 20)); v.push_back(us_desde(t)); R(cudaFree(q)); }
    mediana("cudaMalloc de 1 MiB", v);
}

// La misma textura de tex3060.cu, en cudaArray (bloques) y en PITCH (filas
// lineales, como el TIC de T0 de BMO-X): 12 puntos x 8 combinaciones, bit a bit.
extern "C" __global__ void muestrear(cudaTextureObject_t t, const float *u, const float *v, float4 *s, int n)
{
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) s[i] = tex2DLod<float4>(t, u[i], v[i], 0.0f);
}

static void parte_pitch()
{
    static const float us[12] = {0.1f, 0.5f, 0.375f, 0.9f, 1.3f, -0.2f, 0.125f, 0.13f, 0.2f, 0.99f, 1.0f, 2.6f};
    static const float vs[12] = {0.1f, 0.5f, 0.625f, 0.2f, 0.4f, 0.7f, 0.125f, 0.13f, 0.9f, 0.99f, 1.0f, -1.4f};
    uchar4 tx[4][4];
    for (int y = 0; y < 4; y++) for (int x = 0; x < 4; x++)
        tx[y][x] = make_uchar4(x * 60, y * 60, (x + y) * 20, 255);
    cudaChannelFormatDesc c = cudaCreateChannelDesc<uchar4>();
    cudaArray_t a; R(cudaMallocArray(&a, &c, 4, 4));
    R(cudaMemcpy2DToArray(a, 0, 0, tx, 16, 16, 4, cudaMemcpyHostToDevice));
    void *lin; size_t paso = 0; R(cudaMallocPitch(&lin, &paso, 16, 4));
    R(cudaMemcpy2D(lin, paso, tx, 16, 16, 4, cudaMemcpyHostToDevice));
    printf("[pitch] cudaMallocPitch de 16 B x 4 filas -> paso %zu B; direccion %% 512 = %llu\n",
           paso, (unsigned long long)((uintptr_t)lin % 512));
    float *du, *dv; float4 *ds;
    R(cudaMalloc(&du, sizeof us)); R(cudaMalloc(&dv, sizeof vs)); R(cudaMalloc(&ds, 12 * sizeof(float4)));
    R(cudaMemcpy(du, us, sizeof us, cudaMemcpyHostToDevice)); R(cudaMemcpy(dv, vs, sizeof vs, cudaMemcpyHostToDevice));
    cudaTextureAddressMode dm[] = {cudaAddressModeWrap, cudaAddressModeMirror, cudaAddressModeClamp, cudaAddressModeBorder};
    const char *dn[] = {"Wrap", "Mirror", "Clamp", "Border"};
    int iguales = 0, distintas = 0;
    for (int f = 0; f < 2; f++) for (int d = 0; d < 4; d++) {
        cudaTextureDesc t; memset(&t, 0, sizeof t);
        t.addressMode[0] = t.addressMode[1] = dm[d]; t.filterMode = f ? cudaFilterModeLinear : cudaFilterModePoint;
        t.readMode = cudaReadModeNormalizedFloat; t.normalizedCoords = 1;
        t.borderColor[1] = 0.5f; t.borderColor[2] = 1.0f; t.borderColor[3] = 1.0f;
        cudaResourceDesc ra; memset(&ra, 0, sizeof ra); ra.resType = cudaResourceTypeArray; ra.res.array.array = a;
        cudaResourceDesc rp; memset(&rp, 0, sizeof rp); rp.resType = cudaResourceTypePitch2D;
        rp.res.pitch2D.devPtr = lin; rp.res.pitch2D.desc = c; rp.res.pitch2D.width = 4; rp.res.pitch2D.height = 4;
        rp.res.pitch2D.pitchInBytes = paso;
        cudaTextureObject_t oa = 0, op = 0;
        R(cudaCreateTextureObject(&oa, &ra, &t, nullptr));
        cudaError_t ep = cudaCreateTextureObject(&op, &rp, &t, nullptr);
        if (ep != cudaSuccess) {
            printf("[pitch] %s %s: cudaCreateTextureObject(pitch) -> %s\n", f ? "Linear" : "Point", dn[d], cudaGetErrorString(ep));
            (void)cudaGetLastError(); R(cudaDestroyTextureObject(oa)); continue;
        }
        float4 sa[12], sp[12];
        muestrear<<<1, 32>>>(oa, du, dv, ds, 12); R(cudaDeviceSynchronize());
        R(cudaMemcpy(sa, ds, sizeof sa, cudaMemcpyDeviceToHost));
        muestrear<<<1, 32>>>(op, du, dv, ds, 12); R(cudaDeviceSynchronize());
        R(cudaMemcpy(sp, ds, sizeof sp, cudaMemcpyDeviceToHost));
        for (int i = 0; i < 12; i++) {
            if (memcmp(&sa[i], &sp[i], sizeof(float4)) == 0) { iguales++; continue; }
            distintas++;
            printf("[pitch] DISTINTA %s %s (%g,%g): array %.9g %.9g %.9g %.9g  pitch %.9g %.9g %.9g %.9g\n",
                   f ? "Linear" : "Point", dn[d], us[i], vs[i], sa[i].x, sa[i].y, sa[i].z, sa[i].w,
                   sp[i].x, sp[i].y, sp[i].z, sp[i].w);
        }
        R(cudaDestroyTextureObject(oa)); R(cudaDestroyTextureObject(op));
    }
    printf("[pitch] array contra pitch: %d muestras iguales bit a bit, %d distintas\n", iguales, distintas);
    R(cudaFree(du)); R(cudaFree(dv)); R(cudaFree(ds)); R(cudaFree(lin)); R(cudaFreeArray(a));
}

int main()
{
    auto t0 = Reloj::now();
    D(cuInit(0));
    double t_init = us_desde(t0);
    CUdevice dev; D(cuDeviceGet(&dev, 0));
    t0 = Reloj::now();
    R(cudaFree(0));  // crea el contexto primario
    double t_ctx = us_desde(t0);
    printf("[tiempo] cuInit %.0f us; crear el contexto primario %.0f us\n", t_init, t_ctx);
    parte_dispositivo(dev);
    parte_modulos();
    parte_asas();
    parte_especiales();
    parte_pitch();
    parte_tiempos();
    printf("[fin] ok\n");
    return 0;
}
