General TODOs that don't really fit in one place

We use crypto_bigint::U256 and ed25519::VerifyingKey which are both 256 bit ints, but i wonder if there's some like cryptographically secure thing that is bad for perf in VerifyingKey? or should we just use the same type?
