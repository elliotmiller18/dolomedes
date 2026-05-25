General TODOs that don't really fit in one place

We use crypto_bigint::U256 and ed25519::VerifyingKey which are both 256 bit ints, but i wonder if there's some like cryptographically secure thing that is bad for perf in VerifyingKey? or should we just use the same type?

Probably make DolomedesClient DolomedesProto and give it its own folder, then have a client for stuff that is used w the cli
and keep all of the internals in DolomedesProto

This whole system is incredibly brittle with unwrap() and expect() calls everywhere on what can be standard os/network errors.
This must change later but for the closed testing I plan on doing it is actually helpful for catching invariants.