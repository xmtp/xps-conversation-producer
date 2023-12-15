use anyhow::Error;
use std::cmp::min;

use appenv::{init, printenv};
use conversation::MessageSender;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    init();
    let env = appenv::environment();
    printenv(&env);
    let message_sender = MessageSender::new(env.rpc_url, env.private_key).await?;

    let rewind = message_sender
        .rewind(&env.conversation_id, min(env.message_count, 1000))
        .await?;
    for (i, message) in rewind.message.iter().enumerate() {
        tracing::info!("Message {}: {}", i, message);
    }

    let callback = |s: &String| tracing::info!("Message: {}", s);
    message_sender
        .follow_messages(&env.conversation_id, &rewind.last_change, callback)
        .await?;

    Ok(())
}
