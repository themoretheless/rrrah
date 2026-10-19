#include <atomic>
#include <cstdint>
#include <thread>
#include <type_traits>
#include <utility>
#include <vector>
#include <iostream>
template<typename T>
class SingleConsumerQueue final
{
    struct Node
    {
        std::atomic<Node*> next{ nullptr };
        T data{};

        Node() = default;

        template<typename U>
        explicit Node(U&& value) : data(std::forward<U>(value)) {}
    };

    alignas(64) std::atomic<Node*> m_head;
    alignas(64) Node* m_tail;

public:
    SingleConsumerQueue()
    {
        Node* dummy = new Node();
        m_tail = dummy;
        m_head.store(dummy, std::memory_order_relaxed);
    }

    ~SingleConsumerQueue()
    {
        for (T tmp; pop(tmp);) {}
        delete m_tail;
    }

    template<typename U>
    void push(U&& item) noexcept
    {
        Node* node = new Node(std::forward<U>(item));
        Node* prev = m_head.exchange(node, std::memory_order_release);
        prev->next.store(node, std::memory_order_release);
    }

    bool pop(T& item) noexcept(std::is_nothrow_move_assignable_v<T>)
    {
        Node* t = m_tail;
        Node* next = t->next.load(std::memory_order_acquire);

        if (next == nullptr)
        {
            if (m_head.load(std::memory_order_acquire) == t)
                return false;

            while ((next = t->next.load(std::memory_order_acquire)) == nullptr)
            {
                std::this_thread::yield();
            }
        }

        item = std::move(next->data);
        m_tail = next;

        delete t;
        return true;
    }

    // Only valid for consumer
    bool empty() const noexcept
    {
        if (m_tail->next.load(std::memory_order_acquire) != nullptr) return false;
        return m_head.load(std::memory_order_acquire) == m_tail;
    }

    // Clear out queue (unsafe with concurrent producers)
    void clear() noexcept
    {
        for (T tmp; pop(tmp);) {}

        const Node* old = m_tail; // One dummy remains
        Node* dummy = new Node();

        m_tail = dummy;
        m_head.store(dummy, std::memory_order_release);

        delete old;
    }
};

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
  std::cout<<total<<"\n";
}
