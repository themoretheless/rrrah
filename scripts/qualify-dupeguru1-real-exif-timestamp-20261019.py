"""Original EXIF matcher over valid, differently colored JPEGs with the same capture time."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-real-exif-timestamp-20261019'
H=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    W.mkdir(exist_ok=False)
    names=['core/pe/matchexif.py','core/pe/exif.py','core/pe/photo.py','qt/pe/photo.py'];pins={p:H(S/p)for p in names}
    for p in pins:assert subprocess.check_output(['git','-C',str(S),'show','HEAD:'+p])==(S/p).read_bytes()
    sys.path.insert(0,str(S))
    from PIL import Image
    from qt.pe.photo import File
    from PyQt5.QtGui import QImage
    from core.pe import matchexif
    from hscommon.jobprogress.job import nulljob
    paths=[]
    for name,color in [('red',(255,0,0)),('blue',(0,0,255))]:
        path=W/(name+'.jpg');tags=Image.Exif();tags[274]=1;tags[36867]='2026:10:19 12:34:56';Image.new('RGB',(40,20),color).save(path,exif=tags);paths.append(path)
    photos=[File(p)for p in paths];matches=matchexif.getmatches(photos,False,nulljob)
    assert len(matches)==1 and matches[0].percentage==100 and H(paths[0])!=H(paths[1])
    pixels=[QImage(str(p)).convertToFormat(QImage.Format_RGB888)for p in paths]
    assert pixels[0].pixelColor(0,0)!=pixels[1].pixelColor(0,0)
    assert all(H(S/p)==pin for p,pin in pins.items())
    result={'status':'verified_metadata_only_EXIF_timestamp_match_on_distinct_real_JPEGs','source_pins':pins,'dimensions':[list(p.dimensions)for p in photos],'timestamps':[p.exif_timestamp for p in photos],'match_count':len(matches),'percentage':matches[0].percentage,'different_file_sha256':True,'different_decoded_pixel_color':True,'input_sha256':{p.name:H(p)for p in paths},'limits':['Actual original Qt Photo, EXIF parser and timestamp matcher; ordinary real JPEG files, no mocked metadata.','EXIF mode explicitly compares capture metadata;100 here is not a pixel-identity statement or a defect claim.','No full GUI, Cocoa/platform matrix, memory probe or production source change.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
