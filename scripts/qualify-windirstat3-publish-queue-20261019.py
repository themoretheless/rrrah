"""Original single-consumer queue body with explicit alignment portability adapter."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-publish-queue-20261019'
W.mkdir(exist_ok=False)
header=S/'windirstat/BlockingQueue.h'
original=header.read_text(encoding='utf-8-sig').split('template<typename T>\nclass SingleConsumerQueue final',1)[1]
body='template<typename T>\nclass SingleConsumerQueue final'+original
assert body.count('std::hardware_destructive_interference_size')==2
adapted=body.replace('std::hardware_destructive_interference_size','64')
probe=W/'probe.cpp'
probe.write_text('''#include <atomic>
#include <cstdint>
#include <thread>
#include <type_traits>
#include <utility>
#include <vector>
#include <iostream>
'''+adapted+'''
int main() {
  uint64_t total=0;
  for(int producers: {1,2,4,8}) for(int round=0;round<3;round++) {
    SingleConsumerQueue<uint64_t> queue;
    std::vector<std::thread> threads;
    const uint64_t count=5000;
    for(int p=0;p<producers;p++) threads.emplace_back([&,p]{
      for(uint64_t i=0;i<count;i++) queue.push((uint64_t(p)<<32)|i);
    });
    std::vector<uint64_t> next(producers,0);
    uint64_t received=0,value=0;
    while(received<count*producers) {
      if(!queue.pop(value)) { std::this_thread::yield(); continue; }
      auto p=value>>32, i=value&0xffffffffULL;
      if(p>=uint64_t(producers)||i!=next[p]++) return 2;
      received++;
    }
    for(auto& thread:threads) thread.join();
    if(!queue.empty()||queue.pop(value)) return 3;
    queue.push(uint64_t(42)); queue.clear();
    if(!queue.empty()||queue.pop(value)) return 4;
    queue.push(uint64_t(99));
    if(!queue.pop(value)||value!=99||!queue.empty()) return 5;
    total+=received;
  }
  std::cout<<total<<"\\n";
}
''')
command=['clang++','-O2','-std=c++20','-pthread',str(probe),'-o',str(W/'probe')]
build=subprocess.run(command,capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=30)
assert run.returncode==0,run.stderr
assert int(run.stdout)==225000
names=['windirstat/BlockingQueue.h','windirstat/SmartPointer.h']
result=dict(status='passed_original_publish_queue_with_alignment_adapter',
    source_pins={n:hashlib.sha256((S/n).read_bytes()).hexdigest() for n in names},
    original_class_sha256=hashlib.sha256(body.encode()).hexdigest(),
    adapted_class_sha256=hashlib.sha256(adapted.encode()).hexdigest(),
    messages_checked=225000,producer_counts=[1,2,4,8],rounds_each=3,
    invariants=['every producer sequence delivered exactly once in order','queue empty after joined producers',
                'quiescent clear empties queue','queue reusable after clear'],
    findings=['Original SingleConsumerQueue supports multiple producers and one consumer; clear explicitly unsafe with active producers.',
              'Publication node allocation occurs inside noexcept push; allocation failure is not a recoverable return in this interface.',
              'BlockingQueue cancellation joins workers and retains cancelled state until reset; ResetQueue clears thread objects while holding mutexes, requiring correct lifecycle ordering.',
              'ReadFileContent excerpt serializes shell-backed streams via one mutex and supports explicit seek offsets; not independent file identity validation.'],
    limits=['Exact SingleConsumerQueue class body extracted; two alignment constants replaced by64 for portable compilation, no algorithm edits.',
            'Not full BlockingQueue/Windows CancelSynchronousIo, SmartPointer runtime or Windows application qualification.',
            'No sanitizer/performance/all-interleaving guarantee; only stated deterministic sequence invariants under actual native threads.',
            'Two full headers read; HelpersTasks.cpp only ReadFileContent excerpt, not whole-file credit.',
            'No foreign production code adopted; native pHash investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'messages':225000,'producer_configurations':4,'rounds_each':3}))
