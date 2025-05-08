# NSE

Network Sequence Executor

# About
NSE is a library that allow users to create a sequence of messages exchange between hosts.
Messages are of TCP type and can be described with a custom protocol.

# Configuration

## Node

A node represents a host on the network. It is defined by:

- a list of `tasks`, which run in the background
- a `sequence`, which defines the ordered actions the node will perform

```yml
node:
  tasks: []
  sequence:
```

## Tasks

Tasks are **asynchronous, periodic actions** that persist throughout the application's lifetime.  
They are executed in parallel with the `sequence`.
## Actions

Each entry in a node's `sequence` is called an **action**, defined by the required `"action"` key.  
The fields associated with an action vary depending on its type.
Each action must specify a protocol. The supported protocols currently are `tcp` and `udp`.

The supported actions are

- `connect` – establish a connection to a remote host
- `disconnect` – close a connection to a remote host
- `bind` – create a TCP server socket
- `send` – send a message (unicast or broadcast)
- `wait` – wait for an event to occur
- `sleep` – block the sequence for a given duration
### connect

The connect action makes a socket connection to a remote host (or node).

```yml
action: connect
to:
  addr: 192.168.1.5
  port: 8000
timeout_ms: 5000
```
### disconnect

The disconnect action disconnect the node from a remote host if a connection between the two exists.

```yml
action: disconnect
target:
  addr: 192.168.1.5
  port: 8000
```
### bind

The bind action creates a TCP server on the given address and port.

```yml
action: bind
interface:
  addr: 192.168.1.4
  port: 8000
protocol: tcp
```

### send

Sends a message either to specific remote hosts (**unicast**) or to a whole subnet (**broadcast**).
> ℹ️ _If the mode is `unicast`, an active connection (see [`connect`](#connect)) is required._

#### unicast

```yml
action: send
mode: unicast
protocol: tcp
from:
  addr: 192.168.0.1
to:
  - addr: 192.168.0.6
    port: 6900
buffer: [0x1, 0x1, 0x1]
```

#### broadcast

```yml
action: send
mode: broadcast
from:
  addr: 192.168.0.1
to:
  subnet: 192.168.0.0/24
buffer: [0x1, 0x1, 0x1]

```

### sleep

The `sleep` event waits for a given amount of time before executing the next action.

```yml
action: sleep
duration_ms: 5000
```
### wait

Blocks the sequence until a specified **event** occurs. Running `tasks` are not paused.
Supported events are :

- `sleep`: pause for a duration
- `messages`: wait for specific incoming messages
- `connection`: wait for a remote client to connect
#### connection

Waits for one or more remote clients to connect to the local server.

```yml
action: wait
event: connection
protocol: tcp
connections:
  - from:
      addr: 192.168.1.2
    to:
      addr: 192.168.1.4
      port: 3000

```
#### messages

Waits for a list of incoming messages, optionally in a defined order.

- `unordered`: proceed when all messages are received, in any order.
- `ordered`: messages must arrive in the defined order; otherwise, the sequence is blocked until the timeout.

```yml
action: wait
event: messages
timeout: 5000
messages:
  order: unordered
  list:
    - from:
        addr: 192.168.0.6
        port: 6900
      to:
        addr: 192.168.0.1
        port: 6901
      protocol: udp
      buffer: [0x1, 0x1, 0x1]
      expect_response: false
```

## Rust Implementation ideas

```rust
enum Action {
    Connect { to: Endpoint, timeout_ms: u64 },
    Disconnect { target: Endpoint },
    Bind { interface: Endpoint, protocol: Protocol },
    Send { mode: SendMode, from: Address, to: Vec<Endpoint>, buffer: Vec<u8> },
    Sleep { duration_ms: u64 },
    Wait { event: WaitEvent },
}

enum WaitEvent {
    Connection(Vec<ConnectionSpec>),
    Messages { order: OrderType, list: Vec<MessageMatch>, timeout_ms: Option<u64> }
```

## Examples

#### 1. Wait for a client
 
This example create a server on a local IP address and port. Once the client connects, the node sends a message to it.

```yml
node:
  tasks: []
  sequence:
    - action: bind
      interface:
        addr: 192.168.1.4
        port: 8000
      protocol: tcp

    - action: wait
      event: connection
      connections:
        - from:
            addr: 192.168.1.3
          to:
            addr: 192.168.1.4
            port: 8000
          protocol: tcp

    - action: send
      mode: unicast
      from:
        addr: 192.168.1.4
      to:
        - addr: 192.168.1.3
          port: 9999
      protocol: udp
      buffer: [0x1, 0x1, 0x1]

```

# Custom protocols

Messages can be structured using **Protocol Buffers**, allowing users to define and serialize custom message formats.

# Technical Architecture

Sequence is structured in two components. The `nse` (Network Sequence Executor) library and the front-end built in React. The whole apps runs on Tauri.

