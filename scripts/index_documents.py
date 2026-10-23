#!/usr/bin/env python3
"""Index saved text and English companions for Bitcoin ASIC evidence. Do not run artifacts."""
import datetime,hashlib,json,sqlite3
from pathlib import Path
import catalog
R=catalog.ROOT

def main():
 data,_=catalog.split_scope(catalog.load_parts());artifacts={a['id']:a for a in data['artifacts'] if a.get('status')=='downloaded'};candidates=[]
 for a in artifacts.values():
  inspection=a.get('inspection',{});path=inspection.get('text_path')
  if path:candidates.append((a,path,'extracted-original-text','See the source language and extraction limits.'))
 for tr in catalog.rows(R/'translations/register.jsonl'):
  for aid in catalog.translation_artifact_ids(tr):
   a=artifacts.get(aid)
   if not a:continue
   if tr.get('source_sha256') and tr['source_sha256']!=a['sha256']:continue
   path=tr.get('text_path') or tr.get('translation_path')
   if path and Path(path).suffix.lower() in ['.txt','.md']:candidates.append((a,path,'english-companion',tr.get('scope','See translation register.')))
 target=R/'catalog/documents.sqlite';temp=target.with_suffix('.sqlite.tmp');temp.unlink(missing_ok=True);db=sqlite3.connect(temp)
 db.executescript('CREATE TABLE documents(id INTEGER PRIMARY KEY,artifact_id TEXT,parent_sha256 TEXT,local_path TEXT,text_path TEXT,text_sha256 TEXT,text_role TEXT,coverage TEXT,title TEXT); CREATE VIRTUAL TABLE document_search USING fts5(title,body,tokenize="unicode61");')
 seen=set();count=0;chars=0;skipped=[]
 for a,path,role,coverage in candidates:
  if (a['id'],path) in seen:continue
  seen.add((a['id'],path));p=R/path
  if not p.is_file():skipped.append(dict(artifact_id=a['id'],path=path,reason='Missing extracted text'));continue
  if not p.resolve().is_relative_to(R.resolve()):raise ValueError('Text path leaves workspace: '+path)
  b=p.read_bytes();text=b.decode('utf-8',errors='replace');count+=1;chars+=len(text)
  db.execute('INSERT INTO documents VALUES (?,?,?,?,?,?,?,?,?)',(count,a['id'],a['sha256'],a['local_path'],path,hashlib.sha256(b).hexdigest(),role,coverage,a['title']))
  db.execute('INSERT INTO document_search(rowid,title,body) VALUES (?,?,?)',(count,a['title'],text))
 db.commit();check=db.execute('PRAGMA integrity_check').fetchone()[0];db.close()
 if check!='ok':raise RuntimeError(check)
 temp.replace(target);report=dict(generated_at=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds'),text_records=count,indexed_characters=chars,skipped=skipped,integrity_check=check,scope='Extracted text and registered English text companions for downloaded main-scope artifacts. Image-only labels and unextracted archive members are not included.')
 (R/'reports/document-index.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
