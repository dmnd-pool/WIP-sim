#!/usr/bin/env python3
"""Build a searchable index of outer archive member names. Do not extract or run files."""
import datetime,json,sqlite3
import catalog
R=catalog.ROOT

def main():
 d,_=catalog.split_scope(catalog.load_parts());valid={a['id']:a for a in d['artifacts'] if a.get('status')=='downloaded' and a.get('payload_available') is not False}
 reports=catalog.rows(R/'reports/artifact-inspection.jsonl');dest=R/'catalog/archive-members.sqlite';temp=dest.with_suffix('.sqlite.tmp');temp.unlink(missing_ok=True);con=sqlite3.connect(temp)
 con.executescript('CREATE TABLE archives(artifact_id TEXT PRIMARY KEY,sha256 TEXT,local_path TEXT,indexed_members INTEGER,truncated INTEGER); CREATE TABLE members(artifact_id TEXT,path TEXT,size_bytes INTEGER,member_type TEXT); CREATE INDEX member_artifact ON members(artifact_id); CREATE VIRTUAL TABLE member_search USING fts5(artifact_id UNINDEXED,path);')
 count=0;parents=0;truncated=[]
 for report in reports:
  aid=report['artifact_id'];a=valid.get(aid);rel=report.get('member_index')
  if not a or not rel or a['sha256']!=report.get('sha256'):continue
  p=R/rel
  if not p.is_file():continue
  records=catalog.rows(p);con.execute('INSERT INTO archives VALUES (?,?,?,?,?)',(aid,a['sha256'],a['local_path'],len(records),int(report.get('archive_truncated',False))))
  con.executemany('INSERT INTO members VALUES (?,?,?,?)',[(aid,r['path'],r.get('size'),r.get('type')) for r in records]);con.executemany('INSERT INTO member_search(artifact_id,path) VALUES (?,?)',[(aid,r['path']) for r in records]);count+=len(records);parents+=1
  if report.get('archive_truncated'):truncated.append(aid)
 con.commit();check=con.execute('PRAGMA integrity_check').fetchone()[0];con.close()
 if check!='ok':raise RuntimeError(check)
 temp.replace(dest);report=dict(generated_at=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds'),archives=parents,members=count,truncated_archives=truncated,scope='Outer archive member names only. Nested archives, partition filesystems, and encrypted containers need separate inspection.',integrity_check=check)
 (R/'reports/archive-index.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
