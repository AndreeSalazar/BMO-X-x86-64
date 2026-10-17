# Nvidia -- GSP-CONTROL-BMO

El gobierno del GSP desde BMO-X: el GSP ejecuta, BMO-X gobierna. El
[crate puro](gsp-control-bmo/README.md) implementa el contrato, seis estados
del arranque y el diagnostico de observaciones. La consulta nativa
`gpu control` y el informe `save` consumen su diagnostico.

El [plan completo](../../../../docs/plan/PLAN_GSP_CONTROL_BMO.md) ordena las
cinco palancas (dar posesion, conceder, pedir, escuchar, recuperar) y deja
APARCADOS, con motivo, el estudio del RISC-V del GSP y el firmware propio.
El supervisor ejecuta en la CPU x86-64 de BMO-X.

El [driver GA10x](../ga10x/ANATOMIA.md), sus emisores, jueces y lectores
conservan sus propietarios. Esta carpeta aporta gobierno, sin duplicar
el controlador ni ejecutar firmware BMO dentro del GSP.

## Investigacion de firmware propio

Pedido del propietario, 17-10: [BROM, HS y reemplazo](HS_INVESTIGACION.md).
El objetivo de investigacion se amplia al control propio, con evidencia
por pieza y pasos verificables. La primera entrega inventaria copias en
el anfitrion; no incorpora firmware propio ni cambia el supervisor puro.
