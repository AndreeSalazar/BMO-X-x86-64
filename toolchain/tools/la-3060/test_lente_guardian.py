"""Pruebas negativas S6: demostrar que el guardian rechaza fugas."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('la_3060', Path(__file__).with_name('la_3060.py'))
guardian = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guardian)


class LensGuard(unittest.TestCase):
    def check_fixture(self, source='', manifest='', build_script=False):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / 'src' / 'nested').mkdir(parents=True)
            (root / 'Cargo.toml').write_text('[package]\nname="fixture"\nversion="0.1.0"\n' + manifest, encoding='utf-8')
            (root / 'src' / 'lib.rs').write_text('#![no_std]\n#![forbid(unsafe_code)]\n', encoding='utf-8')
            (root / 'src' / 'nested' / 'reader.rs').write_text(source, encoding='utf-8')
            if build_script:
                (root / 'build.rs').write_text('fn main() {}', encoding='utf-8')
            failures = []
            self.assertEqual(guardian.lente_comun(failures, str(root)), 2)
            return failures

    def test_neutral_core_is_accepted(self):
        self.assertEqual(self.check_fixture('pub fn count(b: &[u8]) -> usize { b.len() }'), [])

    def test_vendor_names_are_rejected_even_in_nested_sources(self):
        for vendor in ('nvidia', 'AMD', 'rdna3', 'bmo_gpu_ga10x', 'bmo_sm86', 'cuda'):
            with self.subTest(vendor=vendor):
                self.assertTrue(self.check_fixture('use ' + vendor + '::decode;'))

    def test_aliased_optional_and_target_dependencies_cannot_hide_driver(self):
        for manifest in ('[dependencies]\ninnocent={package="driver",version="1",optional=true}\n',
                         '[target."cfg(windows)".dependencies]\ninnocent="1"\n',
                         '[build-dependencies]\ninnocent="1"\n'):
            with self.subTest(manifest=manifest):
                self.assertTrue(self.check_fixture(manifest=manifest))

    def test_unsafe_access_is_rejected(self):
        self.assertTrue(self.check_fixture('unsafe { core::ptr::read_volatile(p) };'))

    def test_redirected_library_and_build_scripts_are_rejected(self):
        self.assertTrue(self.check_fixture(manifest='[lib]\npath="hidden.rs"\n'))
        self.assertTrue(self.check_fixture(build_script=True))

    def check_query(self, source):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / 'sm86_bytes.rs').write_text(source, encoding='utf-8')
            previous = guardian.GSPCUBO
            try:
                guardian.GSPCUBO = folder
                failures = []
                guardian.lente_lectura(failures)
                return failures
            finally:
                guardian.GSPCUBO = previous

    def test_query_keeps_prepared_provenance(self):
        self.assertEqual(self.check_query('let s = (Origin::Prepared, Space::Resource);'), [])
        self.assertTrue(self.check_query('let s = (Origin::Readback, Space::GpuVirtual);'))

    def test_query_cannot_gain_device_operations(self):
        for operation in ('iommu_orden(1)', 'preparar_hasta(dsk)', 'unsafe { read_volatile(p) }'):
            with self.subTest(operation=operation):
                self.assertTrue(self.check_query('let s = (Origin::Prepared, Space::Resource); ' + operation))

    def test_missing_sources_are_not_success(self):
        with tempfile.TemporaryDirectory() as folder:
            failures = []
            self.assertEqual(guardian.lente_comun(failures, folder), 0)
            self.assertTrue(failures)


class NvidiaInspectionGuard(unittest.TestCase):
    def fixture(self, extra='', header='VERDE'):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name in ('mod.rs', 'empuje.rs', 'metodos.rs'):
                (root / name).write_text('//! [carril] ' + header + '\n//! [consumo] NADA\n', encoding='utf-8')
            (root / 'nested').mkdir()
            (root / 'nested' / 'extra.rs').write_text('//! [carril] VERDE\n//! [consumo] NADA\n' + extra, encoding='utf-8')
            failures = []
            self.assertEqual(guardian.inspeccion_nvidia(failures, folder), 4)
            return failures

    def test_reader_can_only_interpret_copies(self):
        self.assertEqual(self.fixture('pub fn count(w: &[u32]) -> usize { w.len() }'), [])

    def test_hardware_authority_is_rejected_in_nested_reader(self):
        for leak in ('use crate::Registros;', 'use crate::arranque::falcon;', 'unsafe { core::ptr::read_volatile(p) }', 'iommu_orden(1);'):
            with self.subTest(leak=leak):
                self.assertTrue(self.fixture(leak))

    def test_changing_the_lane_or_losing_the_reader_is_rejected(self):
        self.assertTrue(self.fixture(header='ROJO'))
        with tempfile.TemporaryDirectory() as folder:
            failures = []
            self.assertEqual(guardian.inspeccion_nvidia(failures, folder), 0)
            self.assertTrue(failures)



class ControlGuard(unittest.TestCase):
    def fixture(self, source='', manifest='', query='', build=False):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root/'src').mkdir()
            (root/'Cargo.toml').write_text('[package]\nname="fixture"\nversion="0.1.0"\n'+manifest, encoding='ascii')
            (root/'src/lib.rs').write_text('//! [carril] VERDE\n//! [consumo] NADA\n#![no_std]\n#![forbid(unsafe_code)]\n'+source, encoding='ascii')
            q=root/'query.rs'; q.write_text(query, encoding='ascii')
            if build: (root/'build.rs').write_text('fn main() {}', encoding='ascii')
            failures=[]
            self.assertEqual(guardian.gsp_control(failures,str(root),str(q)),1)
            return failures

    def test_pure_and_readonly_query(self):
        self.assertEqual(self.fixture(query='let x=bmo::info(1);'),[])

    def test_actions_and_dependencies_are_rejected(self):
        for source in ('unsafe { write_volatile(p,v) }', 'use std::fs;', 'extern crate hidden;'):
            self.assertTrue(self.fixture(source=source))
        for query in ('bmo::iommu_orden_con(1,0);', 'bmo::ciclos();', 'unsafe { asm!("nop") }'):
            self.assertTrue(self.fixture(query=query))
        self.assertTrue(self.fixture(manifest='[dependencies]\nhidden="1"\n'))
        self.assertTrue(self.fixture(manifest='[target."cfg(windows)".dependencies]\nhidden="1"\n'))
        self.assertTrue(self.fixture(manifest='[lib]\npath="hidden.rs"\n'))
        self.assertTrue(self.fixture(build=True))

    def test_variants_pointers_and_external_includes_cannot_hide_access(self):
        for source in (
            '#[cfg(feature="hidden")] fn f() { unsafe { asm!("nop") } }',
            '#[cfg(target_os="none")] fn f(p: *mut u8) {}',
            'fn f(p: *const u8) {}',
            'include!("outside.rs");',
            'include_bytes!("outside.bin");',
            '#[path="../../outside.rs"] mod hidden;',
            '#[allow(unsafe_code)] mod hidden {}',
        ):
            with self.subTest(source=source):
                self.assertTrue(self.fixture(source=source))
                self.assertTrue(self.fixture(query=source))

class LibosGuard(unittest.TestCase):
    def fixture(self, extra='', manifest='', build=False):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root/'src/nested').mkdir(parents=True)
            (root/'Cargo.toml').write_text('[package]\nname="fixture"\nversion="0.1.0"\n'+manifest, encoding='ascii')
            header = '//! [carril] VERDE\n//! [consumo] NADA\n'
            for name in ('lib.rs', 'anillo.rs', 'diccionario.rs', 'fuente.rs'):
                text = header + ('#![no_std]\n#![forbid(unsafe_code)]\n' if name == 'lib.rs' else '')
                (root/'src'/name).write_text(text, encoding='ascii')
            (root/'src/nested/extra.rs').write_text(header+extra, encoding='ascii')
            if build:
                (root/'build.rs').write_text('fn main() {}', encoding='ascii')
            failures = []
            self.assertEqual(guardian.libos_copias(failures, str(root)), 5)
            return failures

    def test_pure_reader_is_accepted(self):
        self.assertEqual(self.fixture('pub fn count(b: &[u8]) -> usize { b.len() }'), [])

    def test_device_access_in_inactive_variant_is_rejected(self):
        for leak in ('use crate::Registros;', 'use std::fs;', 'fn f(p: *const u8) {}',
                     'include!("outside.rs");', '#[path="../../outside.rs"] mod hidden;',
                     '#[allow(unsafe_code)] mod hidden {}', 'unsafe { asm!("nop") }'):
            with self.subTest(leak=leak):
                self.assertTrue(self.fixture('#[cfg(feature="hidden")]\n' + leak))

    def test_dependencies_and_build_escape_are_rejected(self):
        for manifest in ('[dependencies]\nhidden="1"\n',
                         '[target."cfg(windows)".dependencies]\nalias={package="hidden",version="1"}\n',
                         '[dev-dependencies]\nhidden="1"\n',
                         '[lib]\npath="outside.rs"\n'):
            with self.subTest(manifest=manifest):
                self.assertTrue(self.fixture(manifest=manifest))
        self.assertTrue(self.fixture(build=True))

    def test_missing_reader_cannot_pass(self):
        with tempfile.TemporaryDirectory() as folder:
            failures = []
            self.assertEqual(guardian.libos_copias(failures, folder), 0)
            self.assertTrue(failures)


if __name__ == '__main__':
    unittest.main()
