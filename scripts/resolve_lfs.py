#!/usr/bin/env python3
"""Identify LFS references and link verified local objects. Download missing public objects on request."""
import argparse,concurrent.futures,json,re
from pathlib import Path
from fetch_artifact import ROOT,fetch
from collect_firmware_archive import bitcoin_candidate

def main():
 p=argparse.ArgumentParser();p.add_argument('--download',action='store_true');p.add_argument('--defer-innosilicon',action='store_true');args=p.parse_args()
 manifests=[json.loads(p.read_text()) for p in (ROOT/'artifacts/manifests').glob('*.json')]
 byhash={m['sha256']:m for m in manifests if m.get('status')=='downloaded' and m.get('payload_available') is not False and m.get('size_bytes',0)>4096}
 inventory={json.loads(l)['id']:json.loads(l) for l in (ROOT/'catalog/discovery/firmware-archive-files.jsonl').read_text().splitlines()}
 out=[];jobs=[];scopes=[]
 for m in manifests:
  if m.get('status')!='downloaded' or m.get('size_bytes',4096)>=4096:continue
  b=(ROOT/m['local_path']).read_bytes();match=re.fullmatch(rb'version https://git-lfs.github.com/spec/v1\r?\noid sha256:([0-9a-f]{64})\r?\nsize ([0-9]+)\r?\n?',b)
  if not match:continue
  sha=match[1].decode();size=int(match[2]);entry=inventory.get(m['id'],{});ap=entry.get('archive_path','')
  m.update(content_role='git-lfs-pointer',payload_available=False,lfs_object_sha256=sha,lfs_object_size=size,content_note='This file is a Git LFS reference. It is not the firmware or document payload.')
  local=byhash.get(sha)
  if local and local['size_bytes']==size:
   m.update(resolved_artifact_id=local['id'],resolved_object_available=True)
   if ap and bitcoin_candidate(ap):scopes.append(dict(artifact_id=local['id'],scope='bitcoin-asic',reason='Verified object referenced by an in-scope Git LFS pointer.'))
  elif ap and bitcoin_candidate(ap):
   url=entry['url'].replace('https://raw.githubusercontent.com/','https://media.githubusercontent.com/media/')
   aid='lfs-object-'+sha[:24];m['resolution_artifact_id']=aid
   if not (args.defer_innosilicon and ap.lower().startswith('innosilicon/t') and '/tools/' not in ap.lower()):
    jobs.append(dict(url=url,artifact_id=aid,kind='tool' if '/tools/' in ap.lower() else entry['kind'],title=ap,source_id='firmware-archive',language='und'))
   scopes.append(dict(artifact_id=aid,scope='bitcoin-asic',reason='Actual object referenced by an in-scope Git LFS pointer.'))
  (ROOT/'artifacts/manifests'/(m['id']+'.json')).write_text(json.dumps(m,indent=2)+'\n')
  out.append({k:m.get(k) for k in ['id','lfs_object_sha256','lfs_object_size','resolved_artifact_id','resolved_object_available','resolution_artifact_id']})
 (ROOT/'reports/git-lfs.jsonl').write_text(''.join(json.dumps(x)+'\n' for x in out))
 part=ROOT/'catalog/parts/lfs';part.mkdir(exist_ok=True)
 (part/'artifact-scope.jsonl').write_text(''.join(json.dumps(x)+'\n' for x in scopes))
 if args.download:
  def run(j):
   r=fetch(**j);wanted=next(x for x in out if x.get('resolution_artifact_id')==j['artifact_id'])
   if r['status']=='downloaded':
    r['matches_lfs_oid']=r['sha256']==wanted['lfs_object_sha256'];r['matches_lfs_size']=r['size_bytes']==wanted['lfs_object_size']
    if not r['matches_lfs_oid'] or not r['matches_lfs_size']:
     r.update(status='failed',payload_available=False,error='Saved response does not match the Git LFS object identity or size.')
    (ROOT/'artifacts/manifests'/(r['id']+'.json')).write_text(json.dumps(r,indent=2)+'\n')
   return {k:r.get(k) for k in ['id','status','matches_lfs_oid','error']}
  with concurrent.futures.ThreadPoolExecutor(12) as pool:
   for result in pool.map(run,{j['artifact_id']:j for j in jobs}.values()):print(json.dumps(result),flush=True)
 print(json.dumps({'pointer_files':len(out),'resolved_local_objects':sum(bool(x.get('resolved_artifact_id')) for x in out),'eligible_downloads':len(jobs)}))
if __name__=='__main__':main()
