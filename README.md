# Dolomedes

Dolomedes is a Rust implementation of the Kademlia distributed hash table with a custom protocol. It is based on the original [Kademlia paper by Maymounkov and Mazieres](https://pdos.csail.mit.edu/~petar/papers/maymounkov-kademlia-lncs.pdf)

This implementation replaces SHA-1 with SHA-2 for node and key identifiers in response to the collision issues with SHA-1 discussed in [Finding Collisions in the Full SHA-1](https://www.iacr.org/archive/crypto2005/36210017/36210017.pdf)

[The SKademlia paper](https://telematics.tm.kit.edu/publications/Files/267/SKademlia_2007.pdf) is great, I've forced nodes to generate their node ids using a SHA-2 hash of their pubkey to prevent **Eclipse** attacks, where an adversary has nodes strategically join the network surrounding a popular file. I also may add a POW requirement to join the network to avoid **Sybil** attacks which flood the network with adversarial nodes.
