import gzip,hashlib,io,sys,tempfile,unittest,zipfile
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import verify_containers

class ContainerChecks(unittest.TestCase):
 def inspect(self,content):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);(root/'sample.bin').write_bytes(content);sha=hashlib.sha256(content).hexdigest()
   with patch.object(verify_containers,'R',root):return verify_containers.check((sha,[dict(id='sample',local_path='sample.bin')]))
 def test_valid_gzip_with_installer_trailer_is_compound(self):
  compressed=gzip.compress(b'firmware data'*100);trailer=b'#!/bin/sh\nexit 0\n';r=self.inspect(compressed+trailer)
  self.assertEqual(r['status'],'stream-valid-trailer-unverified');self.assertEqual(r['gzip_member_bytes'],len(compressed));self.assertEqual(r['trailing_sha256'],hashlib.sha256(trailer).hexdigest());self.assertEqual(r['trailing_role'],'shell-script')
 def test_incomplete_gzip_is_rejected(self):
  self.assertEqual(self.inspect(gzip.compress(b'complete data')[:-4])['status'],'fail')
 def test_gzip_crc_error_is_rejected(self):
  data=bytearray(gzip.compress(b'complete data'));data[-8]^=1
  self.assertEqual(self.inspect(data)['status'],'fail')
 def test_zip_member_crc_error_is_rejected(self):
  buf=io.BytesIO()
  with zipfile.ZipFile(buf,'w',compression=zipfile.ZIP_STORED) as z:z.writestr('payload',b'known-payload')
  data=bytearray(buf.getvalue());data[data.index(b'known-payload')]^=1
  self.assertEqual(self.inspect(data)['status'],'fail')
if __name__=='__main__':unittest.main()
