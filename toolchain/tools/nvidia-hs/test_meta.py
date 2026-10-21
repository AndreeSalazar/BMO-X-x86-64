import unittest
import meta


def foto(copia=True, estado=0, permisos=15, inicio=0x12000, fin=0x120FF, cambio=None):
    def linea(mensaje, v):
        return f"[   gpu] WPR foto: {mensaje} ={v}"
    lineas = [linea("id local", 1)]
    for k, v in [("fase", 1), ("TSC antes", 100), ("TSC despues", 200),
                 ("IOVA", 0x3E007000), ("fisica propia esperada", 0x12000),
                 ("fisica inicio", inicio), ("fisica fin", fin), ("permisos", permisos),
                 ("estado host", estado), ("copia observada disponible", int(copia)),
                 ("mascara diferencias", 0 if cambio is None else 1 << cambio)]:
        lineas.append(linea(k, v))
    for i in range(32):
        lineas += [linea("palabra indice", i), linea("palabra esperada", (1 << 63) | i)]
        if copia:
            lineas.append(linea("palabra observada", ((1 << 63) | i) ^ (1 if i == cambio else 0)))
    lineas.append(linea("cierre id local", 1))
    return lineas


def leer(lineas):
    return meta.analizar("\n".join(lineas))["fotos"][0]


class MetaTests(unittest.TestCase):
    def test_copia_completa_nombra_campos_y_conserva_bits_altos(self):
        f = leer(foto())
        self.assertTrue(f["completa"], f["errores"])
        self.assertEqual(f["estado"], "coincide")
        self.assertEqual(f["palabras"][25]["campo"], "bootCount")
        self.assertEqual(f["palabras"][31]["campo"], "verified")
        self.assertEqual(f["palabras"][31]["observada"], "0x800000000000001f")

    def test_todos_los_campos_cambiados_se_validan_sin_declarar_igualdad(self):
        for i in range(32):
            f = leer(foto(estado=5, cambio=i))
            self.assertTrue(f["completa"], f["errores"])
            self.assertEqual(f["estado"], "contenido_distinto")
            self.assertEqual(f["mascara_diferencias"], 1 << i)
        self.assertFalse(leer(foto(estado=0, cambio=31))["completa"])

    def test_sin_traduccion_no_es_copia_de_ceros_ni_pagina_legible(self):
        f = leer(foto(copia=False, estado=1, permisos=0, inicio=meta.MAX, fin=meta.MAX))
        self.assertTrue(f["completa"], f["errores"])
        self.assertEqual(f["estado"], "sin_traduccion")
        self.assertNotIn("observada", f["palabras"][0])
        self.assertFalse(leer(foto(estado=1, permisos=0, inicio=meta.MAX, fin=meta.MAX))["completa"])

    def test_pagina_distinta_y_sin_permiso_son_fotos_coherentes_sin_copia(self):
        for opciones in [dict(estado=2, inicio=0x13000, fin=0x130FF), dict(estado=3, permisos=5)]:
            self.assertTrue(leer(foto(copia=False, **opciones))["completa"])

    def test_recortes_duplicados_ids_y_tiempos_no_pasan_por_completos(self):
        for indice, texto in [(0, "[   gpu] WPR foto: id local =3"),
                              (3, "[   gpu] WPR foto: TSC despues =1"),
                              (-1, "[   gpu] WPR foto: cierre id local =2"),
                              (4, "[   gpu] WPR foto: IOVA =1"),
                              (4, f"[   gpu] WPR foto: IOVA ={1 << 64}")]:
            lineas = foto()
            lineas[indice] = texto
            self.assertFalse(leer(lineas)["completa"])
        lineas = foto()
        lineas.insert(2, lineas[1])
        self.assertFalse(leer(lineas)["completa"])
        self.assertFalse(leer(foto()[:-2])["completa"])
        f = meta.analizar("\n".join(foto()[:20] + foto()))
        self.assertFalse(f["fotos"][0]["completa"])
        self.assertEqual(f["fotos_completas"], 1)

    def test_informes_anteriores_no_inventan_fotos(self):
        self.assertEqual(meta.analizar("[   gpu] HS foto: TSC antes =100")["fotos"], [])


if __name__ == "__main__":
    unittest.main()
