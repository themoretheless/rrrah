/* CC0: independent libjxl finite-loop, fractional-tick RGBA16 controls. */
#include <jxl/encode.h>
#include <jxl/decode.h>
#include <jxl/color_encoding.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr,"failed line %d: %s\n",__LINE__,#x); exit(1); } } while(0)
static void pixels(uint16_t *p, unsigned frame) {
  const uint16_t alpha[4]={0,1,32768,65535};
  for(unsigned i=0;i<15;i++) {
    p[4*i]=32768+(i+frame)%2;
    p[4*i+1]=(i*3371+frame*721)%65536;
    p[4*i+2]=(i*917+frame*1337)%65536;
    p[4*i+3]=alpha[(i+frame)%4];
  }
}
static void write_file(const char *directory,const char *name,const void *p,size_t n) {
  char path[4096];CHECK(snprintf(path,sizeof(path),"%s/%s",directory,name)>0);
  FILE *f=fopen(path,"wb");CHECK(f);CHECK(fwrite(p,1,n,f)==n);CHECK(fclose(f)==0);
}
int main(int argc,char **argv) {
  CHECK(argc==3);
  unsigned frame_count=(unsigned)strtoul(argv[2],NULL,10);CHECK(frame_count>=3 && frame_count<=10000);
  char basename[80];snprintf(basename,sizeof(basename),"animation-scale-%u.jxl",frame_count);
  JxlEncoder *enc=JxlEncoderCreate(NULL);CHECK(enc);
  JxlBasicInfo info;JxlEncoderInitBasicInfo(&info);
  info.xsize=5;info.ysize=3;info.bits_per_sample=16;info.uses_original_profile=JXL_TRUE;
  info.num_color_channels=3;info.num_extra_channels=1;info.alpha_bits=16;
  info.have_animation=JXL_TRUE;info.animation.tps_numerator=24000;info.animation.tps_denominator=1001;info.animation.num_loops=2;
  CHECK(JxlEncoderSetBasicInfo(enc,&info)==JXL_ENC_SUCCESS);
  JxlColorEncoding color;JxlColorEncodingSetToSRGB(&color,JXL_FALSE);
  CHECK(JxlEncoderSetColorEncoding(enc,&color)==JXL_ENC_SUCCESS);
  JxlPixelFormat format={4,JXL_TYPE_UINT16,JXL_NATIVE_ENDIAN,0};
  const uint32_t durations[3]={3,7,11};
  for(unsigned frame=0;frame<frame_count;frame++) {
    JxlEncoderFrameSettings *settings=JxlEncoderFrameSettingsCreate(enc,NULL);CHECK(settings);
    CHECK(JxlEncoderSetFrameLossless(settings,JXL_TRUE)==JXL_ENC_SUCCESS);
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_KEEP_INVISIBLE,1)==JXL_ENC_SUCCESS);
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_EFFORT,1)==JXL_ENC_SUCCESS);
    JxlFrameHeader header;JxlEncoderInitFrameHeader(&header);header.duration=durations[frame%3];
    CHECK(JxlEncoderSetFrameHeader(settings,&header)==JXL_ENC_SUCCESS);
    uint16_t input[60];pixels(input,frame);
    CHECK(JxlEncoderAddImageFrame(settings,&format,input,sizeof(input))==JXL_ENC_SUCCESS);
  }
  JxlEncoderCloseInput(enc);
  size_t capacity=1048576, length=0;uint8_t *encoded=(uint8_t*)malloc(capacity);CHECK(encoded);
  for(;;) {
    uint8_t *next=encoded+length;size_t available=capacity-length;
    JxlEncoderStatus status=JxlEncoderProcessOutput(enc,&next,&available);length=(size_t)(next-encoded);
    if(status==JXL_ENC_SUCCESS)break;
    CHECK(status==JXL_ENC_NEED_MORE_OUTPUT && capacity<67108864);
    capacity*=2;encoded=(uint8_t*)realloc(encoded,capacity);CHECK(encoded);
  }
  JxlEncoderDestroy(enc);write_file(argv[1],basename,encoded,length);
  JxlDecoder *dec=JxlDecoderCreate(NULL);CHECK(dec);
  CHECK(JxlDecoderSubscribeEvents(dec,JXL_DEC_BASIC_INFO|JXL_DEC_FRAME|JXL_DEC_FULL_IMAGE)==JXL_DEC_SUCCESS);
  CHECK(JxlDecoderSetInput(dec,encoded,length)==JXL_DEC_SUCCESS);JxlDecoderCloseInput(dec);
  uint16_t decoded[60];unsigned frame=0;
  for(;;) {
    JxlDecoderStatus status=JxlDecoderProcessInput(dec);
    if(status==JXL_DEC_SUCCESS)break;
    if(status==JXL_DEC_BASIC_INFO) {
      CHECK(JxlDecoderGetBasicInfo(dec,&info)==JXL_DEC_SUCCESS);
      CHECK(info.bits_per_sample==16 && info.alpha_bits==16 && info.animation.num_loops==2);
      CHECK(info.animation.tps_numerator==24000 && info.animation.tps_denominator==1001);
    } else if(status==JXL_DEC_FRAME) {
      JxlFrameHeader header;CHECK(JxlDecoderGetFrameHeader(dec,&header)==JXL_DEC_SUCCESS);
      CHECK(frame<frame_count && header.duration==durations[frame%3]);
    } else if(status==JXL_DEC_NEED_IMAGE_OUT_BUFFER) {
      size_t size;CHECK(JxlDecoderImageOutBufferSize(dec,&format,&size)==JXL_DEC_SUCCESS);CHECK(size==sizeof(decoded));
      CHECK(JxlDecoderSetImageOutBuffer(dec,&format,decoded,sizeof(decoded))==JXL_DEC_SUCCESS);
    } else if(status==JXL_DEC_FULL_IMAGE) {
      uint16_t expected[60];pixels(expected,frame);CHECK(memcmp(decoded,expected,sizeof(decoded))==0);
      uint8_t little[120];for(unsigned i=0;i<60;i++){little[2*i]=decoded[i]&255;little[2*i+1]=decoded[i]>>8;}
      if(frame==0 || frame==frame_count/2 || frame==frame_count-1) {
        char name[128];snprintf(name,sizeof(name),"%s.%u.rgba16le",basename,frame);
        write_file(argv[1],name,little,sizeof(little));
      }
      frame++;
    } else CHECK(0);
  }
  CHECK(frame==frame_count);JxlDecoderDestroy(dec);free(encoded);
  printf("{\"libjxl_version\":%u,\"frames\":%u,\"encoded_bytes\":%zu,\"width\":5,\"height\":3,\"bits\":16,\"all_native_samples_match_authored\":true}\n",JxlDecoderVersion(),frame_count,length);
  return 0;
}
