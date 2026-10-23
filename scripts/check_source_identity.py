#!/usr/bin/env python3
"""Compare source archive commit markers with saved Git commit claims."""
import json,re,tarfile,zipfile,concurrent.futures
from pathlib import Path
from fetch_artifact import ROOT

def check(m):
 r={'artifact_id':m['id'],'sha256':m['sha256'],'claimed_commit':m.get('git_commit'),'status':'no-commit-claim','executed':False}
 if m.get('acquisition_method')=='byte-range-extracted-from-downloaded-file':
  r.update(status='parent-file-byte-range',parent_artifact_id=m.get('parent_artifact_id'),parent_sha256=m.get('parent_sha256'),byte_offset=m.get('byte_offset'),size_bytes=m.get('size_bytes'),note='The source bytes were extracted from this parent file. They do not have a separate Git commit.')
  return r
 if m.get('acquisition_method')=='locally-assembled-verified-git-tree':
  r.update(status='locally-assembled-git-tree',source_commit=m.get('source_commit'),git_tree=m.get('git_tree'),note='This is a local reconstruction from verified Git blobs. It is not a publisher archive; use the per-file Git identity report.')
  return r
 if m.get('acquisition_method')=='contained-in-downloaded-archive':
  r.update(status='parent-archive-member',parent_artifact_id=m.get('parent_artifact_id'),parent_sha256=m.get('parent_sha256'),archive_member=m.get('archive_member'),note='The Git commit identifies the parent repository snapshot. It is not an embedded commit marker in this member.')
  return r
 if not re.fullmatch('[0-9a-f]{40}',m.get('git_commit') or ''):return r
 p=ROOT/m['local_path']
 try:
  with p.open('rb') as f:head=f.read(4)
  if head.startswith(b'PK'):
   with zipfile.ZipFile(p) as z:marker=z.comment.decode(errors='replace').strip()
  else:
   with tarfile.open(p,'r|*') as t:
    first=t.next();marker=t.pax_headers.get('comment','');r['first_member']=first.name if first else None
  r['archive_commit_marker']=marker
  r['status']='commit-match' if marker==m['git_commit'] else 'commit-mismatch' if re.fullmatch('[0-9a-f]{40}',marker) else 'marker-not-found'
 except Exception as e:r.update(status='inspection-error',error=str(e))
 return r
if __name__=='__main__':
 ms=[json.loads(p.read_text()) for p in (ROOT/'artifacts/manifests').glob('*.json')];ms=[m for m in ms if m.get('status')=='downloaded' and m.get('kind')=='source']
 with concurrent.futures.ThreadPoolExecutor(4) as ex:rows=list(ex.map(check,ms))
 (ROOT/'reports/source-identity.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in rows))
 from collections import Counter
 print(json.dumps(dict(Counter(r['status'] for r in rows))))
 raise SystemExit(1 if any(r['status']=='commit-mismatch' for r in rows) else 0)
