"""Arranque UEFI aislado de BMO en QEMU; recoge evidencia, no acredita RTX."""
import argparse
import hashlib
import json
import pathlib
import queue
import shutil
import subprocess
import threading
import time


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--staging', type=pathlib.Path, required=True)
    ap.add_argument('--out', type=pathlib.Path, required=True)
    ap.add_argument('--qemu', type=pathlib.Path, default=pathlib.Path('C:/Program Files/qemu/qemu-system-x86_64.exe'))
    ap.add_argument('--share', type=pathlib.Path, default=pathlib.Path('C:/Program Files/qemu/share'))
    ap.add_argument('--seconds', type=int, default=45)
    ap.add_argument('--accel', choices=('whpx', 'tcg'), default='whpx')
    ap.add_argument('--usb', action='store_true', help='incluir xHCI virtual (opcional para diagnostico)')
    args = ap.parse_args()
    if not 5 <= args.seconds <= 180:
        ap.error('--seconds debe estar entre 5 y 180')
    staging, out = args.staging.resolve(), args.out.resolve()
    efi = staging / 'EFI/BOOT/BOOTX64.EFI'
    code, template = args.share / 'edk2-x86_64-code.fd', args.share / 'edk2-i386-vars.fd'
    for p in (args.qemu, efi, code, template):
        if not p.is_file():
            ap.error(f'falta {p}')
    out.mkdir(parents=True, exist_ok=False)
    esp = out / 'esp'
    shutil.copytree(staging / 'EFI', esp / 'EFI')
    if (staging / 'BMO-DATA').is_dir():
        shutil.copytree(staging / 'BMO-DATA', esp, dirs_exist_ok=True)
    variables = out / 'vars.fd'
    shutil.copyfile(template, variables)
    overlay = out / 'disco.qcow2'
    image_tool = args.qemu.with_name('qemu-img.exe' if args.qemu.suffix == '.exe' else 'qemu-img')
    subprocess.run([str(image_tool), 'create', '-f', 'qcow2', '-F', 'raw',
                    '-b', f'fat:ro:{esp.as_posix()}', str(overlay)], check=True, capture_output=True)
    command = [str(args.qemu), '-name', 'BMO-X prueba UEFI aislada',
               '-machine', 'pc', '-accel', args.accel,
               '-cpu', 'host' if args.accel == 'whpx' else 'max',
               '-smp', '1', '-m', '2048',
               '-drive', f'if=pflash,format=raw,readonly=on,file={code.as_posix()}',
               '-drive', f'if=pflash,format=raw,file={variables.as_posix()}',
               '-device', 'ich9-ahci,id=ahci',
               '-drive', f'format=qcow2,if=none,id=bmo,file={overlay.as_posix()}',
               '-device', 'ide-hd,drive=bmo,bus=ahci.0',
               '-vga', 'std', '-nic', 'none', '-display', 'none',
               '-serial', f'file:{(out / "serial.log").as_posix()}',
               '-monitor', 'none', '-qmp', 'stdio', '-no-reboot', '-no-shutdown']
    if args.usb:
        command += ['-device', 'qemu-xhci', '-device', 'usb-kbd', '-device', 'usb-mouse']
    report = {'command': command, 'efi_sha256': hashlib.sha256(efi.read_bytes()).hexdigest(),
              'qemu_version': subprocess.check_output([str(args.qemu), '--version'], text=True).splitlines()[0],
              'elapsed_seconds_requested': args.seconds, 'rtx_tested': False,
              'graphics': 'std VGA emulada; sin passthrough',
              'result': 'not_started', 'boot_success': None}
    replies = queue.Queue()
    counter = 0
    with (out / 'qemu.stderr.log').open('w', encoding='utf-8') as err:
        proc = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=err, text=True, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        def reader():
            for line in proc.stdout:
                replies.put(json.loads(line))
        threading.Thread(target=reader, daemon=True).start()

        def request(name, arguments=None):
            nonlocal counter
            counter += 1
            msg = {'execute': name, 'id': counter}
            if arguments:
                msg['arguments'] = arguments
            proc.stdin.write(json.dumps(msg) + '\n')
            proc.stdin.flush()
            limit = time.monotonic() + 10
            while time.monotonic() < limit:
                reply = replies.get(timeout=max(0.01, limit - time.monotonic()))
                if reply.get('id') == counter:
                    if 'error' in reply:
                        raise RuntimeError(reply['error'])
                    return reply['return']
            raise TimeoutError(name)
        try:
            greeting = replies.get(timeout=15)
            if 'QMP' not in greeting:
                raise RuntimeError('QMP no entrego su saludo')
            request('qmp_capabilities')
            started = time.monotonic()
            while time.monotonic() - started < args.seconds and proc.poll() is None:
                time.sleep(0.2)
            report['status'] = request('query-status')
            report['pci'] = request('query-pci')
            request('screendump', {'filename': (out / 'pantalla.png').as_posix(), 'format': 'png'})
            report['result'] = 'evidence_collected_requires_inspection'
        except Exception as exc:
            report['result'] = 'failed'
            report['error'] = f'{type(exc).__name__}: {exc}'
        finally:
            if proc.poll() is None:
                try:
                    request('quit')
                except (OSError, RuntimeError, TimeoutError, queue.Empty):
                    proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=5)
            report['process_exit'] = proc.returncode
    (out / 'resultado.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps({k: v for k, v in report.items() if k not in ('command', 'pci')}, indent=2))
    return 0 if report['result'] != 'failed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
