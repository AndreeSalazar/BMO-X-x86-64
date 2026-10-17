# Nvidia -- GSP-CONTROL-BMO y su frontera

Carpeta reservada por decision del propietario el 16-10-2026 para el nuevo
supervisor NVIDIA. El nombre `Nvidia` se conserva tal como lo pidio.

El [plan completo](../../../../docs/plan/PLAN_GSP_CONTROL_BMO.md) define
potencial, etapas, evidencia, guardianes y la investigacion condicional
de un mini-kernel dentro del GSP.

Crate previsto: `bmo-nvidia-gsp-control`, en `gsp-control-bmo/`. Todavia no
se crea Cargo.toml ni se registra una implementacion vacia en el workspace.
La primera fase inventaria los propietarios actuales y define contratos.

El supervisor puro ejecutara en la CPU x86-64 de BMO-X. Las operaciones de
aparato seguiran pasando por el adaptador autorizado del kernel. El estudio
del RISC-V interno interpreta copias de firmware y no porta el sistema a
otra CPU. La ejecucion de firmware BMO dentro del GSP aun no esta demostrada.

El [driver GA10x existente](../ga10x/ANATOMIA.md), su emisor, jueces y
lectores siguen en sus rutas actuales. Esta carpeta no los duplica ni
atribuye al futuro supervisor las funciones que ya realizan.
