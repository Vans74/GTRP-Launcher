#!/usr/bin/env python3
"""Abaisse (en Z local) des frames NOMMÉES précises d'un DFF, sans rien d'autre.

Sert au calage fin des coronas d'urgence ImVehFt : les frames `light_em*` servent
d'ancre aux coronas, mais ImVehFt dessine la corona LÉGÈREMENT AU-DESSUS du point.
On abaisse donc ces ancres pour que la lumière retombe pile sur la rampe.

Écriture : patch binaire des 4 octets du flottant Z (structure du DFF intacte,
DragonFF ne sachant pas réécrire ce fichier). Lecture des noms via DragonFF.

Usage : lower-frames-z.py <in.dff> <out.dff> <dz> <nom1> [nom2 ...]
        (les noms sont insensibles à la casse ; correspondance exacte)
"""
import struct
import sys

FRAME_SIZE = 56
POS_Z_OFF = 44
CHUNK_STRUCT = 0x01
CHUNK_FRAMELIST = 0x0E
CHUNK_CLUMP = 0x10


def find_frame_array(data: bytes):
    def read_hdr(off):
        typ, size, _ = struct.unpack_from("<III", data, off)
        return typ, size, off + 12

    typ, size, clump_data = read_hdr(0)
    if typ != CHUNK_CLUMP:
        raise SystemExit(f"ERREUR: chunk racine {typ:#x} != Clump")
    off, end = clump_data, clump_data + size
    while off < end:
        ctyp, csize, cdata = read_hdr(off)
        if ctyp == CHUNK_FRAMELIST:
            styp, _, sdata = read_hdr(cdata)
            if styp != CHUNK_STRUCT:
                raise SystemExit("ERREUR: Frame List sans Struct")
            num = struct.unpack_from("<I", data, sdata)[0]
            return sdata + 4, num
        off = cdata + csize
    raise SystemExit("ERREUR: Frame List introuvable")


def main() -> None:
    if len(sys.argv) < 5:
        raise SystemExit(f"Usage: {sys.argv[0]} <in.dff> <out.dff> <dz> <nom1> [nom2 ...]")
    src, dst, dz = sys.argv[1], sys.argv[2], float(sys.argv[3])
    wanted = {n.lower() for n in sys.argv[4:]}

    sys.path.insert(0, "/tmp/DragonFF")
    from gtaLib import dff  # noqa: WPS433

    obj = dff.dff()
    obj.load_file(src)
    frames = obj.frame_list

    data = bytearray(open(src, "rb").read())
    base, n_bin = find_frame_array(bytes(data))
    if n_bin != len(frames):
        raise SystemExit(f"ERREUR: nb frames binaire {n_bin} != DragonFF {len(frames)}")

    hit = 0
    size_before = len(data)
    print(f"  -> abaissement dz={dz} des frames : {sorted(wanted)}")
    for i, fr in enumerate(frames):
        if (fr.name or "").lower() in wanted:
            fo = base + i * FRAME_SIZE + POS_Z_OFF
            z = struct.unpack_from("<f", data, fo)[0]
            struct.pack_into("<f", data, fo, z + dz)
            print(f"       [{i:>3}] {fr.name:<14} z {z:+.4f} -> {z + dz:+.4f}")
            hit += 1

    if hit == 0:
        raise SystemExit(f"ERREUR: aucune frame nommée {sorted(wanted)} trouvée")

    assert len(data) == size_before, "taille modifiée — abandon"
    open(dst, "wb").write(bytes(data))
    print(f"  -> {dst} OK ({hit} frame(s), taille inchangée : {size_before} o)")


if __name__ == "__main__":
    main()
