![XPS](xps.png)

## Quick Start (Development)

- [READ THE DOCS](https://xmtp.github.io/xps-conversation-producer)

### Dev Containers Development

This contract supports containerized development. From Visual Studio Code Dev Containers extension

`Reopen in Container`

or

Command line build using docker

```bash
$ docker build . -t xps-contract:1
```

## Testing (command line)

```bash
$ cargo test
```

### Sending Messages with a `MessageSender`

#### Developer Introduction

The `MessageSender` interface, part of the xps gateway, defines the interface in managing data within our decentralized messaging system. This interface allows client applications to interact with the system in three key ways:

1. **Sending Messages (`sendMessage`)**: This function enables the transmission of diverse data types encapsulated in bytes payloads to specified `conversationId`s. It's designed for flexibility, allowing a wide range of data to be sent within the network. Upon a successful transaction, a `PayloadSent` event is triggered, serving as a confirmation and record of the action.

```rust
    let tx = contract.send_message(conversation_id, message_bytes);
    let receipt = tx
        .gas(GAS_LIMIT)
        .send()
        .await;
```

2. **Event Indexing and Trail Creation (`PayloadSent`)**: The `PayloadSent` event is uniquely indexed by `conversationId`, known as `topic1`, and includes the bytes payload. Additionally, it records the `lastMessage` block number, creating an enumerated message trail. This implements the ability to rewind or replay messages.

```rust
    let filter = Filter::new()
        .from_block(U64::from(last_change))
        .to_block(U64::from(last_change))
        .event("PayloadSent(bytes32,bytes,uint256)")
        .address(vec![contract_addr])
        .topic1(conversation_topic);
    let logs = self.client.get_logs(&filter).await;
    if let Ok(logs) = logs {
        for log in logs.iter() {
            if tracing::level_enabled!(tracing::Level::TRACE) {
                tracing::trace!("log: {:?}", log);
            }
        }
    }
```

3. **Active Listening**: Applications can actively listen to the ongoing stream of payload data, allowing real-time data processing and response to the message flow.

```rust
    let filter = Filter::new()
        .from_block(U64::from(start_block))
        .event("PayloadSent(bytes32,bytes,uint256)")
        .address(vec![contract_addr])
        .topic1(conversation_topic);

    let mut stream = self.client.subscribe_logs(&filter).await.unwrap();
    while let Some(log) = stream.next().await {
        if tracing::level_enabled!(tracing::Level::TRACE) {
            tracing::trace!("log: {:?}", log);
        }
    }
```

For a practical demonstration of these operations, the [xps-conversation-producer](https://github.com/xmtp/xps-conversation-producer) project provides a working example of the implementation of these roles in a live environment. This example can be helpful for developers looking to understand the practical application of `MessageSender` in a real-world scenario.
