#import <Foundation/Foundation.h>
@interface KWMatcher : NSObject
@property (nonatomic,strong) id subject;
+ (NSArray *)matcherStrings;
- (BOOL)evaluate;
- (NSString *)failureMessageForShould;
- (NSString *)failureMessageForShouldNot;
@end
