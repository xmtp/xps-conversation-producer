use anyhow::Error;
use std::cmp::max;

use lipsum::lipsum_words;

use appenv::{init, printenv};
use conversation::MessageSender;

fn lipsum_message(size: usize) -> String {
    let mut message = String::new();
    while message.len() < size {
        if !message.is_empty() {
            message.push(' ');
        }
        let remaining_words = max(5, (size - message.len()) / 5);
        message.push_str(&lipsum_words(remaining_words));
    }
    message
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();
    init();
    let env = appenv::environment();
    printenv(&env);
    let message_sender = MessageSender::new(env.rpc_url, env.private_key).await?;
    let message = lipsum_message(env.message_size as usize);
    for _ in 0..env.message_count {
        tracing::info!("Conversation: {}", env.conversation_id);
        tracing::info!("Sending message bytes: {}", message.len());
        tracing::debug!("Sending message: {}", message);
        message_sender
            .send_message(&env.conversation_id, &message)
            .await?;
    }
    Ok(())
}
