"""Exercise original Qt Photo and C blocks for eight ordinary EXIF orientations."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/dupeguru1-20261019/source'
W=R/'.dedup-work/dupeguru1-qt-exif-orientations-v2-20261019'
H=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def main():
    W.mkdir(exist_ok=False)
    source_names=['core/pe/exif.py','core/pe/photo.py','qt/pe/photo.py','qt/pe/block.py','qt/pe/modules/block.c']
    pins={p:H(S/p)for p in source_names}
    for p in pins:assert subprocess.check_output(['git','-C',str(S),'show','HEAD:'+p])==(S/p).read_bytes()
    sys.path.insert(0,str(S))
    from PIL import Image
    from PyQt5.QtGui import QImage
    from PyQt5.QtCore import QT_VERSION_STR,PYQT_VERSION_STR,qVersion
    from core.pe import exif
    from qt.pe.photo import File
    from qt.pe import _block_qt
    binary_pin=H(Path(_block_qt.__file__))
    base=Image.new('RGB',(40,20))
    for y in range(20):
        for x in range(40):base.putpixel((x,y),((x*6)%256,(y*12)%256,((x+y)*4)%256))
    transforms={2:Image.Transpose.FLIP_LEFT_RIGHT,3:Image.Transpose.ROTATE_180,4:Image.Transpose.FLIP_TOP_BOTTOM,5:Image.Transpose.TRANSPOSE,6:Image.Transpose.ROTATE_270,7:Image.Transpose.TRANSVERSE,8:Image.Transpose.ROTATE_90}
    rows=[];sheet=Image.new('RGB',(320,160),'white')
    for orientation in range(1,9):
        tags=Image.Exif();tags[274]=orientation;tags[36867]='2026:10:19 12:34:56'
        path=W/f'orientation-{orientation}.jpg';base.save(path,quality=95,subsampling=0,exif=tags)
        with path.open('rb')as stream:fields=exif.get_fields(stream)
        assert fields['Orientation']==[orientation] and fields['DateTimeOriginal']=='2026:10:19 12:34:56'
        wrapped=File(path);assert wrapped.get_orientation()==orientation
        expected_dimensions=(20,40)if orientation>=5 else(40,20)
        assert wrapped.dimensions==expected_dimensions
        qimage=QImage(str(path)).convertToFormat(QImage.Format_RGB888);assert not qimage.isNull()
        raw=qimage.bits().asstring(qimage.byteCount())
        reference=Image.frombytes('RGB',(qimage.width(),qimage.height()),raw,'raw','RGB',qimage.bytesPerLine(),1)
        if orientation!=1:reference=reference.transpose(transforms[orientation])
        assert reference.size==expected_dimensions
        width,height=reference.size;bw=max(width//15,1);bh=max(height//15,1);expected=[]
        for by in range(15):
            top=min(by*bh,height-bh-1)
            for bx in range(15):
                left=min(bx*bw,width-bw-1)
                pixels=list(reference.crop((left,top,left+bw,top+bh)).getdata())
                expected.append(tuple(sum(p[c]for p in pixels)//len(pixels)for c in range(3)))
        actual=wrapped.get_blocks(15);assert len(actual)==225
        mismatches=sum(x!=y for x,y in zip(actual,expected))
        rows.append({'orientation':orientation,'dimensions':list(wrapped.dimensions),'timestamp':wrapped.exif_timestamp,'blocks':len(actual),'independent_transform_block_mismatches':mismatches,'jpeg_sha256':H(path)})
        preview=reference.copy();preview.thumbnail((76,76));sheet.paste(preview,(((orientation-1)%4)*80,((orientation-1)//4)*80))
    sheet.save(W/'orientations.png')
    assert all(H(S/p)==pin for p,pin in pins.items())and H(Path(_block_qt.__file__))==binary_pin
    result={'status':'executed_original_Qt_photo_eight_EXIF_orientations','qt_compile_version':QT_VERSION_STR,'qt_runtime_version':qVersion(),'pyqt_version':PYQT_VERSION_STR,'source_pins':pins,'native_module':_block_qt.__file__,'native_binary_sha256':binary_pin,'rows':rows,'all_orientations_match_independent_transform':all(r['independent_transform_block_mismatches']==0 for r in rows),'limits':['Actual QImage/QImageReader and unchanged original Qt C block module; no Photo/Pillow platform decoder adapter.','Independent Pillow orthogonal transforms start from actual decoded Qt RGB pixels to avoid JPEG decoder rounding differences.','Independent block sampling reproduces original native grid coordinates; this does not certify complete pixel coverage or ideal sampling.','Ordinary valid40x20JPEGs only; no invalid images, native buffers, sanitizers or memory-error probes.','No full GUI, Cocoa/Windows/Linux or whole repository qualification.']}
    (W/'result.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))

if __name__=='__main__':main()
