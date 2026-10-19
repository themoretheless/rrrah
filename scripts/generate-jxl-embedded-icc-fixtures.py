#!/usr/bin/env python3
"""CC0 RGBA16 animation with an explicitly embedded, independently verified ICC."""
from pathlib import Path
import argparse, hashlib, json, os, shlex, subprocess, tempfile
root = Path(__file__).resolve().parents[1]
ap=argparse.ArgumentParser(description=__doc__)
ap.add_argument('--output',type=Path,default=root/'tests/fixtures/raster')
ap.add_argument('--profile',type=Path,default=root/'tests/fixtures/raster/linear-identity-rgb.icc')
ap.add_argument('--name',default='embedded-icc')
args=ap.parse_args();out=args.output;out.mkdir(parents=True,exist_ok=True);profile=args.profile
stem=f'animation-{args.name}.jxl'
source = (root / 'scripts/jxl-animation-oracle.c').read_text()
source = source.replace('CHECK(argc==2);', 'CHECK(argc==3);')
source = source.replace('CHECK(JxlEncoderSetColorEncoding(enc,&color)==JXL_ENC_SUCCESS);', '''
  uint8_t icc[65536]; FILE *profile=fopen(argv[2],"rb"); CHECK(profile);
  size_t icc_size=fread(icc,1,sizeof(icc),profile); CHECK(feof(profile)); fclose(profile);
  CHECK(JxlEncoderSetICCProfile(enc,icc,icc_size)==JXL_ENC_SUCCESS);''')
source = source.replace('JXL_DEC_BASIC_INFO|JXL_DEC_FRAME|JXL_DEC_FULL_IMAGE', 'JXL_DEC_BASIC_INFO|JXL_DEC_COLOR_ENCODING|JXL_DEC_FRAME|JXL_DEC_FULL_IMAGE')
source = source.replace('} else if(status==JXL_DEC_FRAME)', '''} else if(status==JXL_DEC_COLOR_ENCODING) {
      size_t size;CHECK(JxlDecoderGetICCProfileSize(dec,JXL_COLOR_PROFILE_TARGET_ORIGINAL,&size)==JXL_DEC_SUCCESS);
      CHECK(size==icc_size);uint8_t actual[65536];
      CHECK(JxlDecoderGetColorAsICCProfile(dec,JXL_COLOR_PROFILE_TARGET_ORIGINAL,actual,size)==JXL_DEC_SUCCESS);
      CHECK(memcmp(actual,icc,size)==0);
    } else if(status==JXL_DEC_FRAME)''')
with tempfile.TemporaryDirectory() as directory:
    tmp = Path(directory); c = tmp/'oracle.c'; c.write_text(source); binary=tmp/'oracle'
    flags=shlex.split(subprocess.check_output(['pkg-config','--cflags','--libs','libjxl'],text=True))
    subprocess.run([os.environ.get('CC','cc'),'-O2',str(c),*flags,'-o',str(binary)],check=True)
    metadata=json.loads(subprocess.check_output([str(binary),str(tmp),str(profile)],text=True))
    for suffix in ['', '.0.rgba16le', '.1.rgba16le', '.2.rgba16le']:
        (out/f'{stem}{suffix}').write_bytes((tmp/f'animation-finite16.jxl{suffix}').read_bytes())
metadata.update({'license':'CC0 authored pixels and producer; ICC retains existing fixture provenance',
                 'derived_c_source_sha256':hashlib.sha256(source.encode()).hexdigest(),
                 'source_sha256':hashlib.sha256((out/stem).read_bytes()).hexdigest(),
                 'profile_sha256':hashlib.sha256(profile.read_bytes()).hexdigest(),
                 'original_icc_matches_authored':True,
                 'scope':'Native samples and original embedded ICC verified by libjxl C decoder; not physical HDR validation.'})
(out/f'jxl-{args.name}.json').write_text(json.dumps(metadata,indent=2)+'\n')
