import hashlib,json,sqlite3,sys,tempfile,unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import catalog

class CatalogueTests(unittest.TestCase):
 def test_scope_does_not_infer_sha256_from_name(self):
  d={k:[] for k in ['entities','relationships','sources','gaps','artifacts','discovery']}
  d['entities']=[{'id':'btc','scope':'bitcoin-asic','source_ids':['shared']},{'id':'gpu','scope':'supplementary-out-of-scope','source_ids':['shared']},{'id':'unverified-sha256'}]
  d['sources']=[{'id':'shared','artifact_ids':['mix','gpu-only']}]
  d['artifacts']=[{'id':'mix'},{'id':'gpu-only'}]
  d['relationships']=[{'id':'good','from_id':'btc','to_id':'btc'},{'id':'bad','from_id':'btc','to_id':'gpu'}]
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);p=root/'catalog/parts/test';p.mkdir(parents=True)
   (p/'artifact-scope.jsonl').write_text(json.dumps({'artifact_id':'gpu-only','scope':'supplementary-out-of-scope'})+'\n')
   main,supp=catalog.split_scope(d,root)
  self.assertEqual([r['id'] for r in main['entities']],['btc'])
  self.assertEqual([r['id'] for r in main['relationships']],['good'])
  self.assertEqual(main['sources'][0]['artifact_ids'],['mix'])
  self.assertEqual({r['id'] for r in supp['entities']},{'gpu','unverified-sha256'})
 def test_missing_edges_are_unknown_and_claims_do_not_chain(self):
  edges=[{'id':'one','from_id':'fw','to_id':'board','relation':'supports','status':'vendor-claimed'}, {'id':'two','from_id':'device','to_id':'board','relation':'uses-control-board','status':'documented'}]
  self.assertEqual(catalog.assess_pair(edges,'fw','device')['result'],'unknown')
  self.assertEqual(catalog.assess_pair(edges,'fw','board')['result'],'support-claim')
  edges.append({'id':'three','from_id':'fw','to_id':'board','relation':'incompatible-with','status':'documented'})
  self.assertEqual(catalog.assess_pair(edges,'fw','board')['result'],'conflicting-claims')
  labels=[{'from_id':'fw','to_id':'board','relation':'supports','status':'archive-label-only'}]
  self.assertEqual(catalog.assess_pair(labels,'fw','board')['result'],'target-label-only')
 def test_file_identity_and_path_boundaries(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);(root/'image.bin').write_bytes(b'first')
   row={'id':'a','status':'downloaded','local_path':'image.bin','size_bytes':5,'sha256':hashlib.sha256(b'first').hexdigest()}
   self.assertEqual(catalog.validate({'artifacts':[row]},root,True),[])
   (root/'image.bin').write_bytes(b'other')
   self.assertIn('SHA-256 mismatch',' '.join(catalog.validate({'artifacts':[row]},root,True)))
   row['local_path']='../outside'
   self.assertIn('path leaves',' '.join(catalog.validate({'artifacts':[row]},root)))
 def test_references_fail_when_missing(self):
  d={'entities':[{'id':'e','kind':'device','name':'x','manufacturer':'x','source_ids':['missing'],'artifact_ids':['not-downloaded']}], 'sources':[], 'artifacts':[]}
  errors=catalog.validate(d)
  self.assertEqual(len(errors),2)
 def test_registered_external_storage(self):
  with tempfile.TemporaryDirectory() as temp, tempfile.TemporaryDirectory() as storage:
   root=Path(temp);external=Path(storage);(external/'file.bin').write_bytes(b'archive')
   (root/'file.bin').symlink_to(external/'file.bin')
   row={'id':'a','status':'downloaded','local_path':'file.bin','size_bytes':7,'sha256':hashlib.sha256(b'archive').hexdigest()}
   self.assertIn('path leaves',' '.join(catalog.validate({'artifacts':[row]},root)))
   (root/'catalog').mkdir();(root/'catalog/storage.json').write_text(json.dumps({'artifact_roots':[str(external)]}))
   self.assertEqual(catalog.validate({'artifacts':[row]},root,True),[])
 def test_generated_database_is_consistent(self):
  p=catalog.ROOT/'catalog/catalog.sqlite'
  if not p.exists():self.skipTest('Build the catalogue first.')
  con=sqlite3.connect(f'file:{p}?mode=ro',uri=True)
  self.assertEqual(con.execute('PRAGMA integrity_check').fetchone()[0],'ok')
  self.assertEqual(con.execute('SELECT COUNT(*) FROM relationships r LEFT JOIN entities a ON a.id=r.from_id LEFT JOIN entities b ON b.id=r.to_id WHERE a.id IS NULL OR b.id IS NULL').fetchone()[0],0)
  self.assertGreater(con.execute("SELECT COUNT(*) FROM search_index WHERE search_index MATCH 'Antminer'").fetchone()[0],0)
 def test_browser_embeds_only_json_and_has_no_remote_assets(self):
  p=catalog.ROOT/'catalog/index.html'
  if not p.exists():self.skipTest('Build the catalogue first.')
  t=p.read_text();raw=t.split('const DATA=',1)[1].split(';\nconst groups=',1)[0]
  d=json.loads(raw)
  self.assertNotIn('</script',raw.lower())
  self.assertNotIn('<script src=',t.lower())
  self.assertTrue(all(r['scope'] in ['bitcoin-asic','supporting-tool','mixed-reference'] for r in d['entities']))

if __name__=='__main__':unittest.main()
