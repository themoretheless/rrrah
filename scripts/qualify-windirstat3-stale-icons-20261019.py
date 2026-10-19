"""Original UI result-admission statements with captured control and synthetic item identity."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-stale-icons-20261019'
W.mkdir(exist_ok=False)
path=S/'windirstat/Controls/IconHandler.cpp'
text=path.read_text(encoding='utf-8-sig')
body=text.split('const auto pending = m_pendingLookups.find(item);',1)[1].split('                });',1)[0]
body='const auto pending = m_pendingLookups.find(item);'+body
probe=W/'probe.cpp'
probe.write_text('''#include <cstdint>
#include <map>
#include <string>
#include <iostream>
struct Item { bool visible; bool IsVisible() const {return visible;} };
struct Control { int found; int redraw=0; int FindListItem(Item*) {return found;}
  void RedrawItems(int,int) {redraw++;} };
std::map<Item*,uint64_t> m_pendingLookups;
void deliver(Item* item,Control* control,uint64_t requestId,int* icon,std::wstring* desc) {
  const int iconTmp=99; const std::wstring descTmp=L"new";
'''+body+'''
}
int main() {
 int controls=0;
 for(bool pending: {false,true}) for(bool matching: {false,true})
 for(bool visible: {false,true}) for(bool found: {false,true}) {
   Item item{visible}; Control control{found?3:-1};
   m_pendingLookups.clear(); if(pending) m_pendingLookups[&item]=matching?42:43;
   int icon=1; std::wstring desc=L"old";
   deliver(&item,&control,42,&icon,&desc);
   bool accepted=pending&&matching&&visible&&found;
   if((icon==99)!=accepted||(desc==L"new")!=accepted||control.redraw!=int(accepted)) return 2;
   bool remains=pending&&!matching;
   if(m_pendingLookups.contains(&item)!=remains) return 3;
   controls++;
 }
 // Same item address, newer request: old completion must not consume current identity.
 Item reused{true}; Control control{0}; int icon=1;
 m_pendingLookups[&reused]=200;
 deliver(&reused,&control,100,&icon,nullptr);
 if(icon!=1||m_pendingLookups[&reused]!=200) return 4;
 deliver(&reused,&control,200,&icon,nullptr);
 if(icon!=99||m_pendingLookups.contains(&reused)) return 5;
 std::cout<<controls+2<<"\\n";
}
''')
build=subprocess.run(['clang++','-O2','-std=c++20',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=10)
assert run.returncode==0 and run.stdout.strip()=='18',run.stderr
actions=S/'windirstat/WinDirStatModel.Actions.cpp'
stop=actions.read_text(encoding='utf-8-sig').split('void CWinDirStatModel::StopScanningEngine(',1)[1].split('void CWinDirStatModel::OnContextMenuExplore',1)[0]
assert stop.index('CancelThreadIo()')<stop.index('SuspendExecution()')<stop.index('CancelExecution(stopReason)')<stop.index('m_thread.join()')
names=['windirstat/Controls/IconHandler.cpp','windirstat/Controls/IconHandler.h']
result=dict(status='passed_original_stale_result_admission_with_captured_UI',
    source_pins={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in names},
    extracted_body_sha256=hashlib.sha256(body.encode()).hexdigest(),admission_controls=18,
    findings=['Request identity must match pending item before writing icon/description; old completion at reused item address rejected without consuming newer request.',
              'Matched but hidden/removed-list item consumes pending identity without publishing a UI update.',
              'Read stop callsite orders I/O interruption, suspend, cancellation/join, wrapper join; aligns with prior queue lifecycle preconditions.',
              'Offline icon requests use synthetic extension-only path to avoid opening original cloud placeholder; actual shell behavior not run.'],
    limits=['Exact original delivery statements compiled with synthetic Item/Control/icon tokens; no real Windows window, shell call or freed pointer execution.',
            'Actions.cpp scan suspend/resume/stop/start excerpts only; no whole-file credit or runtime cancellation proof.',
            'Two complete icon source/header files read, but GDI+ rendering and shell icon ownership not runtime-qualified.',
            'No production adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'admission_controls':18,'stop_call_order_checked':True}))
