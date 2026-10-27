"""Invoke the pinned converter in confinement, with recorded API adaptations.

No cell, connection, feature, missing sink or timing value is synthesized here.
Rust owns input identities, limits, output retention and final comparison.
"""
import contextlib
import gc
import hashlib
import json
import os
import sys
import time

with open('/netlist-inputs.json', encoding='utf-8') as source:
    inputs = json.load(source)
with open('/tool-inputs.json', encoding='utf-8') as source:
    tools = json.load(source)
sys.path[:0] = ['/converter', '/upstream', '/parser', '/yaml-package', '/support'] + [
    item['guest_path'] for item in tools['inputs']
    if item['guest_path'].startswith('/wheels/')]


def phase(name):
    print(json.dumps({'phase': name, 'monotonic_seconds': time.monotonic()}),
          file=sys.stderr, flush=True)


def file_identity(path, maximum):
    size = os.stat(path).st_size
    assert size <= maximum, 'output file bound'
    digest = hashlib.sha256()
    with open(path, 'rb') as source:
        for block in iter(lambda: source.read(1 << 20), b''):
            digest.update(block)
    return {'sha256': digest.hexdigest(), 'size': size}


def failure(error):
    records = []
    trace = error.__traceback__
    while trace is not None and len(records) < 16:
        records.append({'file': trace.tb_frame.f_code.co_filename,
                        'line': trace.tb_lineno,
                        'function': trace.tb_frame.f_code.co_name})
        trace = trace.tb_next
    return {'type': type(error).__name__, 'message': str(error)[:4096],
            'trace': records}


def graph_summary():
    path = '/tmp/connections.db'
    if not os.path.exists(path):
        return None
    import sqlite3
    result = file_identity(path, 1 << 30)
    with sqlite3.connect('file:' + path + '?mode=ro', uri=True) as connection:
        result['quick_check'] = [row[0] for row in connection.execute('PRAGMA quick_check')]
        expected = ['tile_type', 'site_type', 'phy_tile', 'site_pin', 'site',
                    'site_instance', 'wire_in_tile', 'node', 'wire']
        actual = {row[0] for row in connection.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table'")}
        assert actual == set(expected), 'connection database schema differs'
        result['counts'] = {table: connection.execute(
            'SELECT count(*) FROM ' + table).fetchone()[0] for table in expected}
        result['unassigned_wires'] = connection.execute(
            'SELECT count(*) FROM wire WHERE node_pkey IS NULL').fetchone()[0]
        result['missing_node_references'] = connection.execute(
            'SELECT count(*) FROM wire LEFT JOIN node ON wire.node_pkey = node.pkey '
            'WHERE node.pkey IS NULL').fetchone()[0]
        result['missing_tile_references'] = connection.execute(
            'SELECT count(*) FROM wire LEFT JOIN phy_tile ON wire.phy_tile_pkey = phy_tile.pkey '
            'WHERE phy_tile.pkey IS NULL').fetchone()[0]
    return result


def main():
    result = {'schema': 'md-sim-fasm-netlist-conversion/1', 'converted': False,
              'error': None, 'graph_error': None, 'graph': None, 'files': {},
              'artifact_errors': {},
              'parser': None, 'parser_fixture': None, 'parser_calls': 0,
              'parser_exhausted': False, 'tool_adaptations': tools['adaptations'],
              'converter_adaptation': inputs['adaptation'],
              'parser_adaptation': inputs['parser_adaptation'],
              'scope': inputs['scope']}
    try:
        with contextlib.redirect_stdout(sys.stderr):
            phase('import-pinned-converter')
            import fasm
            from prjxray_fasm_export import parse_saved_chunks, parser_fixture, names, name_digest
            from fasm2bels import fasm2bels
            expected = inputs['expected']
            assert file_identity('/selected.fasm', 16 << 20) == inputs['export']['fasm']['object']
            with open('/export-assessment.json', encoding='utf-8') as source:
                assessment = json.load(source)
            assert assessment['passed'] and assessment['details']['roundtrip_passed']
            assert assessment['details']['fasm'] == inputs['export']['fasm']['object']
            result['parser_fixture'] = parser_fixture()

            def checked_filename(path):
                assert path == '/selected.fasm' and result['parser_calls'] == 0
                result['parser_calls'] += 1
                phase('bounded-verified-fasm-parser')
                parsed = []
                result['parser'] = parse_saved_chunks(path, 256 << 10,
                    inputs['export']['fasm']['object']['sha256'], parsed.append)
                canonical = names(parsed)
                assert len(canonical) == expected['formatted_features']
                assert name_digest(canonical) == expected['formatted_names_sha256']
                result['parsed_names_sha256'] = name_digest(canonical)
                result['parsed_names_count'] = len(canonical)
                del canonical
                gc.collect()
                phase('unchanged-converter-features-and-routes')
                yield from parsed
                result['parser_exhausted'] = True
                parsed.clear()
                gc.collect()

            original_parser = fasm.parse_fasm_filename
            fasm.parse_fasm_filename = checked_filename
            sys.argv = ['fasm2bels', '--db_root', '/db', '--part', expected['part'],
                        '--connection_database', '/tmp/connections.db',
                        '--fasm_file', '/selected.fasm', '--top', 'top',
                        '--verilog_file', '/tmp/selected.v', '--xdc_file', '/tmp/selected.xdc']
            result['converter_argv'] = sys.argv.copy()
            phase('unchanged-connection-database-and-converter')
            try:
                fasm2bels.main()
            finally:
                fasm.parse_fasm_filename = original_parser
            assert result['parser_calls'] == 1 and result['parser_exhausted']
            result['converted'] = True
    except Exception as error:
        result['error'] = failure(error)
    phase('retain-conversion-artifacts')
    try:
        result['graph'] = graph_summary()
    except Exception as error:
        result['graph_error'] = failure(error)
    for name in ['selected.v', 'selected.xdc']:
        path = '/tmp/' + name
        if os.path.exists(path):
            try:
                identity = file_identity(path, 32 << 20)
                with open(path, encoding='utf-8') as source:
                    text = source.read()
                assert len(text.encode()) == identity['size']
                result['files'][name] = {'object': identity, 'text': text}
            except Exception as error:
                result['artifact_errors'][name] = failure(error)
                result['converted'] = False
    json.dump(result, sys.stdout, separators=(',', ':'))
    print(flush=True)
    return 0 if result['converted'] and result['graph_error'] is None else 1


if __name__ == '__main__':
    sys.exit(main())
