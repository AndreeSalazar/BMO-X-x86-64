# El TEX de sm_86, sacado de un binario (P3b4c.8 T1, 29-09)

`tex_bindless.ptx`: un `tex.level.2d` con el asa en un REGISTRO (cargada con
un LDG). Compilado y leido asi, en Linux, con las herramientas de NVIDIA de
PyPI (las mismas de S4, `PLAN_LA_3060.md`):

```text
pip install --target cuda "nvidia-cuda-nvcc-cu12==12.9.*"
pip download --no-deps nvidia-cuda-nvdisasm nvidia-cuda-cuobjdump   (13.4.92)
cuda/nvidia/cuda_nvcc/bin/ptxas -arch=sm_86 tex_bindless.ptx -o t.cubin
nvidia/cu13/bin/cuobjdump -sass t.cubin
```

Lo que dijo:

```text
TEX.SCR.B.LZ R6, R4, R4, R0, 2D ;   /* 0x3800000004047361 */
                                    /* 0x004f4400009e0f06 */
```

Los campos se sacaron moviendo UN bit o UN registro cada vez y volviendo a
desensamblar con `nvdisasm -b SM86 -raw` (la tabla, en
`src/trabajos/texturas.rs`). El codificador `texturas::tex` da esos bits y
cinco combinaciones mas que `nvdisasm` leyo como se pidieron (sus pruebas).
[!] Desensamblar bien NO es correr bien: eso lo dice el metal (T3).
