#!/usr/bin/env python3
"""Check the archived Bitcoin header fixture. This is not a miner emulator."""
import hashlib,json,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def compact_target(bits):
    exponent=bits>>24;coefficient=bits&0x007fffff
    if bits&0x00800000 or coefficient==0:raise ValueError('Target is negative or zero.')
    target=coefficient>>(8*(3-exponent)) if exponent<=3 else coefficient<<(8*(exponent-3))
    if target<=0 or target.bit_length()>256:raise ValueError('Target is outside the unsigned 256-bit range.')
    return target

def verify(f):
    header=bytes.fromhex(f['header_hex'])
    if len(header)!=80:raise ValueError('A Bitcoin block header has 80 bytes.')
    expected=struct.pack('<I',f['version'])+bytes.fromhex(f['previous_block_hash_display'])[::-1]+bytes.fromhex(f['merkle_root_display'])[::-1]+struct.pack('<III',f['time'],int(f['bits_hex'],16),f['nonce'])
    if header!=expected:raise ValueError('Header fields do not match the serialized bytes.')
    digest=hashlib.sha256(hashlib.sha256(header).digest()).digest()
    if digest[::-1].hex()!=f['expected_hash_display']:raise ValueError('Hash does not match the known result.')
    target=compact_target(int(f['bits_hex'],16));valid=int.from_bytes(digest,'little')<=target
    if not valid:raise ValueError('Hash is above the target.')
    return {'fixture_id':f['id'],'header_bytes':len(header),'hash_display':digest[::-1].hex(),'target_hex':f'{target:064x}','proof_of_work_valid':valid,'scope':'Header fixture only. No ASIC, pool, firmware, or device is simulated.'}
if __name__=='__main__':
    print(json.dumps(verify(json.loads((ROOT/'fixtures/bitcoin/genesis-header.json').read_text())),indent=2))
