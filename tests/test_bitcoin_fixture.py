import copy,json,sys,unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from verify_bitcoin_fixture import ROOT,compact_target,verify
class BitcoinFixtureTests(unittest.TestCase):
 def test_primary_source_genesis_fixture(self):
  f=json.loads((ROOT/'fixtures/bitcoin/genesis-header.json').read_text());self.assertTrue(verify(f)['proof_of_work_valid'])
 def test_nonce_change_does_not_pass_known_hash(self):
  f=json.loads((ROOT/'fixtures/bitcoin/genesis-header.json').read_text());f['nonce']+=1
  with self.assertRaisesRegex(ValueError,'fields'):verify(f)
 def test_invalid_compact_targets(self):
  for bits in [0,0x1d80ffff,0x23010000]:
   with self.assertRaises(ValueError):compact_target(bits)
if __name__=='__main__':unittest.main()
