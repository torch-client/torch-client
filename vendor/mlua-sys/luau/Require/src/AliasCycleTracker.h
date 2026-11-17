#pragma once

#include "Luau/DenseHash.h"

#include <optional>
#include <string>
#include <vector>

namespace Luau::Require
{

class AliasCycleTracker
{
public:
    std::optional<std::string> add(std::string alias);

private:
    std::string getStringifiedCycle(const std::string& repeated) const;

    DenseHashSet<std::string> seen{""};
    std::vector<std::string> ordered;
};

}
