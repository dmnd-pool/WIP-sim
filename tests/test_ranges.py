import hashlib,json,sys,tempfile,threading,unittest
from pathlib import Path
from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import fetch_ranges
PAYLOAD=b'Bitcoin ASIC catalogue integrity fixture.'*100
class Handler(BaseHTTPRequestHandler):
 def do_GET(self):
  if self.path=='/wrong':
   self.send_response(200);self.send_header('Content-Length',str(len(PAYLOAD)));self.end_headers();self.wfile.write(PAYLOAD);return
  start,end=map(int,self.headers['Range'].removeprefix('bytes=').split('-'))
  body=PAYLOAD[start:end+1];self.send_response(206);self.send_header('Content-Range',f'bytes {start}-{end}/{len(PAYLOAD)}');self.send_header('ETag','"fixture"');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
 def log_message(self,*args):pass
class RangeTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.root=Path(self.temp.name);(self.root/'artifacts/manifests').mkdir(parents=True)
  self.original=fetch_ranges.ROOT;fetch_ranges.ROOT=self.root
  self.server=ThreadingHTTPServer(('127.0.0.1',0),Handler);self.thread=threading.Thread(target=self.server.serve_forever,daemon=True);self.thread.start()
  self.url=f'http://127.0.0.1:{self.server.server_port}'
 def tearDown(self):
  self.server.shutdown();self.server.server_close();self.thread.join();fetch_ranges.ROOT=self.original;self.temp.cleanup()
 def test_ranges_assemble_and_verify(self):
  r=fetch_ranges.fetch_ranges(self.url+'/image.bin','range-test',len(PAYLOAD),expected_sha256=hashlib.sha256(PAYLOAD).hexdigest(),chunk_bytes=211,workers=4)
  self.assertEqual(r['status'],'downloaded');self.assertEqual((self.root/r['local_path']).read_bytes(),PAYLOAD);self.assertTrue(r['matches_publisher_sha256'])
 def test_server_ignoring_range_is_rejected(self):
  r=fetch_ranges.fetch_ranges(self.url+'/wrong','range-test',len(PAYLOAD),chunk_bytes=211)
  self.assertEqual(r['status'],'failed');self.assertIsNone(r['local_path'])
 def test_wrong_publisher_hash_is_rejected(self):
  r=fetch_ranges.fetch_ranges(self.url+'/image.bin','range-test',len(PAYLOAD),expected_sha256='0'*64,chunk_bytes=211)
  self.assertEqual(r['status'],'failed');self.assertIn('SHA-256 mismatch',r['error'])
if __name__=='__main__':unittest.main()
