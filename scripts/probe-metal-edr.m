#import <AppKit/AppKit.h>
int main(void) {
 @autoreleasepool {
  NSMutableArray *screens = [NSMutableArray array];
  for (NSScreen *screen in NSScreen.screens) {
   [screens addObject:@{@"name":screen.localizedName,
    @"current_edr_headroom":@(screen.maximumExtendedDynamicRangeColorComponentValue),
    @"potential_edr_headroom":@(screen.maximumPotentialExtendedDynamicRangeColorComponentValue),
    @"reference_edr_headroom":@(screen.maximumReferenceExtendedDynamicRangeColorComponentValue)}];
  }
  NSData *data = [NSJSONSerialization dataWithJSONObject:screens options:NSJSONWritingPrettyPrinted error:nil];
  fwrite(data.bytes,1,data.length,stdout);
 }
}
