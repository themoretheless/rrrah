#import "FTpHashKiwiMatcher.h"
#import "pHash.h"
#include <cmath>
#include <iostream>
#include <limits>
Controller control;
@implementation KWMatcher
+ (NSArray *)matcherStrings { return @[]; }
- (BOOL)evaluate { return NO; }
- (NSString *)failureMessageForShould { return @""; }
- (NSString *)failureMessageForShouldNot { return @""; }
@end
int ph_image_digest(const char* path,double sigma,double gamma,Digest& out,int angles){
 ++control.hash_calls;control.paths.push_back(path?path:"(null)");control.sigma=sigma;control.gamma=gamma;control.angles=angles;out.tag=control.hash_calls;return control.hash_calls==control.fail_call?-1:0;
}
int ph_crosscorr(const Digest&,const Digest&,double& score,double threshold){++control.cross_calls;score=control.score;control.threshold=threshold;return control.code;}
static bool raises(void (^body)(void)) { @try { body(); } @catch(NSException*) { return true; } return false; }
int main(){@autoreleasepool{
 int checks=0;auto check=[&](bool condition){if(!condition){std::cerr<<"failed control "<<checks+1<<"\n";std::exit(1);}++checks;};
 auto reset=[&](int code,double score,int fail=0){control=Controller{};control.code=code;control.score=score;control.fail_call=fail;};
 FTpHashKiwiMatcher* matcher=[FTpHashKiwiMatcher new];matcher.subject=imageAtPath(@"actual.jpg");auto expected=imageAtPath(@"expected.jpg");
 check([[FTpHashKiwiMatcher matcherStrings] count]==2);
 [matcher equalImage:expected];reset(1,1.);check([matcher evaluate]);check(control.threshold==0.999);check(control.hash_calls==2&&control.cross_calls==1);check(control.angles==180&&control.sigma==1.&&control.gamma==1.);check(control.paths[0]=="actual.jpg"&&control.paths[1]=="expected.jpg");
 [matcher equalImage:expected withThreshold:0.9];reset(0,0.25);check(![matcher evaluate]);check([[matcher valueForKey:@"ccp"]doubleValue]==0.25);
 reset(1,1.);check([matcher evaluate]);check([[matcher valueForKey:@"ccp"]doubleValue]==0.25);check([[matcher failureMessageForShouldNot] containsString:@"0.250000"]);
 reset(-1,0.);check(raises(^{[matcher evaluate];}));
 reset(1,1.,1);check(raises(^{[matcher evaluate];}));check(control.hash_calls==1&&control.cross_calls==0);
 reset(1,1.,2);check(raises(^{[matcher evaluate];}));check(control.hash_calls==2&&control.cross_calls==0);
 matcher.subject=@"wrong";reset(1,1.);check(raises(^{[matcher evaluate];}));check(control.hash_calls==0);
 matcher.subject=imageAtPath(@"actual.jpg");[matcher equalImage:nil];reset(1,1.);check(raises(^{[matcher evaluate];}));check(control.hash_calls==0);
 [matcher equalImage:expected];matcher.subject=imageAtPath(nil);reset(1,1.);check([matcher evaluate]);check(control.paths[0]=="(null)");
 NSMutableString* path=[NSMutableString stringWithString:@"before.jpg"];matcher.subject=imageAtPath(path);[path appendString:@"-changed"];reset(1,1.);check([matcher evaluate]);check(control.paths[0]=="before.jpg-changed");
 matcher.subject=imageAtURL([NSURL URLWithString:@"https://example.invalid/frame.jpg"]);reset(1,1.);check([matcher evaluate]);check(control.paths[0]=="/frame.jpg");
 for(double value:{-1.,2.,std::numeric_limits<double>::quiet_NaN(),std::numeric_limits<double>::infinity()}){[matcher equalImage:expected withThreshold:value];reset(1,1.);check([matcher evaluate]);check(std::isnan(value)?std::isnan(control.threshold):control.threshold==value);}
 std::cout<<"CHECKS="<<checks<<" DEFAULT_THRESHOLD=0.999 STALE_POSITIVE_SCORE=0.25\n";
}}
