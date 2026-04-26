# THIS IS DATED JUST USING IT AS A REFERENCE, IGNORE

# Protocol

## Encoding Rules
- All node IDs are 32 bytes.
- Node contact format (in order):
  - Node ID (32 bytes)
  - IP marker (1 byte): `0x04` = IPv4, `0x06` = IPv6
  - IP address (4 | 16 bytes depending on ipv4 or v6)
  - Port (2 bytes)

## RPCs

### `PING`
Sender payload:
```
[sender id]
```
Recipient payload:
```
[recipient id]
```

### `FIND_NODE`
Sender payload:
```
[sender id][target id]
```
Recipient payload:
```
[recipient id][contact info of closest nodes to target id * 8]
```

### `FIND_VALUE`
Sender payload:
```
[sender id][target id]
```
Recipient payload:
```
[recipient id]
if no STORE has been received at this id:
    [contact info of closest nodes to target id * 8]
if a STORE has been received at this id:
    [stored value]
```

### `STORE`
Sender payload (request):
```
[sender id][key (node id)][filename (optional; if none, recipient may choose)][size of file in chunks]
```
Recipient payload (ack):
```
[recipient id][0xAA]
```
Sender payload (data stream):
```
[sender id][chunk index][4kb chunk] (repeat until file is finished)
```
Recipient payload (final ack):
```
[recipient id][0xBB]
```
