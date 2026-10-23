#include "cluster.hpp"
#include <iostream>
#include <algorithm>
int HashResult::hamming(const HashResult& a, const HashResult& b) {
    int dist = 0;
    int len = (int)std::min(a.bits.size(), b.bits.size());
    for (int i = 0; i < len; i++) {
        dist += __builtin_popcount(a.bits[i] ^ b.bits[i]);
    }
    return dist;
}

#include "/Users/themoretheless/Documents/Sources/rrrah/docs/research/dedup-500/evidence/Mahoskian--sift/02-distance.cpp"
static void write_groups_json(
    std::ostream& out,
    const std::vector<GroupInfo>& groups,
    const std::vector<std::string>& files,
    int min_group_filter
) {
    // Separate groups from ungrouped (singletons below min_group_filter)
    std::vector<const GroupInfo*> real_groups;
    std::vector<int> ungrouped;

    for (auto& g : groups) {
        if ((int)g.members.size() >= min_group_filter) {
            real_groups.push_back(&g);
        } else {
            for (int m : g.members) ungrouped.push_back(m);
        }
    }

    out << "  \"groups\": [\n";
    for (size_t i = 0; i < real_groups.size(); i++) {
        auto* g = real_groups[i];
        out << "    {\n";
        out << "      \"id\": " << g->id << ",\n";
        out << "      \"members\": [";
        for (size_t j = 0; j < g->members.size(); j++) {
            out << g->members[j];
            if (j + 1 < g->members.size()) out << ", ";
        }
        out << "],\n";
        out << "      \"member_files\": [";
        for (size_t j = 0; j < g->members.size(); j++) {
            out << "\"" << files[g->members[j]] << "\"";
            if (j + 1 < g->members.size()) out << ", ";
        }
        out << "],\n";
        out << "      \"max_internal_distance\": " << g->max_internal_distance << ",\n";
        out << "      \"avg_internal_distance\": " << g->avg_internal_distance << "\n";
        out << "    }";
        if (i + 1 < real_groups.size()) out << ",";
        out << "\n";
    }
    out << "  ],\n";

    out << "  \"ungrouped\": [";
    for (size_t i = 0; i < ungrouped.size(); i++) {
        out << ungrouped[i];
        if (i + 1 < ungrouped.size()) out << ", ";
    }
    out << "]";
}

static void write_distance_matrix_json(std::ostream& out, const DistanceMatrix& dm) {
    out << "  \"distance_matrix\": [\n";
    for (int i = 0; i < dm.n; i++) {
        out << "    [";
        for (int j = 0; j < dm.n; j++) {
            out << dm.get(i, j);
            if (j + 1 < dm.n) out << ", ";
        }
        out << "]";
        if (i + 1 < dm.n) out << ",";
        out << "\n";
    }
    out << "  ]";
}

static void write_stats_json(std::ostream& out, const DistanceMatrix& dm,
                              const std::vector<GroupInfo>& groups,
                              const std::vector<int>& ungrouped,
                              int min_group_filter) {
    int total_groups = 0;
    int total_ungrouped = (int)ungrouped.size();
    for (auto& g : groups) {
        if ((int)g.members.size() >= min_group_filter)
            total_groups++;
        else
            total_ungrouped += (int)g.members.size();
    }

    out << "  \"stats\": {\n";
    out << "    \"total_files\": " << dm.n << ",\n";
    out << "    \"total_groups\": " << total_groups << ",\n";
    out << "    \"total_ungrouped\": " << total_ungrouped << ",\n";
    out << "    \"min_distance\": " << dm.min_distance() << ",\n";
    out << "    \"max_distance\": " << dm.max_distance() << ",\n";
    out << "    \"avg_distance\": " << dm.avg_distance() << "\n";
    out << "  }";
}

static void write_cluster_json(
    std::ostream& out,
    const ClusterInput& input,
    const DistanceMatrix& dm,
    const std::string& method,
    const std::vector<GroupInfo>& groups,
    int min_group_filter,
    // Method-specific data (nullptr if not applicable)
    const std::vector<DendrogramStep>* dendrogram,
    const std::vector<MembershipInfo>* membership,
    const std::vector<CondensedTreeNode>* condensed_tree,
    // Params
    const std::vector<std::pair<std::string, std::string>>& params
) {
    out << "{\n";
    out << "  \"algorithm\": \"" << input.algorithm << "\",\n";
    out << "  \"hash_size\": " << input.hash_size << ",\n";
    out << "  \"hash_bits\": " << input.hash_bits << ",\n";
    out << "  \"method\": \"" << method << "\",\n";

    // Params
    out << "  \"params\": {\n";
    for (size_t i = 0; i < params.size(); i++) {
        out << "    \"" << params[i].first << "\": " << params[i].second;
        if (i + 1 < params.size()) out << ",";
        out << "\n";
    }
    out << "  },\n";

    // Files array (ordered, indices used elsewhere)
    out << "  \"files\": [";
    for (size_t i = 0; i < input.files.size(); i++) {
        out << "\"" << input.files[i] << "\"";
        if (i + 1 < input.files.size()) out << ", ";
    }
    out << "],\n";

    // Distance matrix
    write_distance_matrix_json(out, dm);
    out << ",\n";

    // Dendrogram (hierarchical only)
    if (dendrogram) {
        out << "  \"dendrogram\": [\n";
        for (size_t i = 0; i < dendrogram->size(); i++) {
            auto& d = (*dendrogram)[i];
            out << "    {\"step\": " << d.step
                << ", \"merged\": [" << d.merged_a << ", " << d.merged_b
                << "], \"distance\": " << d.distance << "}";
            if (i + 1 < dendrogram->size()) out << ",";
            out << "\n";
        }
        out << "  ],\n";
    }

    // Membership (hdbscan only)
    if (membership) {
        out << "  \"membership\": [\n";
        for (size_t i = 0; i < membership->size(); i++) {
            auto& m = (*membership)[i];
            out << "    {\"file\": " << m.file
                << ", \"group\": " << m.group
                << ", \"confidence\": " << m.confidence << "}";
            if (i + 1 < membership->size()) out << ",";
            out << "\n";
        }
        out << "  ],\n";
    }

    // Condensed tree (hdbscan only)
    if (condensed_tree) {
        out << "  \"condensed_tree\": [\n";
        for (size_t i = 0; i < condensed_tree->size(); i++) {
            auto& ct = (*condensed_tree)[i];
            out << "    {\"parent\": " << ct.parent
                << ", \"child\": " << ct.child
                << ", \"lambda\": " << ct.lambda
                << ", \"child_size\": " << ct.child_size << "}";
            if (i + 1 < condensed_tree->size()) out << ",";
            out << "\n";
        }
        out << "  ],\n";
    }

    // Groups
    // Collect ungrouped
    std::vector<int> ungrouped;
    for (auto& g : groups)
        if ((int)g.members.size() < min_group_filter)
            for (int m : g.members) ungrouped.push_back(m);

    write_groups_json(out, groups, input.files, min_group_filter);
    out << ",\n";

    // Stats
    write_stats_json(out, dm, groups, ungrouped, min_group_filter);
    out << "\n";

    out << "}\n";
}


int main(){ClusterInput in;in.algorithm="dhash";in.hash_size=2;in.hash_bits=4;in.files={"one.png"};DistanceMatrix dm;dm.n=1;dm.data={{0}};GroupInfo g;g.id=0;g.members={0};g.max_internal_distance=0;g.avg_internal_distance=0;write_cluster_json(std::cout,in,dm,"threshold",{g},2,nullptr,nullptr,nullptr,{});}
