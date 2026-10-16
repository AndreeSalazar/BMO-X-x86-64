"""Inventario local para MUNDO: lee recursos y firmas, sin ejecutar Java.

No extrae clases, imagenes ni mundos. El resultado va a target/ por defecto.
Las firmas describen interfaces; no reconstruyen el cuerpo de los metodos.
"""

import argparse
import hashlib
import json
import struct
import zipfile
from pathlib import Path


class Reader:
    def __init__(self, data):
        self.data = data
        self.offset = 0

    def take(self, count):
        end = self.offset + count
        if count < 0 or end > len(self.data):
            raise ValueError("clase truncada")
        value = self.data[self.offset:end]
        self.offset = end
        return value

    def number(self, count):
        return int.from_bytes(self.take(count), "big")


def class_signatures(data):
    r = Reader(data)
    if r.take(4) != b"\xca\xfe\xba\xbe":
        raise ValueError("no es una clase Java")
    minor, major = r.number(2), r.number(2)
    pool = [None] * r.number(2)
    i = 1
    widths = {3: 4, 4: 4, 5: 8, 6: 8, 7: 2, 8: 2, 9: 4,
              10: 4, 11: 4, 12: 4, 15: 3, 16: 2, 17: 4, 18: 4,
              19: 2, 20: 2}
    while i < len(pool):
        tag = r.number(1)
        if tag == 1:
            pool[i] = r.take(r.number(2)).decode("utf-8", errors="replace")
        elif tag in widths:
            r.take(widths[tag])
        else:
            raise ValueError(f"constant pool tag {tag} desconocido")
        i += 2 if tag in (5, 6) else 1
    r.take(6)  # flags, this_class, super_class
    r.take(2 * r.number(2))  # interfaces

    def members():
        out = []
        for _ in range(r.number(2)):
            flags, name, descriptor = r.number(2), r.number(2), r.number(2)
            item = {"name": pool[name], "descriptor": pool[descriptor], "flags": flags}
            for _ in range(r.number(2)):
                attribute = pool[r.number(2)]
                body = r.take(r.number(4))
                if attribute == "Code":
                    code = Reader(body)
                    item["max_stack"] = code.number(2)
                    item["locals"] = code.number(2)
                    item["bytecode_bytes"] = code.number(4)
                    code.take(item["bytecode_bytes"])
            out.append(item)
        return out

    fields = members()
    methods = members()
    return {"major": major, "minor": minor, "fields": len(fields), "methods": methods}


def read_entry(archive, name):
    if archive.getinfo(name).file_size > 4 * 1024 * 1024:
        raise ValueError(f"entrada demasiado grande: {name}")
    return archive.read(name)


def inventory(game, version, world):
    folder = game / "versions" / version
    jar = folder / f"{version}.jar"
    meta = json.loads((folder / f"{version}.json").read_text(encoding="utf-8"))
    with jar.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    report = {"version": meta["id"], "java": meta.get("javaVersion"),
              "jar_bytes": jar.stat().st_size,
              "jar_sha256": digest}
    classes = ["world/level/block/Block", "world/level/block/state/BlockBehaviour",
               "client/Camera", "client/player/LocalPlayer",
               "client/renderer/LevelRenderer", "client/renderer/chunk/SectionCompiler"]
    with zipfile.ZipFile(jar) as archive:
        names = archive.namelist()
        report["counts"] = {
            "block_png": sum(n.startswith("assets/minecraft/textures/block/") and n.endswith(".png") for n in names),
            "block_models": sum(n.startswith("assets/minecraft/models/block/") and n.endswith(".json") for n in names),
            "blockstates": sum(n.startswith("assets/minecraft/blockstates/") and n.endswith(".json") for n in names),
            "classes": sum(n.endswith(".class") for n in names),
            "java_sources": sum(n.endswith(".java") for n in names),
        }
        report["classes"] = {}
        for name in classes:
            entry = f"net/minecraft/{name}.class"
            if entry in names:
                report["classes"][name] = class_signatures(read_entry(archive, entry))
        report["basic_models"] = {}
        for block in ("stone", "dirt", "grass_block", "oak_log", "oak_leaves", "oak_planks"):
            entry = f"assets/minecraft/models/block/{block}.json"
            if entry not in names:
                continue
            model = json.loads(read_entry(archive, entry))
            report["basic_models"][block] = {
                "parent": model.get("parent"), "textures": model.get("textures", {}),
                "elements": len(model.get("elements", [])),
                "tinted_faces": sum("tintindex" in f for e in model.get("elements", []) for f in e.get("faces", {}).values()),
            }
        report["basic_textures"] = {}
        for texture in ("stone", "dirt", "grass_block_top", "grass_block_side",
                        "grass_block_side_overlay", "oak_log", "oak_leaves", "oak_planks"):
            entry = f"assets/minecraft/textures/block/{texture}.png"
            if entry in names:
                data = read_entry(archive, entry)
                if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
                    raise ValueError("PNG no reconocido")
                report["basic_textures"][texture] = {
                    "size": list(struct.unpack(">II", data[16:24])), "bytes": len(data)}
    save = game / "saves" / world
    report["terrain_regions"] = [
        {"path": p.relative_to(save).as_posix(), "bytes": p.stat().st_size}
        for p in sorted(save.rglob("*.mca")) if p.parent.name == "region"
    ] if save.is_dir() else []
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("game_dir", type=Path)
    parser.add_argument("--version", default="26.3")
    parser.add_argument("--world", default="New World")
    parser.add_argument("--out", type=Path, default=Path("target/estudio/minecraft/inventory.json"))
    args = parser.parse_args()
    report = inventory(args.game_dir, args.version, args.world)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, indent=2, ensure_ascii=True) + "\n", encoding="ascii")
    print(json.dumps({"output": str(args.out), "version": report["version"],
                      "counts": report["counts"], "terrain_regions": len(report["terrain_regions"])}, ensure_ascii=True))


if __name__ == "__main__":
    main()
