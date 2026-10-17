# Nvidia -- GSP-CONTROL-BMO

Supervisor del GSP por etapas. El [crate puro](gsp-control-bmo/README.md)
ya implementa el contrato, seis estados y diagnostico de observaciones.
La consulta nativa `gpu control` y el informe `save` consumen su diagnostico.

El [plan completo](../../../../docs/plan/PLAN_GSP_CONTROL_BMO.md) conserva
las etapas de protocolo, capturas, anatomia del firmware y rendimiento.
El supervisor ejecuta en la CPU x86-64 de BMO-X; el estudio del coprocesador
no cambia la arquitectura del sistema.

El [driver GA10x](../ga10x/ANATOMIA.md), sus emisores, jueces y lectores
conservan sus propietarios. Esta carpeta aporta supervision, sin duplicar
el controlador ni ejecutar firmware BMO dentro del GSP.
