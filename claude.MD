This is a reference implementation of a P2P file share similar to BitTorrent with a Kademlia routing table. You will mostly be asked to do simple, boring implementations. Avoid making changes that affect the layout of the system and always ask before going outside of your spec. Readability is priortizied over perfect error handling, so prefer the ? operator as well as expects/unwraps liberally over verbose error handling, as this is a reference implementation. 

STYLE RULES:
- prefer iterator chaining over explicit loops
- NEVER use as, if you see an "as" anywhere you should change it into an into()/try_into().expect()
- strongly avoid creating structs that are only used in one place. Notice how almost structs in the core of this project are used in more than one place, even if they're POD.
- NEVER add directives to ignore compiler warnings
- NEVER prefix variables with underscores, if a variable is unused the unused warning is an intentional signal
- always cargo fmt when you're finished working. There are a lot of unused warning errors so cargo clippy is useless noise that will clog up your context window, so only directly cargo clippy your changes or suppress unused warnings temporarily.