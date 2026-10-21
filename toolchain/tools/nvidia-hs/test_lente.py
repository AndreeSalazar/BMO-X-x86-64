import unittest
import lente


def captura(base=0x110000):
    lineas = ["[   gpu] HS foto externa previa a STARTCPU; id =1",
              "[   gpu] HS foto: TSC antes =100", "[   gpu] HS foto: TSC despues =200",
              "[   gpu] HS foto: mascara PRI =0"]
    for i, (o, _) in enumerate(lente.REGS):
        valor = 3 if i == 15 else 0
        lineas.append(f"[   gpu] HS foto legible: id[31:24] dir[23:0] raw[63:32] ={base+o | 1 << 24 | valor << 32}")
    lineas.append("[   gpu] HS limite: foto externa; autenticacion interna y PC/TCM HS no~ =1")
    return lineas


class LenteTests(unittest.TestCase):
    def test_foto_de_argumentos_puestos_no_se_confunde_con_la_foto_antigua(self):
        lineas = captura(0x840000)
        lineas[0] = "[   gpu] HS foto externa argumentos puestos antes STARTCPU; id =1"
        f = lente.analizar("\n".join(lineas))["fotos"][0]
        self.assertTrue(f["completa"])
        self.assertIn("argumentos puestos", f["fase"])

    def test_cero_legible_y_full_idle_distintos_en_ambos_motores(self):
        for base, motor in lente.BASES.items():
            f = lente.analizar("\n".join(captura(base)))["fotos"][0]
            self.assertTrue(f["completa"])
            self.assertEqual(f["motor"], motor)
            self.assertEqual(f["registros"][0]["crudo"], "0x00000000")
            self.assertTrue(f["registros"][15]["full"])
            self.assertTrue(f["registros"][15]["idle"])

    def test_pri_no_se_traduce_a_actividad(self):
        lineas = captura()
        lineas[3] = "[   gpu] HS foto: mascara PRI =16"
        lineas[8] = f"[   gpu] HS foto PRI: id[31:24] dir[23:0] raw[63:32] ={0x111388 | 1 << 24 | 0xBADF1002 << 32}"
        f = lente.analizar("\n".join(lineas))["fotos"][0]
        self.assertTrue(f["completa"])
        self.assertEqual(f["registros"][4]["lectura"], "PRI")
        self.assertNotIn("activo", f["registros"][4])

    def test_lista_incompleta_y_sin_cierre_no_se_declaran_completas(self):
        f = lente.analizar("\n".join(captura()[:6]))["fotos"][0]
        self.assertFalse(f["completa"])
        self.assertIn("falta_cierre", f["errores"])
        self.assertIn("lista_incompleta", f["errores"])

    def test_cambio_de_id_o_direccion_no_se_empareja(self):
        for cambio in ("id", "direccion", "pri", "id_registro", "ancho"):
            lineas = captura()
            if cambio == "id": lineas[-1] = lineas[-1][:-1]+"2"
            elif cambio == "direccion": lineas[4] = "[   gpu] HS foto legible: direccion | crudo << 32 =123"
            elif cambio == "pri": lineas[4] = f"[   gpu] HS foto legible: id/dir/raw ={0x110040 | 1 << 24 | 0xBADF1002 << 32}"
            elif cambio == "id_registro": lineas[4] = f"[   gpu] HS foto legible: id/dir/raw ={0x110040 | 2 << 24}"
            else: lineas[4] = f"[   gpu] HS foto legible: id/dir/raw ={0x110040 | 1 << 24 | 1 << 65}"
            self.assertFalse(lente.analizar("\n".join(lineas))["fotos"][0]["completa"])

    def test_no_fabricar_fotos_de_informes_anteriores(self):
        self.assertEqual(lente.analizar("[   gpu] otro mensaje =0")["fotos"], [])

    def test_tiempos_mascara_y_foto_interrumpida(self):
        for campo, valor in [(1, 300), (3, 1)]:
            lineas = captura()
            lineas[campo] = lineas[campo].rsplit("=", 1)[0]+f"={valor}"
            self.assertFalse(lente.analizar("\n".join(lineas))["fotos"][0]["completa"])
        resultado = lente.analizar("\n".join(captura()[:5]+captura()))
        self.assertIn("foto_interrumpida", resultado["fotos"][0]["errores"])
        self.assertEqual(resultado["fotos_completas"], 1)

    def test_truncado_de_mensaje_a_60_bytes_conserva_el_formato(self):
        lineas = captura()
        final = lineas[-1].split(" =", 1)
        lineas[-1] = final[0][:60]+"~ ="+final[1]
        self.assertTrue(lente.analizar("\n".join(lineas))["fotos"][0]["completa"])

    def test_cabeceras_duplicadas_no_ocultan_un_marco_perdido(self):
        lineas = captura()
        lineas.insert(2, lineas[1])
        f = lente.analizar("\n".join(lineas))["fotos"][0]
        self.assertFalse(f["completa"])
        self.assertIn("campo_duplicado", f["errores"])


if __name__ == "__main__":
    unittest.main()
