extern int formatSigned;
void setupAlphaTable();
void decompressBlockAlpha16bitC(unsigned char*,unsigned char*,int,int,int,int,int);
extern "C" void ericsson_probe(unsigned char* source,unsigned char* output,int sign) {
 setupAlphaTable();formatSigned=sign;
 decompressBlockAlpha16bitC(source,output,4,4,0,0,1);
}
