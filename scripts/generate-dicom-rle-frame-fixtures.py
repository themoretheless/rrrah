"""CC0 RLE frame controls, independent pydicom3.0.2 encode/decode oracle."""
from pathlib import Path
import numpy as np
from pydicom.dataset import Dataset,FileMetaDataset
from pydicom.pixels.encoders import RLELosslessEncoder
from pydicom.encaps import encapsulate
from pydicom.uid import RLELossless
root=Path(__file__).resolve().parents[1]/'tests/fixtures/raster';manifest=['file\twidth\theight\tsamples\tbits']
for width in (1,2,3,127,128,129,257):
 for samples in (1,3):
  for bits in (8,16):
   height=3;dtype=np.uint8 if bits==8 else np.uint16
   shape=(height,width,samples) if samples==3 else (height,width)
   p=np.arange(height*width*samples,dtype=np.uint32).reshape(shape)
   a=((p*257+p*p*37+53)%(1<<bits)).astype(dtype);a[0]=((1<<bits)-1)//3
   kwargs=dict(rows=height,columns=width,samples_per_pixel=samples,bits_allocated=bits,bits_stored=bits,pixel_representation=0,photometric_interpretation='RGB' if samples==3 else 'MONOCHROME2',number_of_frames=1,planar_configuration=0)
   frame=RLELosslessEncoder.encode(a,encoding_plugin='pydicom',**kwargs)
   ds=Dataset();ds.file_meta=FileMetaDataset();ds.file_meta.TransferSyntaxUID=RLELossless
   for attr,value in [('Rows',height),('Columns',width),('SamplesPerPixel',samples),('BitsAllocated',bits),('BitsStored',bits),('HighBit',bits-1),('PixelRepresentation',0),('PhotometricInterpretation',kwargs['photometric_interpretation'])]:setattr(ds,attr,value)
   if samples==3:ds.PlanarConfiguration=0
   ds.PixelData=encapsulate([frame]);ds['PixelData'].is_undefined_length=True
   decoded=ds.pixel_array;assert np.array_equal(decoded,a)
   name=f'dicom-rle-frame-{width}-s{samples}-{bits}.rle';(root/name).write_bytes(frame)
   (root/f'{name}.interleaved').write_bytes(decoded.astype('<u2' if bits==16 else np.uint8).tobytes())
   planar=decoded.transpose(2,0,1) if samples==3 else decoded
   (root/f'{name}.planar').write_bytes(planar.astype('<u2' if bits==16 else np.uint8).tobytes());manifest.append(f'{name}\t{width}\t{height}\t{samples}\t{bits}')
(root/'dicom-rle-frame-manifest.tsv').write_text('\n'.join(manifest)+'\n');print('wrote',len(manifest)-1,'RLE frames')
