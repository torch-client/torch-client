#pragma once

namespace Luau
{

template<typename SetType, typename Key>
struct ScopedSeenSet
{
    SetType& seen;
    Key key;

    ScopedSeenSet(SetType& seen, Key key)
        : seen(seen)
        , key(key)
    {
        seen.insert(key);
    }

    ~ScopedSeenSet()
    {
        seen.erase(key);
    }

    ScopedSeenSet(const ScopedSeenSet&) = delete;
    ScopedSeenSet& operator=(const ScopedSeenSet&) = delete;
};

}
