// 14-10 (el CS de Cyberpunk que no se sabia correr): el % de floats (frem)
// y los BINOP que en LLVM son solo de enteros. cs_6_0, de `dxc`.
ByteAddressBuffer ent : register(t0);
RWByteAddressBuffer sal : register(u0);
[numthreads(32, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    float a = asfloat(ent.Load(id.x * 8));
    float b = asfloat(ent.Load(id.x * 8 + 4));
    uint u = asuint(a);
    sal.Store(id.x * 8, asuint(a % b));
    sal.Store(id.x * 8 + 4, ((u >> 3) ^ (u << 5) | (u & 0xF0)) + u % 7u + u / 3u);
}
