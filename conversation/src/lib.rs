use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};

use anyhow::Error;
use ethers::{
    contract::abigen,
    core::k256::ecdsa::SigningKey,
    prelude::{EthEvent, LocalWallet, Provider, SignerMiddleware, Wallet},
    providers::{Middleware, StreamExt, Ws},
    types::{Address, Bytes, Filter, H160, H256, U256, U64},
};

use ethabi::Token;

use sha3::{Digest, Sha3_256};

type WalletType = Wallet<SigningKey>;
type Client = SignerMiddleware<Provider<Ws>, WalletType>;
type MessageCallback = fn(&String);

pub const GAS_LIMIT: u64 = 250_000u64;
pub const REQUIRED_CONFIRMATIONS: usize = 1;
pub const SENDER_CONTRACT: &str = "0x15aE865d0645816d8EEAB0b7496fdd24227d1801";

// Generate rust bindings for the DIDRegistry contract
abigen!(
    XPSSender,
    "../abi/MessageSender.json",
    derives(serde::Deserialize, serde::Serialize)
);

#[derive(Debug, Clone, Serialize, Deserialize, EthEvent)]
pub struct PayloadSent {
    pub conversation_id: [u8; 32],
    pub message: String,
    pub prev_change: U256,
}

pub struct MessageSender {
    contract: XPSSender<Client>,
    signer: Arc<Client>,
}

impl MessageSender {
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
            let signer = Arc::new(middleware);
            tracing::info!("Contract Connected: {sender_address}");
            let sender_address = H160::from_str(sender_address).unwrap();
            let contract = XPSSender::new(sender_address, signer.clone());

            Ok(Self { contract, signer })
        } else {
            let err = wallet_result.unwrap_err();
            tracing::error!("Wallet error: {:?}", err);
            Err(err)
        }
    }

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

    pub async fn last_n_message(
        &self,
        conversation: &String,
        n: u32,
    ) -> Result<Vec<String>, Error> {
        let mut n = n;
        let mut result_vec = Vec::new();
        let conversation_id = to_conversation_id(conversation).unwrap();
        let last_change_result: Result<U256, _> =
            self.contract.last_message(conversation_id).call().await;
        tracing::info!("conversation_id: {}", hex::encode(conversation_id));
        if let Err(err) = last_change_result {
            tracing::error!("last change error: {:?}", err);
            return Err(anyhow::anyhow!("failed to get last change"));
        }
        while let Ok(prev_block) = last_change_result {
            let prev_change = U64::from(prev_block.as_u64());
            if prev_change == U64::zero() {
                tracing::info!("{} messages found", result_vec.len());
                break;
            }
            tracing::debug!("prev_change: {prev_change}");
            let conversation_topic = [H256::from(conversation_id)];
            let contract_addr = SENDER_CONTRACT.parse::<Address>().unwrap();
            let filter = Filter::new()
                .from_block(prev_change)
                .to_block(prev_change)
                .event("PayloadSent(bytes32,bytes,uint256)")
                .address(vec![contract_addr])
                .topic1(conversation_topic.to_vec());

            let logs = self.signer.get_logs(&filter).await;
            if let Ok(logs) = logs {
                for log in logs.iter() {
                    if tracing::level_enabled!(tracing::Level::TRACE) {                    
                        tracing::trace!("log: {:?}", log);
                    }
                    let param_result = decode_payload_sent(log.data.to_vec());
                    if let Ok(param) = param_result {
                        tracing::debug!("param: {:?}", param);
                        let message = param[0].clone().into_string().unwrap();
                        tracing::trace!("message: {message}");
                        result_vec.push(message);
                        let logged_prev_change = param[1].clone().into_uint().unwrap();
                        last_change_result = Ok(logged_prev_change);
                    } else {
                        let err = param_result.unwrap_err();
                        tracing::error!("param error: {:?}", err);
                        return Err(err);
                    }

                    n -= 1;
                    if n == 0 {
                        last_change_result = Ok(U256::zero());
                        break;
                    }
                }
            }
        }

        result_vec.reverse();
        Ok(result_vec)
    }

    pub async fn follow_messages(
        &self,
        conversation: &String,
        callback: MessageCallback,
    ) -> Result<(), Error> {
        let conversation_id = to_conversation_id(conversation).unwrap();
        tracing::info!("conversation_id: {}", hex::encode(conversation_id));
        let conversation_topic = [H256::from(conversation_id)];
        let contract_addr = SENDER_CONTRACT.parse::<Address>().unwrap();
        let mut last_change_result: Result<U256, _> =
            self.contract.last_message(conversation_id).call().await;
        if let Err(err) = last_change_result {
            tracing::error!("last change error: {:?}", err);
            return Err(anyhow::anyhow!("failed to get last change"));
        }
        let last_change = U64::from(last_change_result.unwrap().as_u64());
        let filter = Filter::new()
            .from_block(last_change)
            .event("PayloadSent(bytes32,bytes,uint256)")
            .address(vec![contract_addr])
            .topic1(conversation_topic.to_vec());

        let mut stream = self.signer.subscribe_logs(&filter).await.unwrap();
        while let Some(log) = stream.next().await {
            if tracing::level_enabled!(tracing::Level::TRACE) {
                tracing::trace!("log: {:?}", log);
            }
            let param_result = decode_payload_sent(log.data.to_vec());
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

fn wallet_from_key(wallet_key: &str) -> Result<WalletType, Error> {
    let wallet = wallet_key.parse::<LocalWallet>()?;
    Ok(wallet)
}

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

fn decode_payload_sent(data: Vec<u8>) -> Result<Vec<Token>, Error> {
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
