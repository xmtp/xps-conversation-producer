use std::{str::FromStr, sync::Arc};

use anyhow::Error;
use ethers::{
    contract::abigen,
    core::k256::ecdsa::SigningKey,
    prelude::{LocalWallet, Provider, SignerMiddleware, Wallet},
    providers::{Middleware, StreamExt, Ws},
    types::{Address, Bytes, Filter, H160, H256, U256, U64},
};

use ethabi::Token;

use sha3::{Digest, Sha3_256};

type WalletType = Wallet<SigningKey>;
type Client = SignerMiddleware<Provider<Ws>, WalletType>;
type MessageCallback = fn(&String);

/// gas limit for transactions
pub const GAS_LIMIT: u64 = 250_000u64;
/// minimum number of confirmations for transactions
pub const REQUIRED_CONFIRMATIONS: usize = 1;
/// XPS MessageSender contract address
pub const SENDER_CONTRACT: &str = "0x15aE865d0645816d8EEAB0b7496fdd24227d1801";

// Generate rust bindings for the DIDRegistry contract
abigen!(
    XPSSender,
    "../abi/MessageSender.json",
    derives(serde::Deserialize, serde::Serialize)
);

/// A struct to hold the message and the last change block.
pub struct MessageRewind {
    pub message: Vec<String>,
    pub last_change: U256,
}

/// A struct to send messages to the XPS Sender contract.
pub struct MessageSender {
    contract: XPSSender<Client>,
    client: Arc<Client>,
}

impl MessageSender {
    /**
     * Create a new MessageSender.
     * rpc_url: the RPC URL for the chain
     * wallet_signer: the private key for the wallet
     */
    pub async fn new(rpc_url: String, wallet_signer: String) -> Result<MessageSender, Error> {
        let sender_address = SENDER_CONTRACT;

        let provider = Provider::<Ws>::connect(rpc_url).await?;
        let chain_id = provider.get_chainid().await?;
        tracing::info!("Connected to chain: {chain_id}");

        // wallet/signer info
        let wallet_result = wallet_from_key(&wallet_signer);
        if let Ok(wallet) = wallet_result {
            tracing::info!("Wallet: {:?}", wallet);
            let middleware = SignerMiddleware::new_with_provider_chain(provider, wallet)
                .await
                .unwrap();
            let client = Arc::new(middleware);
            tracing::info!("Contract Connected: {sender_address}");
            let sender_address = H160::from_str(sender_address).unwrap();
            let contract = XPSSender::new(sender_address, client.clone());

            Ok(Self { contract, client })
        } else {
            let err = wallet_result.unwrap_err();
            tracing::error!("Wallet error: {:?}", err);
            Err(err)
        }
    }

    /**
     * Send a message to the XPS Sender contract.
     * conversation: the conversation ID
     * message: the message to send
     * Returns Ok(()) if the transaction was successful.
     */
    pub async fn send_message(&self, conversation: &String, message: &String) -> Result<(), Error> {
        let conversation_id_result = to_conversation_id(conversation);
        if let Err(err) = conversation_id_result {
            tracing::error!("Conversation ID error: {:?}", err);
            return Err(anyhow::anyhow!("failed to get conversation ID"));
        }
        let conversation_id = conversation_id_result.unwrap();
        let message_bytes = Bytes::from(message.as_bytes().to_vec());
        let tx = self.contract.send_message(conversation_id, message_bytes);
        let receipt = tx
            .gas(GAS_LIMIT)
            .send()
            .await
            .unwrap()
            .confirmations(REQUIRED_CONFIRMATIONS)
            .await;
        if let Err(err) = receipt {
            tracing::error!("Transaction error: {:?}", err);
            return Err(anyhow::anyhow!("failed to send message"));
        }
        tracing::info!("Transaction receipt: {:?}", receipt);
        Ok(())
    }

    /**
     * Rewind the conversation to the last n messages.
     * Returns Ok(MessageRewind) a struct containing messages and the last change block.
     */
    pub async fn rewind(&self, conversation: &String, n: u32) -> Result<MessageRewind, Error> {
        let mut n = n;
        let conversation_id = to_conversation_id(conversation).unwrap();
        let last_change_result: Result<U256, _> =
            self.contract.last_message(conversation_id).call().await;
        tracing::info!("conversation_id: {}", hex::encode(conversation_id));
        if let Err(err) = last_change_result {
            tracing::error!("last change error: {:?}", err);
            return Err(anyhow::anyhow!("failed to get last change"));
        }
        let mut rewind = MessageRewind {
            message: Vec::new(),
            last_change: U256::zero(),
        };
        let mut last_change = last_change_result.unwrap();
        rewind.last_change = last_change;
        while last_change != U256::zero() {
            tracing::debug!("prev_change: {}", last_change);
            let conversation_topic = [H256::from(conversation_id)];
            let contract_addr = SENDER_CONTRACT.parse::<Address>().unwrap();
            let filter = Filter::new()
                .from_block(U64::from(last_change.as_u64()))
                .to_block(U64::from(last_change.as_u64()))
                .event("PayloadSent(bytes32,bytes,uint256)")
                .address(vec![contract_addr])
                .topic1(conversation_topic.to_vec());
            let logs = self.client.get_logs(&filter).await;
            if let Ok(logs) = logs {
                for log in logs.iter() {
                    if tracing::level_enabled!(tracing::Level::TRACE) {
                        tracing::trace!("log: {:?}", log);
                    }
                    let param_result = abi_decode_payload_sent(log.data.to_vec());
                    if let Ok(param) = param_result {
                        tracing::debug!("param: {:?}", param);
                        let message = param[0].clone().into_string().unwrap();
                        if tracing::level_enabled!(tracing::Level::TRACE) {
                            tracing::trace!("message: {message}");
                        }
                        rewind.message.push(message);
                        last_change = param[1].clone().into_uint().unwrap();
                    } else {
                        let err = param_result.unwrap_err();
                        tracing::error!("param error: {:?}", err);
                        return Err(err);
                    }

                    n -= 1;
                    if n == 0 {
                        last_change = U256::zero();
                        break;
                    }
                }
            }
        }

        rewind.message.reverse();
        tracing::info!("{} messages found", rewind.message.len());
        Ok(rewind)
    }

    /**
     * Follow the conversation and call the callback function for each new message.
     * conversation: the conversation ID
     * start_block: the block to start following from
     * callback: the callback function to call for each new message
     * Returns Ok(()) if the transaction was successful.
     */
    pub async fn follow_messages(
        &self,
        conversation: &String,
        start_block: &U256,
        callback: MessageCallback,
    ) -> Result<(), Error> {
        let conversation_id = to_conversation_id(conversation).unwrap();
        tracing::info!("conversation_id: {}", hex::encode(conversation_id));
        let conversation_topic = [H256::from(conversation_id)];
        let contract_addr = SENDER_CONTRACT.parse::<Address>().unwrap();
        let filter = Filter::new()
            .from_block(U64::from(start_block.as_u64()))
            .event("PayloadSent(bytes32,bytes,uint256)")
            .address(vec![contract_addr])
            .topic1(conversation_topic.to_vec());

        let mut stream = self.client.subscribe_logs(&filter).await.unwrap();
        while let Some(log) = stream.next().await {
            if tracing::level_enabled!(tracing::Level::TRACE) {
                tracing::trace!("log: {:?}", log);
            }
            let param_result = abi_decode_payload_sent(log.data.to_vec());
            if let Ok(param) = param_result {
                tracing::debug!("param: {:?}", param);
                let message = param[0].clone().into_string().unwrap();
                tracing::trace!("message: {message}");
                callback(&message);
            } else {
                let err = param_result.unwrap_err();
                tracing::error!("param error: {:?}", err);
                return Err(err);
            }
        }
        Ok(())
    }
}

/*
 * Create a wallet from a private key.
 * wallet_key: the private key
 * Returns Ok(WalletType) if the wallet was created successfully.
 */
fn wallet_from_key(wallet_key: &str) -> Result<WalletType, Error> {
    let wallet = wallet_key.parse::<LocalWallet>()?;
    Ok(wallet)
}

/*
 * Create a conversation ID from a conversation string.
 * conversation: the conversation string
 * Returns Ok([u8; 32]) if the conversation ID was created successfully.
 */
fn to_conversation_id(conversation: &String) -> Result<[u8; 32], Error> {
    let mut hasher = Sha3_256::default();
    hasher.update(conversation.as_bytes());
    let result = hasher.finalize();
    let conversation_id = H256::from_slice(&result);
    let conversation_id = *conversation_id.as_fixed_bytes();
    if conversation_id.len() > 32 {
        return Err(anyhow::anyhow!("Conversation ID too long"));
    }
    Ok(conversation_id)
}

/*
 * Decode the payload sent event.
 * data: the event data
 * Returns Ok(Vec<Token>) if the event was decoded successfully.
 */
fn abi_decode_payload_sent(data: Vec<u8>) -> Result<Vec<Token>, Error> {
    let param = [ethabi::ParamType::String, ethabi::ParamType::Uint(256)];
    let decoded = ethabi::decode(&param, &data)?;
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_conversation_id() {
        let conversation = String::from("test");
        let conversation_id = to_conversation_id(&conversation).unwrap();
        let expected: [u8; 32] = [
            54, 240, 40, 88, 11, 176, 44, 200, 39, 42, 154, 2, 15, 66, 0, 227, 70, 226, 118, 174,
            102, 78, 69, 238, 128, 116, 85, 116, 226, 245, 171, 128,
        ];
        assert_eq!(conversation_id, expected);
    }
}
