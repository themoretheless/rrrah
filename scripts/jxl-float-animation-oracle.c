/* CC0: independent libjxl linear float/HDR, fractional-tick RGBA32F controls. */
#include <jxl/encode.h>
#include <jxl/decode.h>
#include <jxl/color_encoding.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr,"failed line %d: %s\n",__LINE__,#x); exit(1); } } while(0)
static void pixels(float *p, unsigned frame) {
  const float values[11]={0.f,.25f,.5f,1.f,2.f,4.f,16.f,-.25f,-2.f,1.00000011920928955078125f,1.0000002384185791015625f};
  const float alpha[4]={0.f,.25f,.5f,1.f};
  for(unsigned i=0;i<15;i++) {
    p[4*i]=values[(i+frame)%11];
    p[4*i+1]=values[(i*3+frame)%11];
    p[4*i+2]=values[(i*7+frame)%11];
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
  info.xsize=5;info.ysize=3;info.bits_per_sample=32;info.exponent_bits_per_sample=8;info.uses_original_profile=JXL_TRUE;
  info.num_color_channels=3;info.num_extra_channels=1;info.alpha_bits=32;info.alpha_exponent_bits=8;
  info.have_animation=JXL_TRUE;info.animation.tps_numerator=24000;info.animation.tps_denominator=1001;info.animation.num_loops=2;
  CHECK(JxlEncoderSetBasicInfo(enc,&info)==JXL_ENC_SUCCESS);
  JxlColorEncoding color;JxlColorEncodingSetToLinearSRGB(&color,JXL_FALSE);
  CHECK(JxlEncoderSetColorEncoding(enc,&color)==JXL_ENC_SUCCESS);
  JxlPixelFormat format={4,JXL_TYPE_FLOAT,JXL_NATIVE_ENDIAN,0};
  const uint32_t durations[3]={3,7,11};
  for(unsigned frame=0;frame<3;frame++) {
    JxlEncoderFrameSettings *settings=JxlEncoderFrameSettingsCreate(enc,NULL);CHECK(settings);
    CHECK(JxlEncoderSetFrameLossless(settings,JXL_TRUE)==JXL_ENC_SUCCESS);
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_KEEP_INVISIBLE,1)==JXL_ENC_SUCCESS);
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_EFFORT,7)==JXL_ENC_SUCCESS);
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_PATCHES,0)==JXL_ENC_SUCCESS);
    /* libjxl0.12 mixed-sign fp32 workaround, issue4902: reference encoder only. */
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_MODULAR_PREDICTOR,0)==JXL_ENC_SUCCESS);
    CHECK(JxlEncoderFrameSettingsSetOption(settings,JXL_ENC_FRAME_SETTING_MODULAR_COLOR_SPACE,0)==JXL_ENC_SUCCESS);
    JxlFrameHeader header;JxlEncoderInitFrameHeader(&header);header.duration=durations[frame];
    CHECK(JxlEncoderSetFrameHeader(settings,&header)==JXL_ENC_SUCCESS);
    float input[60];pixels(input,frame);
    CHECK(JxlEncoderAddImageFrame(settings,&format,input,sizeof(input))==JXL_ENC_SUCCESS);
  }
  JxlEncoderCloseInput(enc);
  uint8_t encoded[16384];uint8_t *next=encoded;size_t available=sizeof(encoded);
  JxlEncoderStatus output_status=JxlEncoderProcessOutput(enc,&next,&available);
  if(output_status!=JXL_ENC_SUCCESS)fprintf(stderr,"encoder_status=%d error=%d remaining=%zu\n",(int)output_status,(int)JxlEncoderGetError(enc),available);
  CHECK(output_status==JXL_ENC_SUCCESS);
  size_t length=(size_t)(next-encoded);JxlEncoderDestroy(enc);
  write_file(argv[1],"animation-linear-float.jxl",encoded,length);
  JxlDecoder *dec=JxlDecoderCreate(NULL);CHECK(dec);
  CHECK(JxlDecoderSubscribeEvents(dec,JXL_DEC_BASIC_INFO|JXL_DEC_FRAME|JXL_DEC_FULL_IMAGE)==JXL_DEC_SUCCESS);
  CHECK(JxlDecoderSetInput(dec,encoded,length)==JXL_DEC_SUCCESS);JxlDecoderCloseInput(dec);
  float decoded[60];unsigned frame=0;
  for(;;) {
    JxlDecoderStatus status=JxlDecoderProcessInput(dec);
    if(status==JXL_DEC_SUCCESS)break;
    if(status==JXL_DEC_BASIC_INFO) {
      CHECK(JxlDecoderGetBasicInfo(dec,&info)==JXL_DEC_SUCCESS);
      CHECK(info.bits_per_sample==32 && info.exponent_bits_per_sample==8 && info.alpha_bits==32 && info.alpha_exponent_bits==8 && info.animation.num_loops==2);
      CHECK(info.animation.tps_numerator==24000 && info.animation.tps_denominator==1001);
    } else if(status==JXL_DEC_FRAME) {
      JxlFrameHeader header;CHECK(JxlDecoderGetFrameHeader(dec,&header)==JXL_DEC_SUCCESS);
      CHECK(frame<3 && header.duration==durations[frame]);
    } else if(status==JXL_DEC_NEED_IMAGE_OUT_BUFFER) {
      size_t size;CHECK(JxlDecoderImageOutBufferSize(dec,&format,&size)==JXL_DEC_SUCCESS);CHECK(size==sizeof(decoded));
      CHECK(JxlDecoderSetImageOutBuffer(dec,&format,decoded,sizeof(decoded))==JXL_DEC_SUCCESS);
    } else if(status==JXL_DEC_FULL_IMAGE) {
      float expected[60];pixels(expected,frame);CHECK(memcmp(decoded,expected,sizeof(decoded))==0);
      uint8_t little[240];for(unsigned i=0;i<60;i++){uint32_t bits;memcpy(&bits,&decoded[i],4);for(unsigned b=0;b<4;b++)little[4*i+b]=(uint8_t)(bits>>(8*b));}
      char name[64];snprintf(name,sizeof(name),"animation-linear-float.jxl.%u.rgba32fle",frame);
      write_file(argv[1],name,little,sizeof(little));frame++;
    } else CHECK(0);
  }
  CHECK(frame==3);JxlDecoderDestroy(dec);
  printf("{\"license\":\"CC0\",\"libjxl_version\":%u,\"width\":5,\"height\":3,\"bits\":32,\"exponent_bits\":8,\"alpha_bits\":32,\"alpha_exponent_bits\":8,\"frames\":3,\"num_loops\":2,\"tps_numerator\":24000,\"tps_denominator\":1001,\"durations\":[3,7,11],\"every_decoded_sample_equals_authored\":true}\n",JxlDecoderVersion());
  return 0;
}
