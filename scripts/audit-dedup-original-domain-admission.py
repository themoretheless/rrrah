"""Static corpus/policy admission arithmetic, separate from native outcomes."""
import hashlib,json,math
from pathlib import Path
p=Path('/Users/themoretheless/.codex/tmp/rrrah-dedup-copydays/prepared.json');m=json.loads(p.read_text());original={v['group_id']:v for v in m['images']['original']};rows=[];factors=[2**(n/2) for n in range(7)]
for q in m['images']['strong']:
 o=original[q['group_id']];pixels=[v['source_size'][0]*v['source_size'][1] for v in [o,q]]
 pyramids=[sum(math.floor(v['source_size'][0]/f)*math.floor(v['source_size'][1]/f) for f in factors) for v in [o,q]]
 rows.append({'query':q['filename'],'original':o['filename'],'pixels':pixels,'combined_pixels':sum(pixels),'over_8m_combined':sum(pixels)>8000000,'over_12_8m_combined':sum(pixels)>12800000,'over_per_file_decode':any(n>6400000 for n in pixels),'over_smoothing_work':any(n*10>64000000 for n in pixels),'pyramid_pixels':pyramids,'over_pyramid_pixels':any(n>12800000 for n in pyramids)})
assert len(rows)==229
result={'status':'verified_static_dimension_admission','required_pairs':229,'counts':{key:sum(v[key] for v in rows) for key in ['over_8m_combined','over_12_8m_combined','over_per_file_decode','over_smoothing_work','over_pyramid_pixels']},'rows':rows,'input_hashes':{str(p):hashlib.sha256(p.read_bytes()).hexdigest(),str(Path(__file__)):hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},'scope':'Dimension arithmetic only; EXIF preserves area but oriented rounding of pyramid levels can differ by width/height swap only (symmetric product invariant). Does not predict candidate acceptance, managed allocation, decoder execution or regional sample work. Combined8M refusal applies only if native execution reaches full-domain pixel confirmation.'}
Path('docs/research/dedup-original-domain-admission.json').write_text(json.dumps(result,indent=2)+'\n');print(result['counts'])
