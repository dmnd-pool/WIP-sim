#!/usr/bin/env python3
"""Resume indexed public downloads. Do not execute the downloaded files."""
import argparse,concurrent.futures,json,shutil
from pathlib import Path
import catalog
from fetch_artifact import ROOT,fetch

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--workers',type=int,default=8)
 parser.add_argument('--retry-failed',action='store_true')
 parser.add_argument('--prefix',default='')
 parser.add_argument('--reserve-gib',type=float,default=25)
 args=parser.parse_args()
 if not 1<=args.workers<=96:parser.error('Use 1 to 96 workers.')
 data,_=catalog.split_scope(catalog.load_parts())
 statuses={'queued'}|({'failed'} if args.retry_failed else set())
 jobs=[a for a in data['artifacts'] if a.get('status') in statuses and a['id'].startswith(args.prefix)]
 def run(a):
  # Keep room for the OS and in-progress transfers. The registered archive drive has a separate reserve.
  roots=[ROOT]
  config=ROOT/'catalog/storage.json'
  if config.exists():roots += [Path(p) for p in json.loads(config.read_text()).get('artifact_roots',[])]
  for root in roots:
   if shutil.disk_usage(root).free<args.reserve_gib*1024**3:return {'id':a['id'],'status':'deferred-storage','reason':'Free space is below the requested reserve.'}
  result=fetch(a['url'],a['id'],a['kind'],a.get('title',''),a.get('language','und'),filename=a.get('filename'),source_id=a.get('source_id'),license_name=a.get('license','unknown'),request_json=a.get('request_json'),max_bytes=2*1024**3)
  return {k:result.get(k) for k in ['id','status','size_bytes','error']}
 with concurrent.futures.ThreadPoolExecutor(args.workers) as pool:
  for result in pool.map(run,jobs):print(json.dumps(result),flush=True)
if __name__=='__main__':main()
