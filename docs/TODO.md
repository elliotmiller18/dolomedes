General TODOs that don't really fit in one place

We use crypto_bigint::U256 and ed25519::VerifyingKey which are both 256 bit ints, but i wonder if there's some like cryptographically secure thing that is bad for perf in VerifyingKey? or should we just use the same type?

Probably make DolomedesClient DolomedesProto and give it its own folder, then have a client for stuff that is used w the cli
and keep all of the internals in DolomedesProto

This whole system is incredibly brittle with unwrap() and expect() calls everywhere on what can be standard os/network errors.
This must change later but for the closed testing I plan on doing it is actually helpful for catching invariants.

My idea is that all external node failures should be handled very gracefully, while all OS and Network failures should be handled very aggressively. We should be crash first and crash on local failures with easy recovery with some kind of crash loop detection.

When we implement saving routing table to disk we need to add saving genesis nodes at setup instead of them being an argument to serve