#include <set>
#include <initializer_list>
#include <iostream>
struct COptions { inline static bool ScanForDuplicates=false;
inline static int ExcludeVolumeMountPoints=0;
inline static int ExcludeJunctions=0;
inline static int ExcludeSymbolicLinksDirectory=0;
inline static int SkipDupeDetectionCloudLinks=0;
inline static int ExcludeHiddenDirectory=0;
inline static int ExcludeProtectedDirectory=0;
inline static int ExcludeDropboxIgnored=0;
inline static int UseBackupRestore=0;
inline static int ExcludeSymbolicLinksFile=0;
inline static int ExcludeHiddenFile=0;
inline static int ExcludeProtectedFile=0;
inline static int ProcessHardlinks=0;
inline static int SampleLargeFiles=0;
};
bool BindsChanged(const std::set<int*>& changed,std::initializer_list<int*> settings) { for(auto* p:settings)if(changed.contains(p))return true;return false;}
struct Edit {bool changed;bool Changed() const{return changed;}};
int main(){int checks=0;int* flags[]={&COptions::ExcludeVolumeMountPoints,&COptions::ExcludeJunctions,&COptions::ExcludeSymbolicLinksDirectory,&COptions::SkipDupeDetectionCloudLinks,&COptions::ExcludeHiddenDirectory,&COptions::ExcludeProtectedDirectory,&COptions::ExcludeDropboxIgnored,&COptions::UseBackupRestore,&COptions::ExcludeSymbolicLinksFile,&COptions::ExcludeHiddenFile,&COptions::ExcludeProtectedFile,&COptions::ProcessHardlinks,&COptions::SampleLargeFiles};
for(unsigned mask=0;mask<(1u<<14);mask++)for(bool active:{false,true}) {
COptions::ScanForDuplicates=active;std::set<int*> checkboxes;for(int i=0;i<13;i++)if(mask&(1u<<i))checkboxes.insert(flags[i]);Edit fileHashAlgorithm{bool(mask&(1u<<13))};
const bool refreshReparsepoints = BindsChanged(checkboxes, { &COptions::ExcludeJunctions,
        &COptions::ExcludeSymbolicLinksDirectory, &COptions::ExcludeVolumeMountPoints, &COptions::ExcludeSymbolicLinksFile });
    const bool refreshAll = BindsChanged(checkboxes, { &COptions::ExcludeHiddenDirectory,
        &COptions::ExcludeProtectedDirectory, &COptions::ExcludeDropboxIgnored,
        &COptions::ExcludeHiddenFile, &COptions::ExcludeProtectedFile, &COptions::ProcessHardlinks }) ||
        (COptions::ScanForDuplicates && (fileHashAlgorithm.Changed() ||
            BindsChanged(checkboxes, { &COptions::SampleLargeFiles })));


bool expectedFull=(mask&((1u<<4)|(1u<<5)|(1u<<6)|(1u<<9)|(1u<<10)|(1u<<11))) || (active&&(mask&((1u<<12)|(1u<<13))));
bool expectedReparse=mask&((1u<<0)|(1u<<1)|(1u<<2)|(1u<<8));
if(refreshAll!=expectedFull||refreshReparsepoints!=expectedReparse)return 2;
checks++;
}
std::cout<<checks;
}
