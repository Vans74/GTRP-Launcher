#!/usr/bin/env python3
"""Normalise les noms de noeuds DFF pour la limite SA-MP 0.3.DL (23 octets)."""

from pathlib import Path
import struct
import sys


CLUMP = 0x10
FRAME_LIST = 0x0E
EXTENSION = 0x03
NODE_NAME = 0x0253F2FE


def chunks(payload: bytes, context: str):
    offset = 0
    while offset < len(payload):
        if len(payload) - offset < 12:
            raise ValueError(f"{context}: en-tete tronque a {offset}")
        chunk_type, size, version = struct.unpack_from("<III", payload, offset)
        end = offset + 12 + size
        if end > len(payload):
            raise ValueError(f"{context}: chunk 0x{chunk_type:X} depasse son parent")
        yield chunk_type, version, payload[offset + 12 : end]
        offset = end
    if offset != len(payload):
        raise ValueError(f"{context}: taille incoherente")


def pack_chunk(chunk_type: int, version: int, payload: bytes) -> bytes:
    return struct.pack("<III", chunk_type, len(payload), version) + payload


def normalise_node(raw: bytes):
    # Rockstar definit la taille du nom par celle du chunk : aucun NUL final.
    clean = raw.rstrip(b"\0")
    try:
        text = clean.decode("ascii")
    except UnicodeDecodeError as exc:
        raise ValueError("nom de noeud non ASCII") from exc
    if len(clean) > 23:
        clean = clean.replace(b" ", b"_")[:23]
    return clean, text


def patch_extension(payload: bytes, reports: list[str]) -> bytes:
    output = []
    for chunk_type, version, child in chunks(payload, "Extension"):
        if chunk_type == NODE_NAME:
            fixed, old_text = normalise_node(child)
            if fixed != child:
                reports.append(
                    f"{old_text!r}: chunk {len(child)} -> {len(fixed)} octets, "
                    f"nom {fixed.decode('ascii')!r}"
                )
            child = fixed
        output.append(pack_chunk(chunk_type, version, child))
    return b"".join(output)


def patch_frame_list(payload: bytes, reports: list[str]) -> bytes:
    output = []
    for chunk_type, version, child in chunks(payload, "FrameList"):
        if chunk_type == EXTENSION:
            child = patch_extension(child, reports)
        output.append(pack_chunk(chunk_type, version, child))
    return b"".join(output)


def patch_clump(payload: bytes, reports: list[str]) -> bytes:
    output = []
    for chunk_type, version, child in chunks(payload, "Clump"):
        if chunk_type == FRAME_LIST:
            child = patch_frame_list(child, reports)
        output.append(pack_chunk(chunk_type, version, child))
    return b"".join(output)


def patch_dff(src: str, dst: str) -> int:
    original = Path(src).read_bytes()
    reports: list[str] = []
    if len(original) < 12:
        raise ValueError("DFF tronque")
    chunk_type, size, version = struct.unpack_from("<III", original, 0)
    if chunk_type != CLUMP or 12 + size > len(original):
        raise ValueError("le premier chunk du DFF n'est pas un Clump valide")
    clump_end = 12 + size
    payload = patch_clump(original[12:clump_end], reports)
    # Certains exports ajoutent un chunk Extension vide et du padding après le
    # Clump. Ces octets ne font pas partie de la géométrie : on les conserve à
    # l'identique au lieu d'essayer de les réinterpréter.
    trailing = original[clump_end:]
    result = pack_chunk(chunk_type, version, payload) + trailing

    # Validation structurelle complete du resultat avant toute ecriture.
    remaining: list[str] = []
    checked_type, checked_size, checked_version = struct.unpack_from("<III", result, 0)
    checked_end = 12 + checked_size
    checked_payload = patch_clump(result[12:checked_end], remaining)
    validated = (
        pack_chunk(checked_type, checked_version, checked_payload)
        + result[checked_end:]
    )
    if remaining or validated != result:
        raise ValueError("le DFF normalise n'est pas idempotent")

    Path(dst).write_bytes(result)
    for report in reports:
        print(f"  -> {report}")
    print(f"  -> taille {len(original)} -> {len(result)} octets")
    return len(reports)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(f"Usage: {sys.argv[0]} SOURCE.dff DESTINATION.dff")
    count = patch_dff(sys.argv[1], sys.argv[2])
    print(f"  -> {sys.argv[2]} OK ({count} noeud(s) normalise(s))")
