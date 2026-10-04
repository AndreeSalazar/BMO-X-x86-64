# PLAN BMO ATENTO -- el sistema que se fija en lo que haces, y se adelanta

> Abierto el **2026-10-04** a peticion del propietario: *"me gustaria saber si
> mi BMO-X se vuelva inteligente y ATENTO a lo que el usuario hace"*, con la
> condicion de siempre: que la CPU no lo pague.
>
> Sin codigo todavia: esto es el papel. Lo de MAQUETA sigue en
> [`PLAN_MAQUETA.md`](PLAN_MAQUETA.md); el cerebro es de TITAN++, que lleva otro
> equipo.

---

# 0. LA IDEA EN UNA LINEA

**Atento no es adivinar: es CONTAR.** Que abres, a que hora, en que orden, que
ventana pones donde, que orden tecleas despues de cual. Contar un evento cuesta
nanosegundos; con esas cuentas el sistema se adelanta sin una red neuronal.

```text
   lo que se ve                       de que cuenta sale
   la rejilla pone primero            lo que mas abres A ESTA HORA
   Alt+Tab propone                    la ventana a la que sueles VOLVER
   Ejecutar sugiere                   la orden que sueles teclear DESPUES de esta
   una ventana se abre                donde la dejaste la ultima vez
```

# 1. EL REPARTO (el de siempre)

- **TITAN++ y el director OBSERVAN y DECIDEN**: llevan las cuentas y eligen.
- **MAQUETA solo MUESTRA**: el estado que le dicen (el orden de la rejilla, la
  fila marcada). No sabe que hay un "atento".
- La semilla ya existe: la linea de sugerencias de Ejecutar
  (`Ultra_userspace/services/director/src/scene/sugerir.rs`).

# 2. LAS REGLAS (no se negocian)

1. **Todo LOCAL.** Las cuentas viven en el disco de la maquina; nada sale por
   la red. Es la misma regla que no deja una IP ni una MAC en el repo.
2. **Visible y borrable.** Una orden muestra lo que se ha contado, en claro, y
   otra lo borra entero. Lo que el sistema sabe de ti lo puedes leer tu.
3. **Contar, no grabar.** Se guardan cuentas (cuantas veces, a que hora), no
   lo que escribiste dentro de una app.
4. **Que no se note en la CPU.** Contar va en el camino del evento que ya se
   procesa; decidir, solo cuando hace falta mostrar algo (al abrir la
   rejilla, al pulsar Alt+Tab). Nada de bucles de fondo: `[consumo] NADA`.
5. **Se puede apagar.** Una linea en `sys/director.cfg`.

# 3. LA ESCALERA

- [ ] **A1 -- LAS CUENTAS.** Abrir una app, enfocar una ventana, una orden de
  Ejecutar: un contador por cosa y por franja de hora, con olvido lento (lo de
  hace un mes pesa menos). Probado en el anfitrion, en un crate puro de
  `platform/shared/`.
- [ ] **A2 -- VERLAS Y BORRARLAS.** `atento` en Ejecutar muestra las cuentas;
  `atento borrar` las borra. En `Ultra_userspace/services/director/src/commands/`.
- [ ] **A3 -- LA REJILLA SE ORDENA SOLA**, por lo que mas abres a esta hora
  (`Ultra_userspace/services/director/src/scene/launcher.rs`).
- [ ] **A4 -- ALT+TAB PROPONE** la ventana a la que sueles volver
  (`platform/shared/bmo-foco`).
- [ ] **A5 -- EJECUTAR SUGIERE LA SIGUIENTE ORDEN**
  (`Ultra_userspace/services/director/src/scene/sugerir.rs`).

[!] Nada de esto se ha escrito ni probado en el Ryzen.
