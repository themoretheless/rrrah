#!/usr/bin/env python3
"""Preserve corpus cases where registration worsens known-grid geometric agreement."""
import argparse,json,pathlib,hashlib
p=argparse.ArgumentParser();p.add_argument('manifest',type=pathlib.Path);p.add_argument('report',type=pathlib.Path);p.add_argument('analysis',type=pathlib.Path);p.add_argument('output',type=pathlib.Path);a=p.parse_args()
m=json.loads(a.manifest.read_text());r=json.loads(a.report.read_text());x=json.loads(a.analysis.read_text())
assert hashlib.sha256(a.manifest.read_bytes()).hexdigest()==r['manifest_sha256']
pairs={(v['sequence'],v['target_index']):v for v in m['pairs']};records={(v['sequence'],v['target_index']):v for v in r['results']}
assert len(records)==r['completed_pairs']
cases=[]
for row in x['rows']:
 key=(row['sequence'],row['target_index']);initial=row['geometry_grid'];registered=row['registered_grid']
 if initial is None or registered is None:continue
 if min(initial['valid_points'],registered['valid_points'])<4 or initial['invalid_predictions'] or registered['invalid_predictions']:continue
 if initial['median_pixels']>2 or registered['median_pixels']<=10:continue
 record=records[key];pair=pairs[key];e=record['evidence']
 assert record['status']=='ok' and e['geometry'] is not None and e['registered'] is not None
 cases.append({'sequence':key[0],'target_index':key[1],'left':pair['left'],'right':pair['right'],'published_homography':pair['normalized_homography'],'estimated_homography':e['geometry'],'registered_homography':e['registered'],'initial_grid':initial,'registered_grid':registered,'candidate':e['candidate'],'inliers':e['inliers'],'correspondences':e['correspondences']})
result={'manifest_sha256':r['manifest_sha256'],'probe_sha256':r['probe_sha256'],'report_sha256':hashlib.sha256(a.report.read_bytes()).hexdigest(),'analysis_pairs':x['completed'],'required_pairs':580,'complete':x['complete'],'cases':cases,'scope':'Diagnostic regression cases: initial grid median <=2 pixels, registered median >10, >=4 valid samples, no invalid projections. Grid/H convention assumptions remain; this is not a duplicate label or continuous-bound certificate.'}
a.output.write_text(json.dumps(result,indent=2)+'\n');print('registration drift cases',len(cases),'analysed pairs',x['completed'])
