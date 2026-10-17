"""El reparto no debe omitir clases, cambiar test=false ni esconder errores."""
import importlib.util
import io
import json
import subprocess
import sys
import threading
import time
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('banco', Path(__file__).with_name('banco.py'))
banco = importlib.util.module_from_spec(spec)
spec.loader.exec_module(banco)


class Banco(unittest.TestCase):
    def plan(self, targets, manifest=''):
        with TemporaryDirectory() as folder:
            path = Path(folder)/'Cargo.toml'
            path.write_text('[package]\nname="fixture"\nversion="0.1.0"\n'+manifest, encoding='ascii')
            return banco.plan({'packages':[{'name':'fixture','manifest_path':str(path),'targets':targets}]})

    def target(self, kind, test=True, doctest=False):
        return dict(kind=[kind],test=test,doctest=doctest)

    def test_all_classes_preserve_workspace_and_only_two_concurrent_cargos(self):
        groups = self.plan([self.target('lib',doctest=True),self.target('bin'),self.target('test')])
        self.assertEqual([g[0] for g in groups],['integracion','unidades','documentacion'])
        for _, command in groups:
            self.assertEqual(command[:len(banco.BASE)],banco.BASE)
            self.assertEqual(command[-2:],['--','-q'])
        self.assertIn('*',groups[0][1])
        self.assertIn('--bins',groups[1][1])
        self.assertIn('--doc',groups[2][1])

    def test_disabled_library_is_never_forced_into_unit_or_doc_tests(self):
        groups=self.plan([self.target('lib',test=False),self.target('test')])
        self.assertEqual([g[0] for g in groups],['integracion'])

    def test_mixed_or_feature_sensitive_disabled_targets_fall_back(self):
        for targets,manifest in [
            ([self.target('lib',test=False),self.target('bin')],''),
            ([self.target('lib',test=False)],'[features]\ndefault=["a"]\na=[]\n'),
            ([self.target('test',test=False)],''),
        ]:
            self.assertEqual(self.plan(targets,manifest),[('completo',banco.BASE+['--','-q'])])

    def test_new_kinds_examples_and_custom_harness_fall_back(self):
        for targets,manifest in [
            ([self.target('example')],''),
            ([self.target('future-kind')],''),
            ([self.target('test')],'[[test]]\nname="custom"\nharness=false\n'),
        ]:
            self.assertEqual(self.plan(targets,manifest),[('completo',banco.BASE+['--','-q'])])

    def test_compile_only_examples_are_not_run(self):
        groups=self.plan([self.target('example',test=False),self.target('lib',doctest=True)])
        self.assertEqual([g[0] for g in groups],['unidades','documentacion'])

    def test_defaults_of_disabled_targets_are_not_silently_removed(self):
        for dependency,parallel in [(None,True),
                (dict(name='disabled',uses_default_features=False,optional=False),False),
                (dict(name='disabled',uses_default_features=True,optional=False),True),
                (dict(name='disabled',uses_default_features=True,optional=True),False),
                (dict(name='disabled',uses_default_features=True,optional=False,kind='build'),False),
                (dict(name='disabled',uses_default_features=True,optional=False,target='cfg(unix)'),False)]:
            with self.subTest(dependency=dependency), TemporaryDirectory() as folder:
                root=Path(folder); off=root/'off.toml';on=root/'on.toml'
                off.write_text('[features]\ndefault=["alloc"]\nalloc=[]\n',encoding='ascii')
                on.write_text('',encoding='ascii')
                metadata={'packages':[
                    dict(name='disabled',manifest_path=str(off),targets=[self.target('lib',False)],dependencies=[]),
                    dict(name='active',manifest_path=str(on),targets=[self.target('lib',doctest=True),self.target('test')],
                         dependencies=[] if dependency is None else [dependency])
                ]}
                groups=banco.plan(metadata)
                self.assertEqual([g[0] for g in groups],
                    ['integracion','unidades','documentacion'] if parallel else ['completo'])

    def test_collection_keeps_group_pairs_and_every_failure(self):
        with TemporaryDirectory() as folder:
            root=Path(folder); records=[]
            for i,code in enumerate([0,7,0]):
                out,err=root/('out%d'%i),root/('err%d'%i)
                out.write_bytes(('result%d\n'%i).encode());err.write_bytes(('Running group%d\n'%i).encode())
                records.append(dict(stdout=str(out),stderr=str(err),codigo=code))
            console=SimpleNamespace(buffer=io.BytesIO(),flush=lambda:None)
            with patch.object(banco.sys,'stdout',console):
                self.assertEqual(banco.collect(records,root/'combined'),7)
            self.assertEqual(console.buffer.getvalue(),b'result0\nresult1\nresult2\n')
            self.assertEqual((root/'combined').read_bytes(),b'Running group0\nRunning group1\nRunning group2\n')

    def test_full_plan_compiles_first_runs_every_group_and_joins_failure(self):
        with TemporaryDirectory() as folder:
            root=Path(folder); manifest=root/'Cargo.toml'
            manifest.write_text('[package]\nname="fixture"\nversion="0.1.0"\n',encoding='ascii')
            meta={'packages':[dict(name='fixture',manifest_path=str(manifest),targets=[
                self.target('lib',doctest=True),self.target('test')])]}
            run_id='banco-1-'+'a'*32
            argv=['banco','--logs',str(root),'--run-id',run_id,'--stderr',str(root/'combined')]
            warmed=False; running=0; peak=0; seen=[]; lock=threading.Lock()
            def execute(name,command,logs,env):
                nonlocal warmed,running,peak
                with lock:
                    if name!='construccion': self.assertTrue(warmed)
                    running+=1;peak=max(peak,running);seen.append(name)
                time.sleep(.02)
                out,err=logs/(name+'.out'),logs/(name+'.err')
                out.write_bytes(b'');err.write_bytes(b'')
                with lock:
                    running-=1
                    if name=='construccion': warmed=True
                return dict(nombre=name,codigo=9 if name=='integracion' else 0,
                            segundos=.02,stdout=str(out),stderr=str(err))
            console=SimpleNamespace(buffer=io.BytesIO(),flush=lambda:None)
            with patch.object(sys,'argv',argv), patch.object(sys,'stdout',console), \
                 patch.object(banco.subprocess,'run',return_value=subprocess.CompletedProcess([],0,json.dumps(meta).encode())), \
                 patch.object(banco,'execute',side_effect=execute):
                self.assertEqual(banco.main(),9)
            self.assertEqual(set(seen),{'construccion','integracion','unidades','documentacion'})
            self.assertEqual(peak,2)
            report=json.loads((root/run_id/'tiempos.json').read_text())
            self.assertEqual(len(report['carriles']),4)



if __name__ == '__main__':
    unittest.main()
