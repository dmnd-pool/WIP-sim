#!/usr/bin/env python3
"""Check saved file hashes once per physical path. Reuse prior checks only on request."""
import argparse,concurrent.futures,datetime,hashlib,json
from pathlib import Path
import catalog
R=catalog.ROOT

def now():return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds')
def fingerprint(p):
 s=p.stat();return {'size_bytes':s.st_size,'mtime_ns':s.st_mtime_ns,'ctime_ns':s.st_ctime_ns,'inode':s.st_ino,'device':s.st_dev}
def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--incremental',action='store_true');ap.add_argument('--workers',type=int,default=4);args=ap.parse_args()
 d,supp=catalog.split_scope(catalog.load_parts());errors=catalog.validate(d)+catalog.validate({'artifacts':supp['artifacts']});ms=d['artifacts']+supp['artifacts']
 if errors:
  report=dict(checked_at=now(),result='fail',structural_errors=errors);(R/'reports/archive-validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));raise SystemExit(1)
 groups={}
 for m in ms:
  if m.get('status')=='downloaded':groups.setdefault(str((R/m['local_path']).resolve()),[]).append(m)
 cache_path=R/'reports/file-hashes.jsonl';cache={r['path']:r for r in catalog.rows(cache_path)} if args.incremental else {};results=[]
 def check(item):
  name,records=item;p=Path(name);before=fingerprint(p);expected={m['sha256'] for m in records};old=cache.get(name)
  row=dict(path=name,artifact_ids=[m['id'] for m in records],expected_sha256=sorted(expected),fingerprint=before,checked_at=now(),reused=False)
  if old and old.get('fingerprint')==before and old.get('sha256') in expected and len(expected)==1:
   row.update(sha256=old['sha256'],checked_at=old['checked_at'],reused=True)
  else:
   h=hashlib.sha256()
   with p.open('rb') as f:
    while b:=f.read(4*1024**2):h.update(b)
   row['sha256']=h.hexdigest()
  row['matches_manifest']=len(expected)==1 and row['sha256'] in expected;row['stable_during_check']=fingerprint(p)==before
  return row
 with concurrent.futures.ThreadPoolExecutor(max(1,min(args.workers,16))) as ex:
  for i,row in enumerate(ex.map(check,groups.items()),1):
   results.append(row)
   if i%500==0:print(json.dumps({'checked_paths':i,'total_paths':len(groups)}),flush=True)
 cache_path.write_text(''.join(json.dumps(r)+'\n' for r in results));bad=[r for r in results if not r['matches_manifest'] or not r['stable_during_check']]
 report=dict(checked_at=now(),result='fail' if bad else 'pass',scope='Downloaded main and retained supplementary artifacts; queued and failed transfers are excluded.',downloaded_records=sum(len(x) for x in groups.values()),unique_physical_paths=len(groups),paths_read=sum(not r['reused'] for r in results),prior_checks_reused=sum(r['reused'] for r in results),bytes_read=sum(r['fingerprint']['size_bytes'] for r in results if not r['reused']),errors=bad,cache_rule='A prior hash is reused only when path, size, mtime, ctime, inode, device and expected hash agree. Use no --incremental flag for a fresh read.')
 (R/'reports/archive-validation.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report),flush=True);raise SystemExit(bool(bad))
if __name__=='__main__':main()
