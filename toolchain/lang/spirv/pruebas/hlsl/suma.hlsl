// suma, en HLSL (DirectX): la MISMA que suma.comp. dxc -spirv la lleva al
// mismo SPIR-V que glslc; register(tN/uN) -> Binding N, DescriptorSet 0.
StructuredBuffer<float> A : register(t0);
StructuredBuffer<float> B : register(t1);
RWStructuredBuffer<float> C : register(u2);
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    C[id.x] = A[id.x] + B[id.x];
}
