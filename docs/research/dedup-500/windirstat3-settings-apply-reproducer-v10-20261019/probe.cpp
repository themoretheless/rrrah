#include <vector>
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
bool CPropertySheet::ApplyPages(const UINT command)
{
    if (m_applying) return false;
    const ScopedValue applying(m_applying, true);
    for (size_t index=0; index<m_pages.size(); ++index)
    {
        auto* page=m_pages[index];
        if (page->Handle() == nullptr) continue;
        if (const auto error = page->PrepareApply())
        {
            SelectPage(static_cast<int>(index));
            ShowMessageBox(m_hWnd, error->message, m_caption, MB_OK | MB_ICONWARNING);
            ::SetFocus(error->control);
            return false;
        }
    }
    if (!ConfirmApply(command)) return false;
    for (const auto& page : m_pages) if (page->Handle() != nullptr) page->BeforeApply();
    for (const auto& page : m_pages) if (page->Handle() != nullptr) page->CommitApply();
    for (const auto& page : m_pages) page->m_bModified = false;
    UpdateApplyButton();
    for (const auto& page : m_pages) if (page->Handle() != nullptr) page->AfterApply();
    OnApplied(command);
    return true;
}


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
