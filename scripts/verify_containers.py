#!/usr/bin/env python3
"""Read root-collected archive streams and ZIP CRCs without extracting or executing files."""
import argparse,concurrent.futures,gzip,hashlib,json,time,zipfile,zlib
from pathlib import Path
import catalog
R=catalog.ROOT;PREFIXES=('epic-asset-','bixbit-','lfs-object-','auradine-','bitdeer-file-','luxor-linked-','asic-to-','hashcore-toolkit-','mirror-')
LIMIT=8*1024**3

def check(item):
 sha,records=item;m=records[0];p=R/m['local_path'];result=dict(sha256=sha,artifact_ids=[r['id'] for r in records],local_path=m['local_path'],executed=False,status='not-a-checked-container')
 try:
  with p.open('rb') as f:magic=f.read(4)
  if magic.startswith(b'PK\x03\x04'):
   with zipfile.ZipFile(p) as z:
    entries=z.infolist();result.update(format='zip',members=len(entries),declared_uncompressed_bytes=sum(x.file_size for x in entries))
    if result['declared_uncompressed_bytes']>LIMIT:return {**result,'status':'limit-skipped','note':'Declared expansion exceeds 8 GiB.'}
    if any(x.flag_bits&1 for x in entries):return {**result,'status':'encrypted-members','note':'Member CRC checks require decryption; no password or key was used.'}
    total=0
    for member in entries:
     if member.is_dir():continue
     with z.open(member) as f:
      while b:=f.read(1024**2):
       total+=len(b)
       if total>LIMIT:raise ValueError('Expanded stream exceeds 8 GiB limit')
    result.update(status='pass',checked_uncompressed_bytes=total,check='Every readable ZIP member reached EOF and passed its CRC check.')
  elif magic.startswith(b'\x1f\x8b'):
   total=0;decoder=zlib.decompressobj(31);consumed=0;trailer=b''
   with p.open('rb') as f:
    while b:=f.read(1024**2):
     consumed+=len(b);pending=b
     while pending:
      expanded=decoder.decompress(pending,1024**2);total+=len(expanded);pending=decoder.unconsumed_tail
      if total>LIMIT:return {**result,'format':'gzip','status':'limit-skipped','note':'Expanded stream exceeds 8 GiB.'}
      if decoder.eof:break
     if decoder.eof:
      trailer=decoder.unused_data+f.read();consumed-=len(decoder.unused_data);break
   if not decoder.eof:raise EOFError('Gzip stream ended before its CRC and length footer.')
   result.update(format='gzip',status='pass',checked_uncompressed_bytes=total,gzip_member_bytes=consumed,check='First gzip stream reached EOF; CRC and length checks passed. TAR members, if any, have a separate structural index.')
   if trailer.strip(b'\0'):
    result.update(status='stream-valid-trailer-unverified',trailing_bytes=len(trailer),trailing_sha256=hashlib.sha256(trailer).hexdigest(),trailing_header_hex=trailer[:32].hex(),trailing_role='shell-script' if trailer.startswith(b'#!') else 'unidentified',note='The file is a compound container. Its first gzip stream is valid. Following bytes require a separate format review; they are not assumed to be corrupt.')
 except Exception as exc:result.update(status='fail',error=str(exc))
 return result

def main():
 ap=argparse.ArgumentParser();ap.add_argument('--incremental',action='store_true');args=ap.parse_args();d,_=catalog.split_scope(catalog.load_parts());groups={}
 for m in d['artifacts']:
  if m.get('status')=='downloaded' and m.get('payload_available') is not False and m['id'].startswith(PREFIXES):groups.setdefault(m['sha256'],[]).append(m)
 dest=R/'reports/container-integrity.jsonl';cache={r['sha256']:r for r in catalog.rows(dest)} if args.incremental else {};rows=[]
 def run(item):
  sha,records=item
  if sha in cache and cache[sha]['status']!='fail':return {**cache[sha],'artifact_ids':[r['id'] for r in records],'reused':True}
  return {**check(item),'reused':False}
 with concurrent.futures.ThreadPoolExecutor(4) as ex:
  for i,r in enumerate(ex.map(run,groups.items()),1):
   rows.append(r)
   if i%100==0:print(json.dumps({'checked_unique_files':i,'total':len(groups)}),flush=True)
 dest.write_text(''.join(json.dumps(r)+'\n' for r in rows))
 from collections import Counter
 summary=dict(status_counts=dict(Counter(r['status'] for r in rows)),unique_sha256=len(rows),prefixes=list(PREFIXES),scope='Root-collected downloaded main-scope files. Known invalid payloads are excluded. This checks ZIP CRC and gzip EOF, not firmware execution or authenticity.',incremental=args.incremental)
 (R/'reports/container-integrity.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary));raise SystemExit(any(r['status']=='fail' for r in rows))
if __name__=='__main__':main()
