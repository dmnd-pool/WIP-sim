#!/usr/bin/env python3
"""Index unsuccessful and superseded download records without hiding known alternatives."""
import collections,datetime,json,urllib.parse
import catalog
R=catalog.ROOT

def main():
 d,_=catalog.split_scope(catalog.load_parts());byid={a['id']:a for a in d['artifacts']};out=[]
 for m in byid.values():
  invalid=m.get('status')=='downloaded' and m.get('payload_available') is False and m.get('content_role')!='git-lfs-pointer'
  if m.get('status') not in ['failed','queued','superseded'] and not invalid:continue
  code=m.get('http_status');error=m.get('error','');alt=m.get('resolved_artifact_id') or m.get('superseded_by')
  category='pending' if m['status']=='queued' else 'superseded' if m['status']=='superseded' else 'rate-limited' if code==429 else 'denied-or-rate-limited' if code in [401,403] else 'url-unavailable' if code in [404,410] else 'server-error' if code and code>=500 else 'transfer-or-format-error'
  if invalid:category='retained-invalid-content';error=m.get('content_validity') or m.get('content_note') or error
  if alt and byid.get(alt,{}).get('status')=='downloaded':category='saved-alternative-available'
  out.append(dict(artifact_id=m['id'],title=m['title'],kind=m['kind'],status=m['status'],category=category,http_status=code,host=urllib.parse.urlparse(m['url']).hostname,url=m['url'],error=error,retry_after=m.get('retry_after'),alternative_artifact_id=alt,alternative_status=byid.get(alt,{}).get('status'),retrieved_at=m.get('retrieved_at')))
 (R/'reports/download-failures.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in out));catalog.write_csv(R/'reports/download-failures.csv',out,list(out[0]) if out else [])
 summary=dict(generated_at=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds'),status_counts=dict(collections.Counter(r['status'] for r in out)),category_counts=dict(collections.Counter(r['category'] for r in out)),limits=['An HTTP failure describes this collection attempt. It does not prove that every public copy is unavailable.','HTTP 403 can mean several things. Check the saved response and source-specific research notes.','A rate-limited request is not a completed download. Retry only after the service permits it.'])
 (R/'reports/download-failures.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
if __name__=='__main__':main()
