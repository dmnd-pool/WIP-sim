"""Invoke unchanged Project X-Ray predicates in the captured Python environment.

The Rust launcher pins the upstream files and confines this process. This file
does not implement a second feature matcher or a bitstream packet decoder.
"""
import gzip
import hashlib
import io
import json
import struct
import sys
from types import SimpleNamespace

sys.path[:0] = ["/upstream", "/yaml-package"]
from prjxray import bitstream
from prjxray.grid_types import BitAlias, Bits, BlockType
from prjxray.tile_segbits import TileSegbits
from prjxray.tile_segbits_alias import TileSegbitsAlias


def checked(path, identity):
    with open(path, "rb") as source:
        data = source.read(identity["size"] + 1)
    assert len(data) == identity["size"]
    assert hashlib.sha256(data).hexdigest() == identity["sha256"]
    return data


def digest_matches(matches):
    """Canonical record bytes; no feature condition is evaluated here."""
    digest = hashlib.sha256(b"md-sim-prjxray-matches/1\0")
    matches = sorted(matches, key=lambda match: match["name"])
    assert len({match["name"] for match in matches}) == len(matches)
    digest.update(struct.pack("<I", len(matches)))
    for match in matches:
        for key in ("name", "database_feature"):
            data = match[key].encode("utf-8")
            digest.update(struct.pack("<I", len(data)))
            digest.update(data)
        positions = sorted(match["asserted_bits"])
        assert match["all_zero_conditions"] == (len(positions) == 0)
        digest.update(bytes([match["all_zero_conditions"]]))
        digest.update(struct.pack("<I", len(positions)))
        for frame, bit in positions:
            assert 0 <= frame < 2**32 and 0 <= bit < 3232
            digest.update(struct.pack("<II", frame, bit))
    return digest.hexdigest()


def frame_data(manifest):
    with open("/frame-map.json", encoding="utf-8") as source:
        oracle = json.load(source)
    assert oracle["inspection_error"] is None
    assert oracle["compressed"] == manifest["bitstream"]["object"]
    assert oracle["frames"]["part_sha256"] == manifest["part"]["object"]["sha256"]
    compressed = checked("/firmware.bit.gz", manifest["bitstream"]["object"])
    with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as source:
        expanded = source.read((4 << 20) + 1)
        assert len(expanded) <= 4 << 20 and source.read(1) == b""
    assert len(expanded) == oracle["expanded_bytes"]
    assert hashlib.sha256(expanded).hexdigest() == oracle["expanded_sha256"]
    header = oracle["header"]
    configuration = expanded[header["configuration_offset"]:]
    assert len(configuration) == header["configuration_size"]
    frames = oracle["frames"]["frames"]
    assert len(frames) == 5144
    assert len({frame["address"] for frame in frames}) == len(frames)

    def lines():
        for frame in frames:
            offset = frame["byte_offset"]
            data = configuration[offset:offset + 404]
            assert len(data) == 404
            assert hashlib.sha256(data).hexdigest() == frame["sha256"]
            for index, (word,) in enumerate(struct.iter_unpack(">I", data)):
                for bit in range(32):
                    if word & (1 << bit):
                        yield "bit_%08x_%03d_%02d\n" % (frame["address"], index, bit)

    return configuration, bitstream.load_bitdata(lines())


def segment_matches(matcher, block, bits, data, physical_tile):
    matches = []
    for positions, feature in matcher.match_bitdata(block, bits, data):
        original = (matcher.map_feature_to_segbits(feature)
                    if isinstance(matcher, TileSegbitsAlias) else feature)
        matches.append({"name": physical_tile + "." + feature.split(".", 1)[1],
                        "database_feature": original,
                        "all_zero_conditions": not positions,
                        "asserted_bits": positions})
    return matches


def alias_fixture():
    # These are the same fixed predicates and coordinates used by the Rust
    # fixture. Each expected result is also asserted directly below.
    text = ("PARENT.ALIAS_SITE.POSITIVE 00_64 !00_65\n"
            "PARENT.ALIAS_SITE.DEFAULT !00_65 !00_66\n"
            "PARENT.ALIAS_SITE.OUTSIDE 00_64 !00_01\n"
            "PARENT.ALIAS_SITE.FALSE 00_65\n")
    path = "/tmp/alias.db"
    with open(path, "w", encoding="ascii") as output:
        output.write(text)
    table = TileSegbits(SimpleNamespace(ppips=None, segbits=path, block_ram_segbits=None))
    db = SimpleNamespace(tile_types={"TYPE": SimpleNamespace(ppips=None)},
                         get_tile_segbits=lambda _: table)
    block = BlockType.CLB_IO_CLK
    bits = Bits(base_address=0, frames=1, offset=0, words=2,
                alias=BitAlias(tile_type="PARENT", start_offset=2,
                               sites={"REAL_SITE": "ALIAS_SITE"}))
    matcher = TileSegbitsAlias(db, "TYPE", {block: bits})
    cases = [({0}, ["DEFAULT", "POSITIVE"]),
             ({0, 1}, ["FALSE"]), ({0, 2}, ["POSITIVE"]), (set(), ["DEFAULT"])]
    outputs = []
    for set_bits, expected in cases:
        matches = segment_matches(matcher, block, bits, {0: ({0}, set_bits)}, "TILE")
        assert sorted(match["name"].split(".")[-1] for match in matches) == expected
        outputs.append({"set_bits": sorted(set_bits), "matches_sha256": digest_matches(matches),
                        "match_count": len(matches)})
    assert sum(any(not matcher.match_filter(block, bit) for bit in rule)
               for rule in table.segbits[block].values()) == 1
    return {"cases": outputs, "outside_alias_rules": 1, "table_sha256": hashlib.sha256(text.encode()).hexdigest()}


def main():
    with open("/feature-inputs.json", encoding="utf-8") as source:
        manifest = json.load(source)
    configuration, data = frame_data(manifest)
    checked("/part.json", manifest["part"]["object"])
    grid = json.loads(checked("/tilegrid.json", manifest["tilegrid"]["object"]))
    tables, identities = {}, []
    for index, table in enumerate(manifest["tables"]):
        path = "/feature-table-%d.db" % index
        checked(path, table["input"]["object"])
        block = BlockType[table["bus"]]
        database = TileSegbits(SimpleNamespace(
            ppips=None, segbits=path if block == BlockType.CLB_IO_CLK else None,
            block_ram_segbits=path if block == BlockType.BLOCK_RAM else None))
        tables[table["tile_type"], table["bus"]] = database, table["input"]["object"]["sha256"]
        identities.append({"tile_type": table["tile_type"], "bus": table["bus"],
                           "sha256": table["input"]["object"]["sha256"],
                           "rules": len(database.segbits[block])})
    decoded, unresolved, explained = [], [], set()
    total_matches = zero_matches = 0
    for tile, raw_tile in sorted(grid.items()):
        for bus, raw in sorted(raw_tile["bits"].items()):
            alias = raw.get("alias")
            required = alias["type"] if alias else raw_tile["type"]
            if (required, bus) not in tables:
                unresolved.append({"tile": tile, "tile_type": raw_tile["type"], "bus": bus,
                                   "required_database_type": required,
                                   "reason": "missing_pinned_feature_database"})
                continue
            block = BlockType[bus]
            table, sha256 = tables[required, bus]
            bits = Bits(base_address=int(raw["baseaddr"], 16), frames=raw["frames"],
                        offset=raw["offset"], words=raw["words"],
                        alias=(BitAlias(tile_type=alias["type"], start_offset=alias["start_offset"],
                                        sites=alias["sites"]) if alias else None))
            matcher, outside = table, 0
            if alias:
                # All eight aliases in the pinned selected grid use one bus.
                # Do not silently omit mappings from a future multi-bus alias.
                assert list(raw_tile["bits"]) == [bus], "multi-bus alias oracle is unsupported"
                db = SimpleNamespace(tile_types={raw_tile["type"]: SimpleNamespace(ppips=None)},
                                     get_tile_segbits=lambda _: table)
                matcher = TileSegbitsAlias(db, raw_tile["type"], {block: bits})
                outside = sum(any(not matcher.match_filter(block, bit) for bit in rule)
                              for rule in table.segbits[block].values())
            matches = segment_matches(matcher, block, bits, data, tile)
            zero_count = sum(match["all_zero_conditions"] for match in matches)
            total_matches += len(matches)
            zero_matches += zero_count
            for match in matches:
                explained.update(match["asserted_bits"])
            decoded.append({"tile": tile, "tile_type": raw_tile["type"], "bus": bus,
                            "database_sha256": sha256, "alias": alias,
                            "features_outside_alias_range": outside,
                            "matches_sha256": digest_matches(matches),
                            "match_count": len(matches), "zero_count": zero_count})
    all_set = {(frame, bit) for frame, (_, positions) in data.items()
               for bit in positions if not 1600 <= bit < 1613}
    assert explained <= all_set
    unknown = sorted(all_set - explained)
    unexplained = []
    for frame, bit in unknown:
        if not unexplained or unexplained[-1]["frame"] != frame:
            unexplained.append({"frame": frame, "bits": []})
        unexplained[-1]["bits"].append(bit)
    report = {"configuration_sha256": hashlib.sha256(configuration).hexdigest(),
              "part_sha256": manifest["part"]["object"]["sha256"],
              "tilegrid_sha256": manifest["tilegrid"]["object"]["sha256"],
              "tables": identities, "decoded": decoded, "unresolved": unresolved,
              "matched_features": total_matches, "all_zero_condition_matches": zero_matches,
              "configuration_set_bits_excluding_ecc": len(all_set),
              "set_bits_explained_by_matching_features": len(explained),
              "set_bits_without_matching_features": len(unknown), "unexplained": unexplained}
    json.dump({"schema": "md-sim-prjxray-feature-oracle/1", "decoding": report,
               "alias_fixture": alias_fixture()}, sys.stdout, separators=(",", ":"))
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()
