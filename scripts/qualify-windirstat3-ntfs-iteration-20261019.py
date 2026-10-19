"""Original NTFS FindNext method on explicit synthetic index maps; no disk parser execution."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-ntfs-iteration-20261019'
W.mkdir(exist_ok=False)
source=S/'windirstat/FinderNtfs.cpp';header=S/'windirstat/FinderNtfs.h'
text=source.read_text(encoding='utf-8-sig')
method='bool FinderNtfs::FindNext()'+text.split('bool FinderNtfs::FindNext()',1)[1].split('bool FinderNtfs::FindFile(',1)[0]
mask=next(line.strip() for line in header.read_text(encoding='utf-8-sig').splitlines() if 'static constexpr ULONGLONG NtfsRecordMask' in line)
probe=W/'probe.cpp'
probe.write_text('''#include <cstdint>
#include <vector>
#include <unordered_map>
#include <iostream>
using ULONGLONG=uint64_t;
struct FinderNtfsContext {
'''+mask+'''
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
'''+method+'''
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
''')
build=subprocess.run(['clang++','-O2','-std=c++20',str(probe),'-o',str(W/'probe')],capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=10)
assert run.returncode==0,run.stderr
names=['windirstat/FinderNtfs.cpp','windirstat/FinderNtfs.h']
result=dict(status='passed_original_NTFS_iterator_characterization',controls=8,
    source_pins={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in names},
    findings=['Original FindNext masks48bit record index for base-map lookup while retaining full sequence-bearing reference on name record.',
              'Synthetic incomplete base map: missing first/middle child returns false without advancing iterator, so later valid siblings are not reached by normal iteration.',
              'This demonstrates behavior given inconsistent maps, not proof a valid Windows scan creates that state.',
              'LoadRoot validates record signature/fixup-array placement and applies sector fixups; attribute payload offsets/lengths need separate qualification.'],
    limits=['Original FindNext method and mask compiled unchanged with synthetic record/map/iterator structs.',
            'No actual MFT parser, malformed disk data, Windows volume, concurrent filesystem mutation or hardlink dedup run.',
            'Two whole NTFS source/header files read; no whole-repository/runtime qualification.',
            'No production adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'controls':8,'incomplete_map_stops_before_later_valid_sibling':True}))
