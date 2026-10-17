"""Banco completo en dos carriles Cargo, sin seleccionar paquetes a mano.

Primero construye TODOS los objetivos por la orden original --no-run, incluidos
los ejemplos. Despues separa unidades/binarios, integracion y documentacion.
Cargo conserva cwd, entorno, features y ejecucion del harness. No invoca .exe
por su cuenta ni usa resultados anteriores como aprobacion.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import tomllib

ROOT = Path(__file__).resolve().parents[3]
BASE = ['cargo', 'test', '--workspace', '--exclude', 'bmo-kernel', '--no-fail-fast']


def defaults_kept(metadata, package, kind):
    """Un root activo ya pide sus defaults: excluirlo como test no los pierde."""
    # Un paquete aislado, sin aristas entrantes ni salientes, no puede variar
    # las features de otro root. Sigue comprobado por --no-run.
    if not package.get('dependencies') and not any(
            d['name'] == package['name'] for p in metadata['packages']
            for d in p.get('dependencies', [])):
        return True
    for other in metadata['packages']:
        if other['name'] in (package['name'], 'bmo-kernel'):
            continue
        active = any((kind == 'unit' and t.get('test') and any(k in ('lib', 'bin') for k in t['kind']))
                     or (kind == 'doc' and 'lib' in t['kind'] and t.get('doctest'))
                     for t in other['targets'])
        if active and any(d['name'] == package['name'] and d.get('uses_default_features')
                          and not d.get('optional') and d.get('kind') is None and d.get('target') is None
                          for d in other.get('dependencies', [])):
            return True
    return False


def plan(metadata):
    """Una clase nueva no se omite: vuelve a la orden completa original."""
    unit_excludes, doc_excludes = [], []
    for package in metadata['packages']:
        if package['name'] == 'bmo-kernel':
            continue
        manifest = tomllib.loads(Path(package['manifest_path']).read_text(encoding='utf-8'))
        for kind in ('lib', 'bin', 'test', 'example'):
            entries = manifest.get(kind, [])
            if isinstance(entries, dict):
                entries = [entries]
            if any(e.get('harness') is False for e in entries):
                return [('completo', BASE + ['--', '-q'])]
        units = [t for t in package['targets'] if any(k in ('lib', 'bin') for k in t['kind'])]
        if any(not t.get('test') for t in units):
            # No forzar test=false con --lib/--bins. Casos mixtos vuelven al original.
            if any(t.get('test') for t in units) or (manifest.get('features', {}).get('default') and not defaults_kept(metadata, package, 'unit')):
                return [('completo', BASE + ['--', '-q'])]
            unit_excludes += ['--exclude', package['name']]
        libraries = [t for t in package['targets'] if 'lib' in t['kind']]
        if libraries and not libraries[0].get('doctest'):
            if manifest.get('features', {}).get('default') and not defaults_kept(metadata, package, 'doc'):
                return [('completo', BASE + ['--', '-q'])]
            doc_excludes += ['--exclude', package['name']]
        for target in package['targets']:
            kinds = target['kind']
            if 'test' in kinds and not target.get('test'):
                return [('completo', BASE + ['--', '-q'])]
            if ('example' in kinds and target.get('test')) or any(
                    k not in ('lib', 'bin', 'test', 'example', 'custom-build') for k in kinds):
                return [('completo', BASE + ['--', '-q'])]
    targets = [t for p in metadata['packages'] if p['name'] != 'bmo-kernel' for t in p['targets']]
    groups = []
    if any('test' in t['kind'] and t.get('test') for t in targets):
        groups.append(('integracion', BASE + ['--test', '*', '--', '-q']))
    if any(any(k in ('lib', 'bin') for k in t['kind']) and t.get('test') for t in targets):
        groups.append(('unidades', BASE + unit_excludes + ['--lib', '--bins', '--', '-q']))
    if any('lib' in t['kind'] and t.get('doctest') for t in targets):
        groups.append(('documentacion', BASE + doc_excludes + ['--doc', '--', '-q']))
    return groups or [('completo', BASE + ['--', '-q'])]


def execute(name, command, folder, env):
    start = time.perf_counter()
    stdout, stderr = folder / (name + '.stdout.txt'), folder / (name + '.stderr.txt')
    with stdout.open('wb') as out, stderr.open('wb') as err:
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=out, stderr=err)
    return dict(nombre=name, codigo=result.returncode, segundos=time.perf_counter()-start,
                stdout=str(stdout), stderr=str(stderr))


def collect(records, stderr_path):
    """Mismo orden de grupos en ambos canales; nunca mezcla parejas de resultados."""
    with stderr_path.open('wb') as err:
        for record in records:
            err.write(Path(record['stderr']).read_bytes())
            sys.stdout.flush()
            sys.stdout.buffer.write(Path(record['stdout']).read_bytes())
            sys.stdout.flush()
    return next((r['codigo'] for r in records if r['codigo'] != 0), 0)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--logs', required=True, type=Path)
    parser.add_argument('--run-id', required=True)
    parser.add_argument('--stderr', required=True, type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r'banco-[0-9]+(?:-[0-9a-f]{32})?', args.run_id):
        parser.error('run-id debe ser banco-<pid> con UUID opcional')
    folder = args.logs / args.run_id
    folder.mkdir(parents=True, exist_ok=True)
    args.stderr.write_bytes(b'')
    start = time.perf_counter()
    records = []
    try:
        meta = subprocess.run(['cargo', 'metadata', '--no-deps', '--format-version', '1'],
                              cwd=ROOT, capture_output=True, check=True)
        metadata = json.loads(meta.stdout)
        if not metadata.get('packages'):
            raise ValueError('workspace sin paquetes: no se puede verificar')
        groups = plan(metadata)
        env = os.environ.copy()
        # Dos binarios como maximo; el presupuesto total de threads es el CPU.
        env.setdefault('RUST_TEST_THREADS', str(max(1, (os.cpu_count() or 2)//2)))
        if groups[0][0] != 'completo':
            warm = execute('construccion', BASE + ['--no-run'], folder, env)
            records.append(warm)
            if warm['codigo']:
                return collect(records, args.stderr)
        with ThreadPoolExecutor(max_workers=2) as pool:
            futures = [pool.submit(execute, name, command, folder, env) for name, command in groups]
            # Todos terminan, incluso si uno falla: equivale a --no-fail-fast.
            records.extend(f.result() for f in futures)
        return collect(records, args.stderr)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        with args.stderr.open('a', encoding='utf-8') as err:
            err.write('error: banco no pudo ejecutar su plan: %s\n' % error)
        print('banco: ejecucion incompleta; ver stderr')
        return 1
    finally:
        report = dict(total_segundos=time.perf_counter()-start, carriles=records)
        (folder/'tiempos.json').write_text(json.dumps(report, indent=2), encoding='utf-8')


if __name__ == '__main__':
    sys.exit(main())
