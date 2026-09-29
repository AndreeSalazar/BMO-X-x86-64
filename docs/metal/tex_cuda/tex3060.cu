// tex3060.cu -- el muestreo de la 3060 medido con CUDA.
//
// Una textura 2D de 4x4 texeles RGBA8, texel (x,y) = R=x*60, G=y*60,
// B=(x+y)*20, A=255, leida con cudaReadModeNormalizedFloat y coordenadas
// normalizadas. Para cada filtro {Point, Linear} y direccion {Wrap, Mirror,
// Clamp, Border (borde = 0,0.5,1,1)} se muestrean 12 puntos con
// tex2DLod<float4>(t, u, v, 0.0f) y se imprime una linea por muestra:
//
//   filtro direccion u v R G B A  bits(R) bits(G) bits(B) bits(A)
//
// Compilar:  nvcc -arch=sm_86 -O2 -o tex3060.exe tex3060.cu
// Los errores se escriben tal cual en stdout (y se sale con 1).

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cuda_runtime.h>

#define N_MUESTRAS 12

#define COMPROBAR(llamada)                                                   \
    do {                                                                     \
        cudaError_t e_ = (llamada);                                          \
        if (e_ != cudaSuccess) {                                             \
            printf("ERROR %s:%d %s -> %d %s: %s\n", __FILE__, __LINE__,      \
                   #llamada, (int)e_, cudaGetErrorName(e_),                  \
                   cudaGetErrorString(e_));                                  \
            fflush(stdout);                                                  \
            exit(1);                                                         \
        }                                                                    \
    } while (0)

extern "C" __global__ void muestrear(cudaTextureObject_t t, const float *u,
                                     const float *v, float4 *salida, int n)
{
    int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n)
        salida[i] = tex2DLod<float4>(t, u[i], v[i], 0.0f);
}

static unsigned int bits(float f)
{
    unsigned int b;
    memcpy(&b, &f, sizeof b);
    return b;
}

int main()
{
    static const float us[N_MUESTRAS] = {0.1f, 0.5f,  0.375f, 0.9f,
                                         1.3f, -0.2f, 0.125f, 0.13f,
                                         0.2f, 0.99f, 1.0f,   2.6f};
    static const float vs[N_MUESTRAS] = {0.1f, 0.5f,  0.625f, 0.2f,
                                         0.4f, 0.7f,  0.125f, 0.13f,
                                         0.9f, 0.99f, 1.0f,   -1.4f};

    struct Filtro { const char *nombre; cudaTextureFilterMode modo; };
    struct Direccion { const char *nombre; cudaTextureAddressMode modo; };
    static const Filtro filtros[] = {
        {"Point", cudaFilterModePoint},
        {"Linear", cudaFilterModeLinear},
    };
    static const Direccion direcciones[] = {
        {"Wrap", cudaAddressModeWrap},
        {"Mirror", cudaAddressModeMirror},
        {"Clamp", cudaAddressModeClamp},
        {"Border", cudaAddressModeBorder},
    };

    int dispositivo = 0;
    cudaDeviceProp prop;
    int driver = 0, runtime = 0;
    COMPROBAR(cudaGetDevice(&dispositivo));
    COMPROBAR(cudaGetDeviceProperties(&prop, dispositivo));
    COMPROBAR(cudaDriverGetVersion(&driver));
    COMPROBAR(cudaRuntimeGetVersion(&runtime));
    printf("# dispositivo %d: %s, sm_%d%d, driver CUDA %d, runtime %d\n",
           dispositivo, prop.name, prop.major, prop.minor, driver, runtime);

    // Los 16 texeles, fila a fila (y), cada fila de x = 0..3.
    uchar4 texeles[4][4];
    for (int y = 0; y < 4; y++)
        for (int x = 0; x < 4; x++)
            texeles[y][x] = make_uchar4((unsigned char)(x * 60),
                                        (unsigned char)(y * 60),
                                        (unsigned char)((x + y) * 20), 255);
    printf("# texeles (x,y) = R G B A:\n");
    for (int y = 0; y < 4; y++) {
        printf("#");
        for (int x = 0; x < 4; x++)
            printf("  (%d,%d)=%3d %3d %3d %3d", x, y, texeles[y][x].x,
                   texeles[y][x].y, texeles[y][x].z, texeles[y][x].w);
        printf("\n");
    }
    printf("# filtro direccion u v R G B A bits(R) bits(G) bits(B) bits(A)\n");

    cudaChannelFormatDesc canal = cudaCreateChannelDesc<uchar4>();
    cudaArray_t arreglo = nullptr;
    COMPROBAR(cudaMallocArray(&arreglo, &canal, 4, 4));
    COMPROBAR(cudaMemcpy2DToArray(arreglo, 0, 0, texeles, 4 * sizeof(uchar4),
                                  4 * sizeof(uchar4), 4,
                                  cudaMemcpyHostToDevice));

    float *d_u = nullptr, *d_v = nullptr;
    float4 *d_salida = nullptr;
    COMPROBAR(cudaMalloc(&d_u, sizeof us));
    COMPROBAR(cudaMalloc(&d_v, sizeof vs));
    COMPROBAR(cudaMalloc(&d_salida, N_MUESTRAS * sizeof(float4)));
    COMPROBAR(cudaMemcpy(d_u, us, sizeof us, cudaMemcpyHostToDevice));
    COMPROBAR(cudaMemcpy(d_v, vs, sizeof vs, cudaMemcpyHostToDevice));

    cudaResourceDesc recurso;
    memset(&recurso, 0, sizeof recurso);
    recurso.resType = cudaResourceTypeArray;
    recurso.res.array.array = arreglo;

    for (const Filtro &f : filtros) {
        for (const Direccion &d : direcciones) {
            cudaTextureDesc desc;
            memset(&desc, 0, sizeof desc);
            desc.addressMode[0] = d.modo;
            desc.addressMode[1] = d.modo;
            desc.filterMode = f.modo;
            desc.readMode = cudaReadModeNormalizedFloat;
            desc.normalizedCoords = 1;
            desc.borderColor[0] = 0.0f;
            desc.borderColor[1] = 0.5f;
            desc.borderColor[2] = 1.0f;
            desc.borderColor[3] = 1.0f;

            cudaTextureObject_t t = 0;
            COMPROBAR(cudaCreateTextureObject(&t, &recurso, &desc, nullptr));

            muestrear<<<1, 32>>>(t, d_u, d_v, d_salida, N_MUESTRAS);
            COMPROBAR(cudaGetLastError());
            COMPROBAR(cudaDeviceSynchronize());

            float4 salida[N_MUESTRAS];
            COMPROBAR(cudaMemcpy(salida, d_salida, sizeof salida,
                                 cudaMemcpyDeviceToHost));
            for (int i = 0; i < N_MUESTRAS; i++) {
                const float4 &s = salida[i];
                printf("%s %s %g %g %.9g %.9g %.9g %.9g "
                       "0x%08x 0x%08x 0x%08x 0x%08x\n",
                       f.nombre, d.nombre, us[i], vs[i], s.x, s.y, s.z, s.w,
                       bits(s.x), bits(s.y), bits(s.z), bits(s.w));
            }

            COMPROBAR(cudaDestroyTextureObject(t));
        }
    }

    COMPROBAR(cudaFree(d_salida));
    COMPROBAR(cudaFree(d_v));
    COMPROBAR(cudaFree(d_u));
    COMPROBAR(cudaFreeArray(arreglo));
    return 0;
}
