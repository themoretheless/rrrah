#include <libheif/heif.h>
#include <stdio.h>
#include <stdlib.h>
static void check(struct heif_error e) { if (e.code) { fprintf(stderr,"%s\n",e.message); exit(2); } }
int main(int argc,char**argv) {
 if(argc!=2)return 2;
 struct heif_context* c=heif_context_alloc(); struct heif_image_handle* h=NULL;
 check(heif_context_read_from_file(c,argv[1],NULL)); check(heif_context_get_primary_image_handle(c,&h));
 uint32_t x=0,y=0; heif_image_handle_get_pixel_aspect_ratio(h,&x,&y);
 printf("stored=%u:%u; ",x,y);
 struct heif_image* image=NULL; check(heif_decode_image(h,&image,heif_colorspace_RGB,heif_chroma_interleaved_RGBA,NULL));
 heif_image_get_pixel_aspect_ratio(image,&x,&y);
 printf("decoded=%u:%u; size=%dx%d\n",x,y,heif_image_get_primary_width(image),heif_image_get_primary_height(image));
 heif_image_release(image);heif_image_handle_release(h);heif_context_free(c);return 0;
}