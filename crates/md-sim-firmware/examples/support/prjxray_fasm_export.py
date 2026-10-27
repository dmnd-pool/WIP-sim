"""Call pinned Project X-Ray APIs with an explicitly adapted FASM grammar.

Rust captures every input and confines this process. The frame reader checks
prior pinned frame offsets against original bytes; it is not a packet decoder.
"""
import gc
import hashlib
import json
import struct
import sys
import time

with open('/tool-inputs.json', encoding='utf-8') as source:
    tools = json.load(source)
sys.path[:0] = ['/upstream', '/parser', '/yaml-package', '/support'] + [
    item['guest_path'] for item in tools['inputs']
    if item['guest_path'].startswith('/wheels/')]
import fasm
from fasm.output import merge_and_sort
from prjxray.db import Database
from prjxray.fasm_disassembler import FasmDisassembler
from prjxray.fasm_assembler import FasmAssembler, FasmLookupError
from prjxray_feature_oracle import checked, frame_data


def phase(name):
    print(json.dumps({'phase': name, 'monotonic_seconds': time.monotonic()}),
          file=sys.stderr, flush=True)


def names(model):
    result = set()
    for line in model:
        if line.set_feature:
            for feature in fasm.canonical_features(line.set_feature):
                result.add(fasm.set_feature_to_str(feature, check_if_canonical=True))
    return result


def name_digest(values):
    digest = hashlib.sha256(b'md-sim-fasm-names/1\0')
    digest.update(struct.pack('<I', len(values)))
    for value in sorted(values):
        encoded = value.encode('ascii')
        digest.update(struct.pack('<I', len(encoded)))
        digest.update(encoded)
    return digest.hexdigest()


def bit_digest(values):
    digest = hashlib.sha256(b'md-sim-fasm-frame-bits/1\0')
    digest.update(struct.pack('<I', len(values)))
    for frame, bit in sorted(values):
        digest.update(struct.pack('<II', frame, bit))
    return digest.hexdigest()


def formatted_line(line):
    """Bound one upstream formatter result; never split an annotation value."""
    assert '\n' not in line and '\r' not in line, 'multiline formatter item is unsupported'
    encoded = line.encode('utf-8')
    assert len(encoded) <= 64 << 10, 'formatter line exceeds bound'
    return len(encoded) + 1


def parse_saved_chunks(path, byte_limit, expected_sha256, consume):
    """Call the unchanged parser on complete, bounded formatter lines."""
    digest = hashlib.sha256()
    chunks = lines = size = maximum = total = 0
    pending = []

    def parse_pending():
        for line in fasm.parse_fasm_string(''.join(pending)):
            consume(line)
        # textX owns cyclic syntax-model objects. Release each completed model.
        gc.collect()

    with open(path, encoding='utf-8', newline='') as source:
        for line in source:
            assert line.endswith('\n') and '\r' not in line
            encoded = line.encode('utf-8')
            assert len(encoded) <= byte_limit
            digest.update(encoded)
            total += len(encoded)
            if pending and size + len(encoded) > byte_limit:
                parse_pending()
                chunks += 1
                pending.clear()
                size = 0
            pending.append(line)
            size += len(encoded)
            maximum = max(maximum, size)
            lines += 1
        if pending:
            parse_pending()
            chunks += 1
    assert digest.hexdigest() == expected_sha256, 'saved FASM text differs'
    return {'chunks': chunks, 'lines': lines, 'maximum_chunk_bytes': maximum,
            'chunk_limit_bytes': byte_limit, 'consumed_bytes': total,
            'consumed_sha256': digest.hexdigest()}


def parser_fixture():
    text = ('# fixed parser boundary regression\n'
            "TILE.INIT[7:0] = 8'h81\n"
            'TILE.ZERO = 0\n'
            'TILE.ENABLE { note = "quoted \\\"text\\\"" } # inline\n'
            '{ unknown_bit = "00000000_0_7" }\n')
    sha256 = hashlib.sha256(text.encode()).hexdigest()
    path = '/tmp/parser-fixture.fasm'
    with open(path, 'w', encoding='utf-8') as target:
        target.write(text)
    expected = [line for line in fasm.parse_fasm_string(text) if any(line)]
    actual = []
    report = parse_saved_chunks(path, 64, sha256, actual.append)
    assert [line for line in actual if any(line)] == expected
    assert report['chunks'] > 1
    try:
        formatted_line('multiline\nannotation')
    except AssertionError:
        pass
    else:
        raise AssertionError('multiline formatter item was not rejected')
    return report


def main():
    fixture = parser_fixture()
    phase('checked-original-frames')
    with open('/feature-inputs.json', encoding='utf-8') as source:
        manifest = json.load(source)
    configuration, data = frame_data(manifest)
    configuration_sha256 = hashlib.sha256(configuration).hexdigest()
    del configuration
    checked('/part.json', manifest['part']['object'])
    checked('/tilegrid.json', manifest['tilegrid']['object'])
    original = {(frame, bit) for frame, (_, bits) in data.items()
                for bit in bits if not 1600 <= bit < 1613}
    with open('/frame-map.json', encoding='utf-8') as source:
        frame_map = json.load(source)
    addresses = {frame['address'] for frame in frame_map['frames']['frames']}
    del frame_map
    assert len(addresses) == 5144
    phase('unchanged-disassembler')
    database = Database('/db', 'xc7z010clg400-1')
    disassembler = FasmDisassembler(database)
    raw = list(disassembler.find_features_in_bitstream(data, verbose=True))
    raw_names = names(raw)
    assert len(raw_names) <= 1_000_000
    unknown_ecc, unknown = [], []
    annotations = []
    for line in raw:
        for annotation in line.annotations or ():
            if annotation.name == 'unknown_bit':
                frame, word, bit = annotation.value.split('_')
                position = (int(frame, 16), int(word) * 32 + int(bit))
                (unknown_ecc if 1600 <= position[1] < 1613 else unknown).append(position)
            elif annotation.name not in ('unknown_segment', 'unknown_segbit'):
                annotations.append({'name': annotation.name, 'value': annotation.value})
    phase('unchanged-fasm-format')
    merged = list(merge_and_sort(raw, zero_function=disassembler.is_zero_feature,
                                sort_key=database.grid().tile_key))
    merged_names = names(merged)
    assert merged_names <= raw_names
    formatted_bytes = sum(formatted_line(item) for line in merged
                          for item in fasm.fasm_line_to_string(line))
    text = fasm.fasm_tuple_to_string(merged)
    encoded = text.encode('utf-8')
    assert len(encoded) == formatted_bytes and len(encoded) <= 32 << 20
    text_sha256 = hashlib.sha256(encoded).hexdigest()
    del encoded
    with open('/tmp/selected.fasm', 'w', encoding='utf-8') as target:
        target.write(text)
    decode_warnings = sorted(disassembler.decode_warnings)
    del raw, merged, data, disassembler
    gc.collect()
    phase('unchanged-fasm-parser-and-assembler')
    roundtrip = {'passed': False, 'error': None, 'error_trace': None}
    assembler = frames = regenerated = missing = extra = None
    try:
        assembler = FasmAssembler(database)
        parsed_names = set()

        def parsed_feature(feature):
            # This callback is part of the unchanged assembler API. It sees
            # each feature read from the actual formatted file in this pass.
            for flat in fasm.canonical_features(feature):
                parsed_names.add(fasm.set_feature_to_str(flat, check_if_canonical=True))

        assembler.set_feature_callback(parsed_feature)
        missing_features = []
        parser = parse_saved_chunks('/tmp/selected.fasm', 256 << 10, text_sha256,
                                    lambda line: assembler.add_fasm_line(line, missing_features))
        if missing_features:
            raise FasmLookupError('\n'.join(missing_features))
        phase('unchanged-frame-generation')
        frames = assembler.get_frames(sparse=False)
        extra_addresses = sorted(set(frames) - addresses)
        assert not extra_addresses, 'assembler frame outside original part map'
        phase('independent-frame-set-comparison')
        regenerated = {(frame, word_index * 32 + bit)
                       for frame, words in frames.items()
                       for word_index, word in enumerate(words)
                       for bit in range(32)
                       if word & (1 << bit) and not (word_index == 50 and bit < 13)}
        missing = original - regenerated
        extra = regenerated - original
        # Callback names came from the saved text through the unchanged parser.
        roundtrip.update({'passed': not missing and not extra and parsed_names == merged_names,
                          'original_set_bits': len(original),
                          'regenerated_set_bits': len(regenerated),
                          'original_bits_sha256': bit_digest(original),
                          'regenerated_bits_sha256': bit_digest(regenerated),
                          'missing_count': len(missing), 'extra_count': len(extra),
                          'missing_first_256': sorted(missing)[:256],
                          'extra_first_256': sorted(extra)[:256],
                          'parser': parser,
                          'parsed_names_sha256': name_digest(parsed_names),
                          'parsed_name_count': len(parsed_names)})
    except Exception as error:
        roundtrip['error'] = type(error).__name__ + ': ' + str(error)
        trace, frame = [], error.__traceback__
        while frame is not None and len(trace) < 16:
            trace.append({'file': frame.tb_frame.f_code.co_filename,
                          'line': frame.tb_lineno,
                          'function': frame.tb_frame.f_code.co_name})
            frame = frame.tb_next
        roundtrip['error_trace'] = trace
        del frame
    del assembler, frames, regenerated, missing, extra, original, database
    gc.collect()
    phase('retained-output')
    result = {'schema': 'md-sim-prjxray-fasm-export/1',
              'passed': roundtrip['passed'] and not unknown,
              'part': 'xc7z010clg400-1', 'frame_count': len(addresses),
              'configuration_sha256': configuration_sha256,
              'raw_names': sorted(raw_names),
              'removed_default_names': sorted(raw_names - merged_names),
              'merged_names_sha256': name_digest(merged_names),
              'merged_name_count': len(merged_names),
              'unknown_ecc_bits': len(unknown_ecc),
              'unknown_non_ecc': sorted(unknown), 'annotations': annotations,
              'decode_warnings': decode_warnings,
              'fasm': text, 'fasm_sha256': text_sha256,
              'roundtrip': roundtrip,
              'parser_fixture': fixture,
              'parser_mode': 'bounded-complete-formatter-lines-v1',
              'tool_adaptations': tools['adaptations'],
              'harness_adaptation': 'Unchanged parse_fasm_string and assembler.add_fasm_line on bounded complete formatter lines; multiline formatter items are rejected before parsing. This avoids the whole-file textX syntax-tree memory peak.',
              'scope': 'Static FASM export and non-ECC bit round trip only. Missing tables remain explicit. No netlist, physical timing or firmware execution.'}
    json.dump(result, sys.stdout, separators=(',', ':'))
    print(flush=True)
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
