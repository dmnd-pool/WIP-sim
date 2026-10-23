#!/usr/bin/env python3
"""List device-level evidence without inferring compatibility or simulation success."""
import collections,json
import catalog

def main():
 d,_=catalog.split_scope(catalog.load_parts());entities={e['id']:e for e in d['entities']};artifacts={a['id']:a for a in d['artifacts']}
 out=[];edges=d['relationships']
 for e in d['entities']:
  if e['kind']!='device':continue
  row={'id':e['id'],'name':e['name'],'manufacturer':e['manufacturer'],'release_year':e.get('release_year'),'date_basis':e.get('date_basis'),'purpose':e.get('purpose'),'simulation_validation':'not-tested','source_ids':e['source_ids']}
  outgoing=[r for r in edges if r['from_id']==e['id']]
  for relation,key in [('uses-control-board','control_boards'),('uses-hashboard','hashboards'),('uses-chip','chips'),('uses-psu','power_supplies')]:
   row[key]=[{'id':r['to_id'],'name':entities[r['to_id']]['name'],'status':r['status'],'relationship_id':r['id'],'constraints':r.get('constraints',[]),'source_ids':r['source_ids']} for r in outgoing if r['relation']==relation]
  claims=[r for r in edges if r['to_id']==e['id'] and r['relation'] in ['supports','builds-for'] and entities[r['from_id']]['kind']=='firmware']
  row['direct_firmware_claims']=[{'id':r['from_id'],'status':r['status'],'relationship_id':r['id'],'constraints':r.get('constraints',[])} for r in claims]
  aids={aid for r in claims for aid in entities[r['from_id']].get('artifact_ids',[])}
  row['downloaded_candidate_payloads']=sorted(aid for aid in aids if artifacts.get(aid,{}).get('status')=='downloaded' and artifacts[aid].get('kind')=='firmware' and artifacts[aid].get('content_role')!='git-lfs-pointer' and artifacts[aid].get('payload_available') is not False)
  row['unresolved_fields']=[k for k in ['control_boards','hashboards','chips','power_supplies','direct_firmware_claims','downloaded_candidate_payloads'] if not row[k]]
  if not row['release_year']:row['unresolved_fields'].append('release_year')
  row['required_before_simulation']=['Select an exact board revision and firmware hash.','Verify boot files, peripheral interfaces, and ASIC work protocol.','Implement and validate the selected execution mode.']
  row['notes']=['Only direct graph links are listed. Partial hardware fields can contain more evidence.','A downloaded candidate payload is not proof of boot or mining compatibility.']
  out.append(row)
 p=catalog.ROOT/'reports/device-matrix.jsonl';p.write_text(''.join(json.dumps(r,ensure_ascii=False)+'\n' for r in out))
 catalog.write_csv(p.with_suffix('.csv'),out,list(out[0]) if out else [])
 print(json.dumps({'device_rows':len(out),'validated_simulations':0,'jsonl':str(p.relative_to(catalog.ROOT))}))
if __name__=='__main__':main()
