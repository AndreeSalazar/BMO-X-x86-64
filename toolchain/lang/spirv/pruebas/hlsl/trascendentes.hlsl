// trascendentes, en HLSL: sin, cos, exp, log y pow sobre la entrada tal cual.
StructuredBuffer<float> V : register(t0);
StructuredBuffer<float> W : register(t1);
RWStructuredBuffer<float> R : register(u2);
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    float x = V[i];
    R[5 * i + 0] = sin(x);
    R[5 * i + 1] = cos(x);
    R[5 * i + 2] = exp(x);
    R[5 * i + 3] = log(x);
    R[5 * i + 4] = pow(x, W[i]);
}
