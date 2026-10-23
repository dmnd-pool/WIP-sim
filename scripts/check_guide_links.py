#!/usr/bin/env python3
"""Check local navigation links in the collection's guides. Does not request external URLs."""
import json,re,urllib.parse
from pathlib import Path
import catalog
R=catalog.ROOT

def main():
 data,_=catalog.split_scope(catalog.load_parts());ids={k:{r['id'] for r in v} for k,v in data.items()};checked=0;broken=[]
 paths=[R/'README.md',R/'translations/README.md',*sorted((R/'docs').rglob('*.md'))]
 for p in paths:
  text=p.read_text();text=re.sub(r'```[^\n]*\n.*?```','',text,flags=re.S)
  for match in re.finditer(r'\]\(([^\s)]+)(?:\s+"[^"]*")?\)',text):
   raw=match[1].strip('<>');u=urllib.parse.urlsplit(raw)
   if u.scheme or u.netloc or not u.path:continue
   path=(p.parent/urllib.parse.unquote(u.path)).resolve();checked+=1;problem=None
   if not path.exists():problem='Local target does not exist'
   elif path==R/'catalog/index.html' and '/' in u.fragment:
    group,ident=u.fragment.split('/',1)
    if group in ids and urllib.parse.unquote(ident) not in ids[group]:problem='Catalogue record does not exist'
   if problem:broken.append(dict(guide=str(p.relative_to(R)),target=raw,problem=problem))
 report=dict(checked_links=checked,broken=broken,result='fail' if broken else 'pass',scope='Local links in README, translation guide, and docs. External URLs and links inside code blocks are excluded.')
 (R/'reports/guide-links.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report));raise SystemExit(bool(broken))
if __name__=='__main__':main()
