/* Test the exact C helpers used by the rebuilt upstream firmware. */
#include <assert.h>
#include "md-sim-adaptation.h"

int main(void)
{
    unsigned char hash[32] = {0};
    assert(md_sim_hash_prefix(hash) == 0);
    for (unsigned i = 0; i < 32; ++i) {
        memset(hash, 0, sizeof(hash));
        hash[i] = 0x80;
        uint32_t expected = i <= 28 ? 0x80000000u : 0x80000000u >> ((i - 28) * 8);
        assert(md_sim_hash_prefix(hash) == expected);
    }
    struct md_sim_credits credits = {0};
    md_sim_sent(&credits, 512);
    md_sim_status_requested(&credits);
    md_sim_clean(&credits);
    /* The old full-queue response cannot cancel a pending clean replacement. */
    assert(md_sim_status_credits(&credits, 0, 0) == 0);
    assert(credits.pending_clean);
    md_sim_sent(&credits, 61);
    assert(!credits.pending_clean);
    md_sim_status_requested(&credits);
    md_sim_sent(&credits, 61);
    /* A response must deduct writes made after its request. */
    assert(md_sim_status_credits(&credits, 100, 0) == 39);
    md_sim_status_requested(&credits);
    md_sim_sent(&credits, 61);
    assert(md_sim_status_credits(&credits, 40, 0) == 0);
    return 0;
}
