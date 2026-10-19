"""Original ApplyPages controller with captured pages/dialog calls and real C++ execution."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-settings-apply-v2-20261019'
W.mkdir(exist_ok=False)
source=S/'windirstat/UiDialogs.cpp'
text=source.read_text(encoding='utf-8-sig')
method='bool CPropertySheet::ApplyPages'+text.split('bool CPropertySheet::ApplyPages',1)[1].split('HBRUSH CPropertySheet::OnCtlColor',1)[0]
method=method.replace('for (const auto [index, page] : std::views::enumerate(m_pages))\n    {', 'for (size_t index=0; index<m_pages.size(); ++index)\n    {\n        auto* page=m_pages[index];')
probe=W/'probe.cpp'
probe.write_text('''#include <vector>
#include <string>
#include <optional>
#include <ranges>
#include <iostream>
using UINT=unsigned; constexpr int MB_OK=0,MB_ICONWARNING=0;
void SetFocus(void*) {} template<class... T> void ShowMessageBox(T...) {}
struct ScopedValue {bool& target;bool old; ScopedValue(bool& v,bool n):target(v),old(v){v=n;}~ScopedValue(){target=old;}};
std::vector<std::string> logEvents;
struct Page {
 int id; bool invalid=false;bool m_bModified=true;
 void* Handle(){return this;}
 struct Error {void* control;std::string message;};
 std::optional<Error> PrepareApply(){logEvents.push_back("prepare"+std::to_string(id));if(invalid)return Error{this,"bad"};return {};}
 void BeforeApply(){logEvents.push_back("before"+std::to_string(id));}
 void CommitApply(){logEvents.push_back("commit"+std::to_string(id));}
 void AfterApply(){logEvents.push_back("after"+std::to_string(id));}
};
struct CPropertySheet {
 bool m_applying=false,confirmed=true;void* m_hWnd=nullptr;std::string m_caption;
 std::vector<Page*> m_pages;
 void SelectPage(int) {logEvents.push_back("error");}
 bool ConfirmApply(UINT){logEvents.push_back("confirm");return confirmed;}
 void UpdateApplyButton(){logEvents.push_back("button");}
 void OnApplied(UINT){logEvents.push_back("applied");}
 bool ApplyPages(UINT);
};
'''+method+'''
int main(){
 int controls=0;
 for(int invalid=0;invalid<3;invalid++) {
  Page a{0},b{1},c{2};CPropertySheet sheet;sheet.m_pages={&a,&b,&c};
  sheet.m_pages[invalid]->invalid=true;logEvents.clear();
  if(sheet.ApplyPages(0)||sheet.m_applying)return 2;
  for(auto& event:logEvents) if(event.starts_with("commit")||event.starts_with("before")||event.starts_with("after"))return 3;
  if(!a.m_bModified||!b.m_bModified||!c.m_bModified)return 4;
  controls++;
 }
 Page a{0},b{1},c{2};CPropertySheet sheet;sheet.m_pages={&a,&b,&c};
 sheet.confirmed=false;logEvents.clear();
 if(sheet.ApplyPages(0)||sheet.m_applying||logEvents.back()!="confirm")return 5;
 controls++;
 sheet.confirmed=true;logEvents.clear();
 if(!sheet.ApplyPages(0)||sheet.m_applying)return 6;
 std::vector<std::string> expected={"prepare0","prepare1","prepare2","confirm","before0","before1","before2","commit0","commit1","commit2","button","after0","after1","after2","applied"};
 if(logEvents!=expected||a.m_bModified||b.m_bModified||c.m_bModified)return 7;
 controls++;
 sheet.m_applying=true;logEvents.clear();
 if(sheet.ApplyPages(0)||!logEvents.empty())return 8;
 controls++;
 std::cout<<controls;
}
''')
build=subprocess.run(['clang++','-O2','-std=c++23',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=10)
assert run.returncode==0,run.stderr
names=['windirstat/Pages/PageShared.cpp','windirstat/Pages/PageShared.h']
result=dict(status='passed_original_apply_controller_with_captured_pages',controls=int(run.stdout),
    whole_files_read={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in names},
    partial_source={'path':'windirstat/UiDialogs.cpp','sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'scope':'ApplyPages and surrounding excerpt, not whole file'},
    findings=['All page PrepareApply validation precedes confirmation, BeforeApply and commit; invalid first/middle/last page triggers no before/commit/effect.',
              'Successful path runs all BeforeApply calls before any CommitApply, then all commits before effects.',
              'Cancellation at ConfirmApply leaves staged writes uncommitted; next PrepareApply clears stale staged callbacks.',
              'Reentrant ApplyPages call rejected by guard; no exception rollback guarantee for failures during commit/effect inferred.'],
    limits=['Original ApplyPages controller with enumerate loop replaced by equivalent indexed loop for libc++ compatibility; synthetic page objects/captured UI calls and ScopedValue equivalent scaffolding.',
            'No actual Windows controls, persistent settings, scan cancellation or exception-atomic commit proof.',
            'Two whole shared-settings files read; UiDialogs.cpp/UiPanes.h excerpts do not count as whole-file review.',
            'No production adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'controls':int(run.stdout)}))
