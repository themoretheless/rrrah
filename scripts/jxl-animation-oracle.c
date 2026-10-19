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
  CHECK(argc==2);
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
  for(unsigned frame=0;frame<3;frame++) {
    JxlEncoderFrameSettings *settings=JxlEncoderFrameSettingsCreate(enc,NULL);CHECK(settings);
    CHECK(JxlEncoderSetFrameLossless(settings,JXL_TRUE)==JXL_ENC_SUCCESS);
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_KEEP_INVISIBLE,1)==JXL_ENC_SUCCESS);
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_EFFORT,1)==JXL_ENC_SUCCESS);
    JxlFrameHeader header;JxlEncoderInitFrameHeader(&header);header.duration=durations[frame];
    CHECK(JxlEncoderSetFrameHeader(settings,&header)==JXL_ENC_SUCCESS);
    uint16_t input[60];pixels(input,frame);
    CHECK(JxlEncoderAddImageFrame(settings,&format,input,sizeof(input))==JXL_ENC_SUCCESS);
  }
  JxlEncoderCloseInput(enc);
  uint8_t encoded[16384];uint8_t *next=encoded;size_t available=sizeof(encoded);
  CHECK(JxlEncoderProcessOutput(enc,&next,&available)==JXL_ENC_SUCCESS);
  size_t length=(size_t)(next-encoded);JxlEncoderDestroy(enc);
  write_file(argv[1],"animation-finite16.jxl",encoded,length);
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
      CHECK(frame<3 && header.duration==durations[frame]);
    } else if(status==JXL_DEC_NEED_IMAGE_OUT_BUFFER) {
      size_t size;CHECK(JxlDecoderImageOutBufferSize(dec,&format,&size)==JXL_DEC_SUCCESS);CHECK(size==sizeof(decoded));
      CHECK(JxlDecoderSetImageOutBuffer(dec,&format,decoded,sizeof(decoded))==JXL_DEC_SUCCESS);
    } else if(status==JXL_DEC_FULL_IMAGE) {
      uint16_t expected[60];pixels(expected,frame);CHECK(memcmp(decoded,expected,sizeof(decoded))==0);
      uint8_t little[120];for(unsigned i=0;i<60;i++){little[2*i]=decoded[i]&255;little[2*i+1]=decoded[i]>>8;}
      char name[64];snprintf(name,sizeof(name),"animation-finite16.jxl.%u.rgba16le",frame);
      write_file(argv[1],name,little,sizeof(little));frame++;
    } else CHECK(0);
  }
  CHECK(frame==3);JxlDecoderDestroy(dec);
  printf("{\"license\":\"CC0\",\"libjxl_version\":%u,\"width\":5,\"height\":3,\"bits\":16,\"alpha_bits\":16,\"frames\":3,\"num_loops\":2,\"tps_numerator\":24000,\"tps_denominator\":1001,\"durations\":[3,7,11],\"every_decoded_sample_equals_authored\":true}\n",JxlDecoderVersion());
  return 0;
}
