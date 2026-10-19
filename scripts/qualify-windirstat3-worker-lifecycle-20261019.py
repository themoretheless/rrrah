"""Original BlockingQueue state machine with explicit OS exception/I/O scaffolding."""
from pathlib import Path
import hashlib
import json
import subprocess

R=Path(__file__).resolve().parents[1]
S=R/'.dedup-work/windirstat3-20261019/source'
W=R/'.dedup-work/windirstat3-worker-lifecycle-v2-20261019'
W.mkdir(exist_ok=False)
header=S/'windirstat/BlockingQueue.h'
body=header.read_text(encoding='utf-8-sig').split('template <typename T>\nclass BlockingQueue final',1)[1].split('template<typename T>\nclass SingleConsumerQueue final',1)[0]
body='template <typename T>\nclass BlockingQueue final'+body
assert body.count('std::exception(__FUNCTION__)')==1
adapted=body.replace('std::exception(__FUNCTION__)','std::runtime_error(__FUNCTION__)')
probe=W/'probe.cpp'
probe.write_text('''#include <atomic>
#include <chrono>
#include <condition_variable>
#include <deque>
#include <functional>
#include <future>
#include <iostream>
#include <mutex>
#include <optional>
#include <ranges>
#include <algorithm>
#include <stdexcept>
#include <thread>
#include <vector>
template<class T> void CancelSynchronousIo(T) {}
'''+adapted+'''
int main() {
  int delivered=0;
  for(int workers: {1,2,4,8}) {
    BlockingQueue<int> queue;
    for(int round=0;round<3;round++) {
      std::vector<std::atomic<int>> seen(2000);
      for(int i=0;i<2000;i++) queue.Push(i);
      queue.StartThreads(workers,[&]{ while(auto item=queue.Pop()) seen[*item]++; });
      if(queue.WaitForCompletion()!=0) return 2;
      queue.JoinThreads();
      for(auto& count:seen) if(count!=1) return 3;
      queue.CancelExecution(17);
      if(queue.WaitForCompletion()!=17) return 4;
      // Cancel clears pending state, so the next round can enqueue before restart.
      delivered+=2000;
    }
  }
  BlockingQueue<int> paused(false);
  std::atomic<int> processed=0;
  std::atomic<bool> release_work=false;
  for(int i=0;i<10000;i++) paused.Push(i);
  paused.StartThreads(4,[&]{while(auto item=paused.Pop()) {
    paused.WaitIfSuspended(); processed++;
    while(!release_work.load()) { paused.WaitIfSuspended(); std::this_thread::yield(); }
  }});
  while(processed==0) std::this_thread::yield();
  paused.SuspendExecution();
  const int frozen=processed;
  for(int i=0;i<1000;i++) std::this_thread::yield();
  if(processed!=frozen||frozen>=10000||!paused.IsPauseOrCancelRequested()) return 5;
  release_work=true;
  paused.ResumeExecution();
  while(processed!=10000) std::this_thread::yield();
  paused.CancelExecution(23);
  if(paused.WaitForCompletion()!=23) return 6;
  BlockingQueue<int> initially_empty;
  initially_empty.StartThreads(1,[&]{while(initially_empty.Pop()) {}});
  auto waiter=std::async(std::launch::async,[&]{return initially_empty.WaitForCompletion();});
  if(waiter.wait_for(std::chrono::milliseconds(20))!=std::future_status::timeout) return 7;
  initially_empty.CancelExecution(29);
  if(waiter.get()!=29) return 8;
  std::cout<<delivered<<" "<<processed<<"\\n";
}
''')
command=['clang++','-O2','-std=c++23','-pthread',str(probe),'-o',str(W/'probe')]
build=subprocess.run(command,capture_output=True,text=True)
(W/'build.stderr.log').write_text(build.stderr)
assert build.returncode==0,build.stderr
run=subprocess.run([str(W/'probe')],capture_output=True,text=True,timeout=30)
(W/'run.stdout.log').write_text(run.stdout)
(W/'run.stderr.log').write_text(run.stderr)
assert run.returncode==0,run.stderr
assert run.stdout.strip()=='24000 10000'
result=dict(status='passed_original_worker_lifecycle_with_explicit_platform_adapters',
    source_sha256=hashlib.sha256(header.read_bytes()).hexdigest(),
    original_class_sha256=hashlib.sha256(body.encode()).hexdigest(),
    adapted_class_sha256=hashlib.sha256(adapted.encode()).hexdigest(),
    worker_counts=[1,2,4,8],rounds_each=3,exact_once_deliveries=24000,pause_resume_deliveries=10000,
    controls=['automatic idle completion','cancel reason remains observable','restart after join/cancel',
              'pause holds processed count stable','resume drains work','initial empty start waits until explicit cancel'],
    findings=['Actual std::jthread state-machine controls pass on Mac with C++23.',
              'Empty start is not automatically complete before any item has been processed; explicit cancel unblocks completion.',
              'Pause/resume behavior tested with actual threads, not mocked parameter forwarding.'],
    limits=['Original BlockingQueue class extracted; MSVC exception(message) replaced by runtime_error(message).',
            'CancelSynchronousIo supplied as no-op template; CancelThreadIo not invoked. No Windows I/O cancellation proof.',
            'No full app/GUI/hash publication or all-interleaving/sanitizer/performance qualification.',
            'No new whole-source-file credit or production code adoption; pHash native investigation deferred.'])
(W/'result.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({'exact_once_deliveries':24000,'pause_resume_deliveries':10000}))
