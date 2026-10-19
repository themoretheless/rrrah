#include <cstdint>
#include <map>
#include <string>
#include <iostream>
struct Item { bool visible; bool IsVisible() const {return visible;} };
struct Control { int found; int redraw=0; int FindListItem(Item*) {return found;}
  void RedrawItems(int,int) {redraw++;} };
std::map<Item*,uint64_t> m_pendingLookups;
void deliver(Item* item,Control* control,uint64_t requestId,int* icon,std::wstring* desc) {
  const int iconTmp=99; const std::wstring descTmp=L"new";
const auto pending = m_pendingLookups.find(item);
                    if (pending == m_pendingLookups.end() || pending->second != requestId) return;
                    m_pendingLookups.erase(pending);
                    const auto i = control->FindListItem(item);
                    if (i == -1 || !item->IsVisible()) return;
                    *icon = iconTmp;
                    if (desc != nullptr) *desc = descTmp;
                    control->RedrawItems(i, i);

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
 std::cout<<controls+2<<"\n";
}
