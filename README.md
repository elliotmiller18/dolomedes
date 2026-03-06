# Dolomedes

Dolomedes is a Rust implementation of the Kademlia distributed hash table with a custom protocol. It is based on the original Kademlia paper by Maymounkov and Mazieres: https://pdos.csail.mit.edu/~petar/papers/maymounkov-kademlia-lncs.pdf

This implementation replaces SHA-1 with SHA-2 for node and key identifiers in response to the collision issues discussed in the SKademlia paper: https://telematics.tm.kit.edu/publications/Files/267/SKademlia_2007.pdf
