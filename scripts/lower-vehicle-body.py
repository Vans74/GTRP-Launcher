#!/usr/bin/env python3
"""Abaisse UNIFORMÉMENT toute la carrosserie d'un véhicule GTA SA (DFF), roues exclues.

Pourquoi « tout » et pas seulement `chassis_dummy` : dans ce modèle (et beaucoup
d'autres), les pièces de carrosserie ne sont PAS toutes enfants de `chassis_dummy`.
Portes, capot, coffre, pare-chocs, phares et surtout les gyrophares `light_em*`
sont des enfants DIRECTS de la racine. Décaler seulement `chassis_dummy` ne bouge
donc que la coque : le reste « flotte » à l'ancienne hauteur (bug précédent).

Méthode correcte : on décale de `dz` (en Z local) TOUTES les frames enfants
directes de la racine (parent == 0), SAUF les roues (wheel_*_dummy), qui définissent
le contact au sol. Comme chaque sous-arbre suit son parent, décaler ce seul niveau
« racine » déplace toute la voiture d'un bloc, uniformément, sans cumul ni
déformation. Les roues restant en place, la caisse descend → stance abaissée.

Écriture : DragonFF ne sait pas réécrire ce DFF (bug interne), on patche donc
directement les 4 octets du flottant Z de chaque frame ciblée. La structure du
fichier (taille, chunks, noms) reste rigoureusement identique.

Usage : lower-vehicle-body.py <in.dff> <out.dff> [dz=-0.055]
"""
import struct
import sys

# Frame RenderWare = 3x3 matrice (36o) + position xyz (12o) + parent (4o) + flags (4o)
FRAME_SIZE = 56
POS_X_OFF = 36  # dans la frame
POS_Z_OFF = 44
PARENT_OFF = 48

CHUNK_STRUCT = 0x01
CHUNK_FRAMELIST = 0x0E
CHUNK_CLUMP = 0x10


def find_frame_array(data: bytes):
    """Renvoie (base_offset, num_frames) du tableau de frames dans le binaire DFF."""

    def read_hdr(off):
        typ, size, ver = struct.unpack_from("<III", data, off)
        return typ, size, off + 12  # (type, taille données, offset données)

    typ, size, clump_data = read_hdr(0)
    if typ != CHUNK_CLUMP:
        raise SystemExit(f"ERREUR: chunk racine {typ:#x} != Clump")
    # parcourt les enfants du clump pour trouver la Frame List
    off = clump_data
    end = clump_data + size
    while off < end:
        ctyp, csize, cdata = read_hdr(off)
        if ctyp == CHUNK_FRAMELIST:
            styp, ssize, sdata = read_hdr(cdata)  # Struct interne
            if styp != CHUNK_STRUCT:
                raise SystemExit("ERREUR: Frame List sans chunk Struct")
            num = struct.unpack_from("<I", data, sdata)[0]
            return sdata + 4, num
        off = cdata + csize
    raise SystemExit("ERREUR: Frame List introuvable")


def main() -> None:
    if len(sys.argv) < 3:
        raise SystemExit(f"Usage: {sys.argv[0]} <in.dff> <out.dff> [dz=-0.055]")
    src, dst = sys.argv[1], sys.argv[2]
    dz = float(sys.argv[3]) if len(sys.argv) > 3 else -0.055

    # --- lecture des métadonnées (noms + parents) via DragonFF -----------------
    sys.path.insert(0, "/tmp/DragonFF")
    from gtaLib import dff  # noqa: WPS433

    obj = dff.dff()
    obj.load_file(src)
    frames = obj.frame_list
    n_dff = len(frames)

    data = bytearray(open(src, "rb").read())
    base, n_bin = find_frame_array(bytes(data))
    if n_bin != n_dff:
        raise SystemExit(f"ERREUR: nb frames binaire {n_bin} != DragonFF {n_dff}")

    # --- vérification croisée : les positions lues en binaire == DragonFF ------
    for i, fr in enumerate(frames):
        fo = base + i * FRAME_SIZE
        bx, by, bz = struct.unpack_from("<fff", data, fo + POS_X_OFF)
        pbin = struct.unpack_from("<i", data, fo + PARENT_OFF)[0]
        if (
            abs(bx - fr.position.x) > 1e-4
            or abs(bz - fr.position.z) > 1e-4
            or pbin != fr.parent
        ):
            raise SystemExit(
                f"ERREUR: désynchro frame {i} ({fr.name}) : "
                f"bin=({bx:.4f},{bz:.4f},p{pbin}) dff=({fr.position.x:.4f},{fr.position.z:.4f},p{fr.parent})"
            )

    # --- sélection des cibles : enfants directs de la racine, hors roues -------
    def is_wheel(name: str) -> bool:
        n = (name or "").lower()
        return n.startswith("wheel_") and n.endswith("_dummy")

    targets = []
    for i, fr in enumerate(frames):
        if fr.parent == 0 and not is_wheel(fr.name):
            targets.append((i, fr.name))

    if not targets:
        raise SystemExit("ERREUR: aucune frame de carrosserie à abaisser (parent==0)")

    size_before = len(data)
    print(f"  -> {len(targets)} frame(s) de carrosserie abaissées de dz={dz} (roues exclues) :")
    for i, name in targets:
        fo = base + i * FRAME_SIZE + POS_Z_OFF
        z = struct.unpack_from("<f", data, fo)[0]
        struct.pack_into("<f", data, fo, z + dz)
        print(f"       [{i:>3}] {name:<20} z {z:+.4f} -> {z + dz:+.4f}")

    kept = [fr.name for fr in frames if fr.parent == 0 and is_wheel(fr.name)]
    print(f"  -> roues conservées en place : {kept}")

    assert len(data) == size_before, "taille modifiée — abandon"
    open(dst, "wb").write(bytes(data))
    print(f"  -> {dst} OK (taille inchangée : {size_before} o)")


if __name__ == "__main__":
    main()
