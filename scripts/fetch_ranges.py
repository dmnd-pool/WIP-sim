#!/usr/bin/env python3
"""Download a public file in checked HTTP ranges. Never execute the payload."""
import concurrent.futures,fcntl,hashlib,json,os,re,urllib.request,time,http.client
from pathlib import Path
from fetch_artifact import ROOT,utcnow

def fetch_ranges(url,artifact_id,expected_size,title='',kind='firmware',source_id=None,expected_sha256=None,workers=8,chunk_bytes=8*1024**2):
 if not re.fullmatch(r'[a-z0-9][a-z0-9._-]*',artifact_id):raise ValueError('Invalid artifact ID')
 locks=ROOT/'work/download-locks';locks.mkdir(parents=True,exist_ok=True)
 with (locks/(artifact_id+'.lock')).open('a') as lock:
  fcntl.flock(lock,fcntl.LOCK_EX)
  return _run(url,artifact_id,expected_size,title,kind,source_id,expected_sha256,workers,chunk_bytes)

def _run(url,aid,total,title,kind,sid,sha,workers,chunk_bytes):
 from urllib.parse import urlparse,unquote
 mp=ROOT/'artifacts/manifests'/(aid+'.json')
 prior=json.loads(mp.read_text()) if mp.exists() else {}
 if prior.get('status')=='downloaded' and (ROOT/prior['local_path']).stat().st_size==prior['size_bytes']:return prior
 if not 0<total<=2*1024**3:raise ValueError('Unexpected file size')
 name=re.sub(r'[^A-Za-z0-9._+-]','_',unquote(Path(urlparse(url).path).name))[:180]
 # Tests can replace ROOT. Production uses the same storage allocator as sequential downloads.
 from fetch_artifact import artifact_directory,ROOT as FETCH_ROOT
 directory=artifact_directory(kind,aid) if ROOT==FETCH_ROOT else ROOT/'artifacts/files'/kind/aid
 directory.mkdir(parents=True,exist_ok=True);path=directory/name
 chunks=ROOT/'work/download-ranges'/aid;chunks.mkdir(parents=True,exist_ok=True)
 volatile={'status','local_path','sha256','size_bytes','error','http_status','final_url','content_type','last_modified','etag','retrieved_at','payload_available','content_validity','retry_after'}
 record={k:v for k,v in prior.items() if k not in volatile and not k.startswith('matches_')}
 record.update(id=aid,url=url,kind=kind,title=title or name,language=prior.get('language','und'),source_id=sid,license=prior.get('license','unknown'),retrieved_at=utcnow(),status='failed',local_path=None,sha256=None,size_bytes=None,executed=False,expected_size_bytes=total,download_method='Verified HTTP byte ranges')
 try:
  headers={'User-Agent':'MiningDeviceCatalogue/1.0 (public research archive)','Range':'bytes=0-0','Accept-Encoding':'identity'}
  with urllib.request.urlopen(urllib.request.Request(url,headers=headers),timeout=60) as r:
   if r.status!=206 or r.headers.get('Content-Range')!=f'bytes 0-0/{total}' or len(r.read())!=1:raise ValueError('Server did not confirm the requested byte range and total size')
   etag=r.headers.get('ETag');record.update(final_url=r.url,http_status=206,etag=etag,content_type=r.headers.get('Content-Type'),last_modified=r.headers.get('Last-Modified'))
  state={'url':url,'etag':etag,'total':total,'chunk_bytes':chunk_bytes,'expected_sha256':sha}
  statefile=chunks/'state.json'
  if statefile.exists() and json.loads(statefile.read_text())!=state:raise ValueError('Range state changed. Review the stored chunks before retrying.')
  statefile.write_text(json.dumps(state,indent=2)+'\n')
  # Reuse the previous sequential prefix only when a publisher SHA-256 will verify the complete result.
  prefix=path.with_suffix(path.suffix+'.part');prefix_size=prefix.stat().st_size if sha and prefix.exists() else 0
  if prefix_size>total:raise ValueError('Saved prefix exceeds expected file size')
  offsets=[(start,min(start+chunk_bytes,total)-1) for start in range(prefix_size,total,chunk_bytes)]
  def piece(bounds):
   start,end=bounds;p=chunks/f'{start}-{end}.chunk';wanted=end-start+1
   if p.exists() and p.stat().st_size==wanted:return p
   reqh={'User-Agent':headers['User-Agent'],'Range':f'bytes={start}-{end}','Accept-Encoding':'identity'}
   if etag:reqh['If-Match']=etag
   temp=p.with_suffix('.partial')
   with urllib.request.urlopen(urllib.request.Request(url,headers=reqh),timeout=90) as r:
    if r.status!=206 or r.headers.get('Content-Range')!=f'bytes {start}-{end}/{total}':raise ValueError('Wrong byte range returned')
    if etag and r.headers.get('ETag')!=etag:raise ValueError('ETag changed during transfer')
    count=0
    with temp.open('wb') as out:
     while b:=r.read(1024**2):
      count+=len(b)
      if count>wanted:raise ValueError('Range exceeded requested size')
      out.write(b)
    if count!=wanted:raise EOFError('Range ended before expected size')
   temp.replace(p);return p
  def retry_piece(bounds):
   for attempt in range(3):
    try:return piece(bounds)
    except (OSError,EOFError,http.client.HTTPException) as exc:
     if getattr(exc,'code',None) in [401,403,404,429]:raise
     if attempt==2:raise
     time.sleep(1+attempt)
  with concurrent.futures.ThreadPoolExecutor(workers) as pool:parts=list(pool.map(retry_piece,offsets))
  assembled=path.with_suffix(path.suffix+'.assemble');digest=hashlib.sha256();written=0
  with assembled.open('wb') as out:
   inputs=([prefix] if prefix_size else [])+parts
   for p in inputs:
    with p.open('rb') as inp:
     while b:=inp.read(1024**2):out.write(b);digest.update(b);written+=len(b)
  measured=digest.hexdigest()
  if written!=total:raise ValueError('Assembled size mismatch')
  if sha and measured!=sha:raise ValueError('Publisher SHA-256 mismatch after range assembly')
  assembled.replace(path)
  record.update(status='downloaded',local_path=str(path.relative_to(ROOT)),sha256=measured,size_bytes=written,publisher_sha256=sha,matches_publisher_sha256=True if sha else None,reused_prefix_bytes=prefix_size)
  # Keep only completed payloads and their manifests. Incomplete chunks remain after failures.
  for p in parts:p.unlink()
  if prefix.exists():prefix.unlink()
 except Exception as exc:
  record['error']=str(exc)
  if isinstance(exc,urllib.error.HTTPError):
   record['http_status']=exc.code;record['retry_after']=exc.headers.get('Retry-After')
 temp=mp.with_suffix('.ranges-tmp');temp.write_text(json.dumps(record,indent=2)+'\n');temp.replace(mp)
 return record
