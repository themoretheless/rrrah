#!/usr/bin/env python3
"""CC0 authored FITS images; Astropy 7.2.0 native/physical/window oracles."""
from pathlib import Path
import hashlib
import numpy as np
from astropy.io import fits

root = Path(__file__).resolve().parents[1] / 'tests/fixtures/raster'
rows=[]
window_min,window_max=-100.,1000.
cases=[
    ('u8','u1',[0,1,2,127,128,254,255]),
    ('i16','i2',[-32768,-257,-1,0,1,257,32767]),
    ('i32','i4',[-2147483648,-16777217,-1,0,1,16777217,2147483647]),
    ('i64','i8',[-2**63,-2**53-1,-1,0,1,2**53+1,2**63-1]),
    ('f32','f4',[-100.,-1.,-0.,0.,0.5,1.0000001192092896,1000.]),
    ('f64','f8',[-100.,-1.,-0.,0.,0.5,1.0000000000000002,1000.]),
    ('u16','u2',[0,1,2,257,32767,32768,65535]),
]
for name,dtype,values in cases:
    data=np.resize(np.array(values,dtype=dtype),30).reshape(2,3,5)
    image=fits.PrimaryHDU(data)
    image.header['BUNIT']='CC0 raw/physical units'
    image.header['OBJECT']="CC0 / O'Brien"
    image.header['CRPIX1']=2.5
    image.header['HISTORY']='CC0 first history'
    image.header['HISTORY']='CC0 second history'
    fits.HDUList([image]).writeto(root/f'fits-{name}.fits',overwrite=True)
# Explicit exact binary scaling and an integer undefined-value code.
for name,blank in [('scaled',False),('blank',True)]:
    data=np.arange(-15,15,dtype='i2').reshape(2,3,5)
    image=fits.PrimaryHDU(data)
    image.header['BSCALE']=-0.5
    image.header['BZERO']=10.25
    if blank:image.header['BLANK']=-15
    image.writeto(root/f'fits-{name}.fits',overwrite=True)
# Empty primary and heap-bearing binary table before two IMAGE HDUs.
column=fits.Column(name='NUM',format='PJ()',array=np.array([np.array([1,2],dtype='i4'),np.array([3],dtype='i4')],dtype=object))
image=fits.ImageHDU(np.arange(30,dtype='i2').reshape(2,3,5),name='SCI')
second=fits.ImageHDU(np.arange(15,dtype='u1').reshape(3,5),name='CAL')
fits.HDUList([fits.PrimaryHDU(),fits.BinTableHDU.from_columns([column]),image,second]).writeto(root/'fits-multi-hdu.fits',overwrite=True)
# Native float32 special values are preserved; their display needs a missing-value policy.
fits.PrimaryHDU(np.array([np.nan,np.inf,-np.inf,-0.,0.,1.],dtype='f4').reshape(2,3)).writeto(root/'fits-nonfinite.fits',overwrite=True)
for path in sorted(root.glob('fits-*.fits')):
    with fits.open(path,memmap=False,do_not_scale_image_data=True,uint=False) as native, fits.open(path,memmap=False) as physical:
        index=0
        for hdu_index,hdu in enumerate(native):
            if not isinstance(hdu,(fits.PrimaryHDU,fits.ImageHDU)) or hdu.data is None:continue
            planes=hdu.data.reshape((-1,)+hdu.data.shape[-2:])
            scaled=physical[hdu_index].data.reshape(planes.shape)
            for section,plane in enumerate(planes):
                h,w=plane.shape
                kind=plane.dtype.kind+str(plane.dtype.itemsize)
                prefix=path.name+f'-{index}'
                raw_name=prefix+'.native'
                physical_name=prefix+'.physical-f64'
                window_name=prefix+'.window-rgba32f'
                (root/raw_name).write_bytes(plane.astype(plane.dtype.newbyteorder('<')).tobytes())
                values=scaled[section].astype('<f8')
                (root/physical_name).write_bytes(values.tobytes())
                rgba=np.ones((h,w,4),dtype='<f4')
                rgba[:,:,:3]=np.clip((values-window_min)/(window_max-window_min),0,1)[:,:,None]
                (root/window_name).write_bytes(rgba.tobytes())
                rows.append(f'{path.name}\t{index}\t{w}\t{h}\t{kind}\t{raw_name}\t{physical_name}\t{window_name}\t{hashlib.sha256(path.read_bytes()).hexdigest()}')
                index+=1
(root/'fits-manifest.tsv').write_text('# Astropy 7.2.0; source\tindex\twidth\theight\ttype\tnative\tphysical\twindow\tsha256; window -100..1000\n'+'\n'.join(rows)+'\n')
