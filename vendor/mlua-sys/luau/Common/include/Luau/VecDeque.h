#pragma once

#include "Luau/Common.h"

#include <algorithm>
#include <limits>
#include <memory>
#include <new>
#include <stdexcept>
#include <type_traits>
#include <utility>

namespace Luau
{
template<typename T, class Allocator = std::allocator<T>>
class VecDeque : Allocator
{
private:
    static_assert(std::is_nothrow_move_constructible_v<T>);
    static_assert(std::is_nothrow_move_assignable_v<T>);

    T* buffer = nullptr;
    size_t buffer_capacity = 0;

    size_t head = 0;
    size_t queue_size = 0;

    void destroyElements() noexcept
    {
        size_t head_size =
            std::min(queue_size, capacity() - head);
        size_t tail_size = queue_size - head_size;

        for (size_t index = head; index < head + head_size; index++)
            buffer[index].~T();

        for (size_t index = 0; index < tail_size; index++)
            buffer[index].~T();
    }

    bool is_full()
    {
        return queue_size == capacity();
    }

    void grow()
    {
        size_t old_capacity = capacity();

        size_t new_capacity = (old_capacity > 0) ? old_capacity * 3 / 2 + 1 : 4;

        if (new_capacity > max_size())
            throw std::bad_array_new_length();

        T* new_buffer = this->allocate(new_capacity);

        LUAU_ASSERT(old_capacity == queue_size);

        size_t head_size =
            std::min(queue_size, old_capacity - head);
        size_t tail_size = queue_size - head_size;

        if (head_size != 0)
            std::uninitialized_move(buffer + head, buffer + head + head_size, new_buffer);

        if (tail_size != 0)
            std::uninitialized_move(buffer, buffer + tail_size, new_buffer + head_size);

        destroyElements();
        this->deallocate(buffer, old_capacity);

        buffer = new_buffer;
        buffer_capacity = new_capacity;
        head = 0;
    }

    size_t logicalToPhysical(size_t pos)
    {
        return (head + pos) % capacity();
    }

public:
    VecDeque() = default;

    explicit VecDeque(const Allocator& alloc) noexcept
        : Allocator{alloc}
    {
    }

    VecDeque(const VecDeque& other)
        : buffer(this->allocate(other.buffer_capacity))
        , buffer_capacity(other.buffer_capacity)
        , head(other.head)
        , queue_size(other.queue_size)
    {
        size_t head_size = std::min(
            other.queue_size,
            other.buffer_capacity - other.head
        );
        size_t tail_size = other.queue_size - head_size;

        if (head_size != 0)
            std::uninitialized_copy(other.buffer + other.head, other.buffer + other.head + head_size, buffer + head);

        if (tail_size != 0)
            std::uninitialized_copy(other.buffer, other.buffer + tail_size, buffer);
    }

    VecDeque(const VecDeque& other, const Allocator& alloc)
        : Allocator{alloc}
        , buffer(this->allocate(other.buffer_capacity))
        , buffer_capacity(other.buffer_capacity)
        , head(other.head)
        , queue_size(other.queue_size)
    {
        size_t head_size = std::min(
            other.queue_size,
            other.buffer_capacity - other.head
        );
        size_t tail_size = other.queue_size - head_size;

        if (head_size != 0)
            std::uninitialized_copy(other.buffer + other.head, other.buffer + other.head + head_size, buffer + head);

        if (tail_size != 0)
            std::uninitialized_copy(other.buffer, other.buffer + tail_size, buffer);
    }

    VecDeque(VecDeque&& other) noexcept
        : buffer(std::exchange(other.buffer, nullptr))
        , buffer_capacity(std::exchange(other.buffer_capacity, 0))
        , head(std::exchange(other.head, 0))
        , queue_size(std::exchange(other.queue_size, 0))
    {
    }

    VecDeque(VecDeque&& other, const Allocator& alloc) noexcept
        : Allocator{alloc}
        , buffer(std::exchange(other.buffer, nullptr))
        , buffer_capacity(std::exchange(other.buffer_capacity, 0))
        , head(std::exchange(other.head, 0))
        , queue_size(std::exchange(other.queue_size, 0))
    {
    }

    VecDeque(std::initializer_list<T> init, const Allocator& alloc = Allocator())
        : Allocator{alloc}
    {
        buffer = this->allocate(init.size());
        buffer_capacity = init.size();
        queue_size = init.size();

        std::uninitialized_copy(init.begin(), init.end(), buffer);
    }

    ~VecDeque() noexcept
    {
        destroyElements();
        this->deallocate(buffer, buffer_capacity);
    }

    VecDeque& operator=(const VecDeque& other)
    {
        if (this == &other)
            return *this;

        destroyElements();

        if (buffer_capacity < other.size())
        {
            this->deallocate(buffer, buffer_capacity);

            buffer = this->allocate(other.buffer_capacity);
            buffer_capacity = other.buffer_capacity;
        }

        size_t head_size = std::min(
            other.queue_size,
            other.buffer_capacity - other.head
        );
        size_t tail_size = other.queue_size - head_size;

        head = 0;
        queue_size = other.queue_size;

        if (head_size != 0)
            std::uninitialized_copy(other.buffer + other.head, other.buffer + other.head + head_size, buffer);

        if (tail_size != 0)
            std::uninitialized_copy(other.buffer, other.buffer + tail_size, buffer + head_size);

        return *this;
    }

    VecDeque& operator=(VecDeque&& other)
    {
        if (this == &other)
            return *this;

        destroyElements();
        this->deallocate(buffer, buffer_capacity);

        buffer = std::exchange(other.buffer, nullptr);
        buffer_capacity = std::exchange(other.buffer_capacity, 0);
        head = std::exchange(other.head, 0);
        queue_size = std::exchange(other.queue_size, 0);

        return *this;
    }

    Allocator get_allocator() const noexcept
    {
        return this;
    }

    T& at(size_t pos)
    {
        if (pos >= queue_size)
            throw std::out_of_range("VecDeque");

        return buffer[logicalToPhysical(pos)];
    }

    const T& at(size_t pos) const
    {
        if (pos >= queue_size)
            throw std::out_of_range("VecDeque");

        return buffer[logicalToPhysical(pos)];
    }

    [[nodiscard]] T& operator[](size_t pos) noexcept
    {
        LUAU_ASSERT(pos < queue_size);

        return buffer[logicalToPhysical(pos)];
    }

    [[nodiscard]] const T& operator[](size_t pos) const noexcept
    {
        LUAU_ASSERT(pos < queue_size);

        return buffer[logicalToPhysical(pos)];
    }

    T& front()
    {
        LUAU_ASSERT(!empty());

        return buffer[head];
    }

    const T& front() const
    {
        LUAU_ASSERT(!empty());

        return buffer[head];
    }

    T& back()
    {
        LUAU_ASSERT(!empty());

        size_t back = logicalToPhysical(queue_size - 1);
        return buffer[back];
    }

    const T& back() const
    {
        LUAU_ASSERT(!empty());

        size_t back = logicalToPhysical(queue_size - 1);
        return buffer[back];
    }

    bool empty() const noexcept
    {
        return queue_size == 0;
    }

    size_t size() const noexcept
    {
        return queue_size;
    }

    size_t max_size() const noexcept
    {
        return std::numeric_limits<size_t>::max() / sizeof(T);
    }

    void reserve(size_t new_capacity)
    {
        if (new_capacity > max_size())
            throw std::length_error("too large");

        size_t old_capacity = capacity();

        if (new_capacity <= old_capacity)
            return;

        size_t head_size =
            std::min(queue_size, old_capacity - head);
        size_t tail_size = queue_size - head_size;

        T* new_buffer = this->allocate(new_capacity);

        if (head_size != 0)
            std::uninitialized_move(buffer + head, buffer + head + head_size, new_buffer);

        if (tail_size != 0)
            std::uninitialized_move(buffer, buffer + tail_size, new_buffer + head_size);

        destroyElements();

        this->deallocate(buffer, old_capacity);

        buffer = new_buffer;
        buffer_capacity = new_capacity;
        head = 0;
    }

    size_t capacity() const noexcept
    {
        return buffer_capacity;
    }

    void shrink_to_fit()
    {
        size_t old_capacity = capacity();
        size_t new_capacity = queue_size;

        if (old_capacity == new_capacity)
            return;

        size_t head_size =
            std::min(queue_size, old_capacity - head);
        size_t tail_size = queue_size - head_size;

        T* new_buffer = this->allocate(new_capacity);

        if (head_size != 0)
            std::uninitialized_move(buffer + head, buffer + head + head_size, new_buffer);

        if (tail_size != 0)
            std::uninitialized_move(buffer, buffer + tail_size, new_buffer + head_size);

        destroyElements();
        this->deallocate(buffer, old_capacity);

        buffer = new_buffer;
        buffer_capacity = new_capacity;
        head = 0;
    }

    [[nodiscard]] bool is_contiguous() const noexcept
    {
        return head <= capacity() - queue_size;
    }

    void clear() noexcept
    {
        destroyElements();

        head = 0;
        queue_size = 0;
    }

    void push_back(const T& value)
    {
        if (is_full())
            grow();

        size_t next_back = logicalToPhysical(queue_size);
        new (buffer + next_back) T(value);
        queue_size++;
    }

    void pop_back()
    {
        LUAU_ASSERT(!empty());

        queue_size--;
        size_t next_back = logicalToPhysical(queue_size);
        buffer[next_back].~T();
    }

    void push_front(const T& value)
    {
        if (is_full())
            grow();

        head = (head == 0) ? capacity() - 1 : head - 1;
        new (buffer + head) T(value);
        queue_size++;
    }

    void pop_front()
    {
        LUAU_ASSERT(!empty());

        buffer[head].~T();
        head++;
        queue_size--;

        if (head == capacity())
            head = 0;
    }
};

}
