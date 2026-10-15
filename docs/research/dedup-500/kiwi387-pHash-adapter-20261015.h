#pragma once
#include <string>
#include <vector>
struct Digest { int tag; };
struct Controller { int hash_calls=0; int fail_call=0; int cross_calls=0; int code=1; double score=1.; double threshold=0.; int angles=0; double sigma=0.;double gamma=0.;std::vector<std::string> paths; };
extern Controller control;
int ph_image_digest(const char*,double,double,Digest&,int);
int ph_crosscorr(const Digest&,const Digest&,double&,double);
