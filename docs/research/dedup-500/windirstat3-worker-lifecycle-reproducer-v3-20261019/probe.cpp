#include <atomic>
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
template <typename T>
class BlockingQueue final
{
    std::mutex m_threadMutex;
    std::vector<std::jthread> m_threads;
    std::deque<T> m_queue;
    std::mutex m_mutex;
    std::condition_variable m_pushed;
    std::condition_variable m_waiting;
    unsigned int m_totalWorkerThreads = 1;
    unsigned int m_workersWaiting = 0;
    unsigned int m_stopReason = 0;
    bool m_started = false;
    bool m_suspended = false;
    bool m_cancelled = false;
    bool m_exitOnAllIdle = true;

    bool AllThreadsIdling() const noexcept
    {
        return m_totalWorkerThreads == m_workersWaiting;
    }

public:
    BlockingQueue(const BlockingQueue&) = delete;
    BlockingQueue(BlockingQueue&&) = delete;
    BlockingQueue& operator=(const BlockingQueue&) = delete;
    BlockingQueue& operator=(BlockingQueue&&) = delete;
    ~BlockingQueue() = default;
    BlockingQueue(const bool exitOnAllIdle = true) : m_exitOnAllIdle(exitOnAllIdle) {}

    void ThreadWrapper(const std::function<void()>& callback)
    {
        try
        {
            callback();
        }
        catch (std::exception&)
        {
            // Exception caught from a long-running task or cancellation
            std::scoped_lock lock(m_mutex);
            m_workersWaiting++;
            m_waiting.notify_all();
        }
    }

    void StartThreads(const unsigned int workerThreads, const std::function<void()>& callback)
    {
        ResetQueue(workerThreads, false);
        std::scoped_lock threadLock(m_threadMutex);

        for ([[maybe_unused]] const auto _ : std::views::iota(0u, m_totalWorkerThreads))
        {
            m_threads.emplace_back(&BlockingQueue::ThreadWrapper, this, callback);
        }
    }

    void Push(T const& value)
    {
        // Push another entry onto the queue
        std::scoped_lock lock(m_mutex);
        m_queue.push_front(value);
        m_pushed.notify_one();
    }

    void Push(T&& value)
    {
        // Push another entry onto the queue (move semantics)
        std::scoped_lock lock(m_mutex);
        m_queue.push_front(std::move(value));
        m_pushed.notify_one();
    }

    std::optional<T> Pop()
    {
        // Record that the worker is waiting for an item until
        // the queue has something in it and we are not suspended
        std::unique_lock lock(m_mutex);
        m_workersWaiting++;
        m_waiting.notify_all();

        // Check if all workers are waiting and queue is empty - time to exit
        if (m_started && AllThreadsIdling() && m_queue.empty() && m_exitOnAllIdle)
        {
            m_cancelled = true;
            m_pushed.notify_all();
            return std::nullopt;
        }

        m_pushed.wait(lock, [&]
        {
            return !m_suspended && !m_queue.empty() || m_cancelled;
        });

        if (m_cancelled)
        {
            // Abort and signal other threads
            m_pushed.notify_all();
            return std::nullopt;
        }

        // Worker now has something to work on so pop it off the queue
        m_workersWaiting--;
        m_started = true;
        T i = std::move(m_queue.front());
        m_queue.pop_front();
        return i;
    }

    void PushIfNotQueued(T const& value)
    {
        std::scoped_lock lock(m_mutex);
        if (std::ranges::contains(m_queue, value)) return;
        m_queue.push_back(value);
        m_pushed.notify_one();
    }

    void PushIfNotQueued(T&& value)
    {
        std::scoped_lock lock(m_mutex);
        if (std::ranges::contains(m_queue, value)) return;
        m_queue.push_back(std::move(value));
        m_pushed.notify_one();
    }

    bool IsPauseOrCancelRequested()
    {
        std::scoped_lock lock(m_mutex);
        return m_suspended || m_cancelled;
    }

    void WaitIfSuspended()
    {
        // wait until not suspended or it's cancelled
        std::unique_lock lock(m_mutex);
        if (!m_suspended && !m_cancelled) return;
        m_workersWaiting++;
        m_waiting.notify_all();
        m_waiting.wait(lock, [&]
        {
            return !m_suspended || m_cancelled;
        });
        m_workersWaiting--;

        // if cancelled then throw to terminate current task
        if (m_cancelled)
        {
            throw std::runtime_error(__FUNCTION__);
        }
    }

    int WaitForCompletion()
    {
        // Wait for all worker threads to be idled or cancelled
        std::unique_lock lock(m_mutex);
        m_waiting.wait(lock, [&]
        {
            return m_started && !m_suspended && AllThreadsIdling() && m_queue.empty() || m_cancelled;
        });

        return m_stopReason;
    }

    void CancelThreadIo()
    {
        std::scoped_lock threadLock(m_threadMutex);
        for (auto& thread : m_threads)
        {
            if (thread.joinable())
                CancelSynchronousIo(thread.native_handle());
        }
    }

    void JoinThreads()
    {
        std::scoped_lock threadLock(m_threadMutex);
        for (auto& thread : m_threads)
        {
            if (thread.joinable()) thread.join();
        }
    }

    void CancelExecution(const int stopReason = -1)
    {
        // Start cancellation process
        if (std::scoped_lock lock(m_mutex); true)
        {
            if (stopReason != -1) m_stopReason = stopReason;
            m_cancelled = true;
            m_waiting.notify_all();
            m_pushed.notify_all();
        }

        // Wait for threads to complete
        JoinThreads();

        // Keep completion observable until StartThreads resets the queue.
        std::scoped_lock lock(m_threadMutex, m_mutex);
        m_threads.clear();
        m_queue.clear();
    }

    void SuspendExecution(const bool clearQueue = false)
    {
        std::unique_lock lock(m_mutex);
        if (!m_started) return;
        m_suspended = true;
        m_waiting.notify_all();
        m_waiting.wait(lock, [&]
        {
            return AllThreadsIdling();
        });
        if (clearQueue) m_queue.clear();
    }

    void ResumeExecution()
    {
        std::scoped_lock lock(m_mutex);
        m_suspended = false;
        m_waiting.notify_all();
        m_pushed.notify_all();
    }

    void ResetQueue(const int totalWorkerThreads, const bool clearQueue = true)
    {
        std::scoped_lock lock(m_threadMutex, m_mutex);
        m_workersWaiting = 0;
        m_suspended = false;
        m_started = false;
        m_cancelled = false;
        m_stopReason = 0;
        m_totalWorkerThreads = totalWorkerThreads;
        m_threads.clear();
        m_threads.reserve(m_totalWorkerThreads);
        if (clearQueue) m_queue.clear();
    }
};


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
  std::cout<<delivered<<" "<<processed<<"\n";
}
