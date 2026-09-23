// saxpy, en HLSL: y[i] = alfa * x[i] + y[i], con alfa y n en un cbuffer.
cbuffer Parametros : register(b0) { float alfa; uint n; };
StructuredBuffer<float> X : register(t1);
RWStructuredBuffer<float> Y : register(u2);
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID) {
    uint i = id.x;
    if (i < n) {
        Y[i] = alfa * X[i] + Y[i];
    }
}
