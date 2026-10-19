#include <cstdint>
#include <vector>
#include <unordered_map>
#include <iostream>
using ULONGLONG=uint64_t;
struct FinderNtfsContext {
static constexpr ULONGLONG NtfsRecordMask = (1ull << 48) - 1;
 struct Base {int marker;}; struct Name {uint64_t FileReference;};
 std::unordered_map<uint64_t,Base> m_baseFileRecordMap;
};
struct FinderNtfs {
 FinderNtfsContext* m_master;
 FinderNtfsContext::Base* m_currentRecord=nullptr;
 const FinderNtfsContext::Name* m_currentRecordName=nullptr;
 std::vector<FinderNtfsContext::Name>::const_iterator m_recordIterator,m_recordIteratorEnd;
 uint64_t m_index=0;
 FinderNtfs(FinderNtfsContext* ctx,const std::vector<FinderNtfsContext::Name>& names):
 m_master(ctx),m_recordIterator(names.begin()),m_recordIteratorEnd(names.end()){}
 bool FindNext();
};
bool FinderNtfs::FindNext()
{
    if (m_recordIterator == m_recordIteratorEnd) return false;
    m_index = m_recordIterator->FileReference & FinderNtfsContext::NtfsRecordMask;
    const auto it = m_master->m_baseFileRecordMap.find(m_index);
    if (it == m_master->m_baseFileRecordMap.end()) return false;
    m_currentRecord = &it->second;
    m_currentRecordName = &(*m_recordIterator);
    ++m_recordIterator;

    return true;
}


int main() {
 FinderNtfsContext ctx;
 ctx.m_baseFileRecordMap[7]={70};ctx.m_baseFileRecordMap[8]={80};
 std::vector<FinderNtfsContext::Name> valid={{(uint64_t(3)<<48)|7},{(uint64_t(4)<<48)|8}};
 FinderNtfs normal(&ctx,valid);
 if(!normal.FindNext()||normal.m_index!=7||normal.m_currentRecord->marker!=70)return 2;
 if(normal.m_currentRecordName->FileReference!=valid[0].FileReference)return 3;
 if(!normal.FindNext()||normal.m_index!=8||normal.FindNext())return 4;
 std::vector<FinderNtfsContext::Name> empty;
 FinderNtfs none(&ctx,empty);if(none.FindNext())return 5;
 std::vector<FinderNtfsContext::Name> missing={{9},{7}};
 FinderNtfs absent(&ctx,missing);
 if(absent.FindNext()||absent.m_recordIterator!=missing.begin())return 6;
 if(absent.FindNext())return 7;
 std::vector<FinderNtfsContext::Name> middle={{7},{9},{8}};
 FinderNtfs partial(&ctx,middle);
 if(!partial.FindNext()||partial.FindNext()||partial.m_recordIterator!=middle.begin()+1)return 8;
 std::cout<<8;
}
