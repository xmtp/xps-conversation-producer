use std::env;

pub struct Environment {
    pub rpc_url: String,
    pub public_key: String,
    pub private_key: String,
    pub conversation_id: String,
    pub message_count: u32,
    pub message_size: u32,
}

pub fn init() {
    dotenv::dotenv().ok();
}

pub fn environment() -> Environment {
    Environment {
        rpc_url: env::var("RPC_URL").expect("RPC_URL must be set"),
        public_key: env::var("PUBLIC_KEY").expect("PUBLIC_KEY must be set"),
        private_key: env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set"),
        conversation_id: env::var("CONVERSATION_ID").expect("CONVERSATION_ID must be set"),
        message_count: env::var("MESSAGE_COUNT")
            .expect("MESSAGE_COUNT must be set")
            .parse::<u32>()
            .expect("MESSAGE_COUNT must be a number"),
        message_size: env::var("MESSAGE_SIZE")
            .expect("MESSAGE_SIZE must be set")
            .parse::<u32>()
            .expect("MESSAGE_SIZE must be a number"),
    }
}

pub fn printenv(env: &Environment) {
    tracing::info!("rpc_url: {}", env.rpc_url.split("v2").next().unwrap());
    tracing::info!("private_key: {}", scram(env.private_key.clone()));
    tracing::info!("conversation_id: {}", env.conversation_id);
    tracing::info!("message_count: {}", env.message_count);
    tracing::info!("message_size: {}", env.message_size);
}

pub fn scram(value: String) -> String {
    let mut scrambled = String::new();
    for _ in 0..value.len().min(10) {
        scrambled.push('*');
    }
    scrambled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment() {
        std::env::set_var("RPC_URL", "https://example.com");
        std::env::set_var("PUBLIC_KEY", "my_public_key");
        std::env::set_var("PRIVATE_KEY", "my_private_key");
        std::env::set_var("CONVERSATION_ID", "the_conversation_id");
        std::env::set_var("MESSAGE_SIZE", "100");
        std::env::set_var("MESSAGE_COUNT", "101");

        let env = environment();

        assert_eq!(env.rpc_url, "https://example.com");
        assert_eq!(env.public_key, "my_public_key");
        assert_eq!(env.private_key, "my_private_key");
        assert_eq!(env.conversation_id, "the_conversation_id");
        assert_eq!(env.message_size, 100);
        assert_eq!(env.message_count, 101);
    }

    #[test]
    #[should_panic]
    fn test_environment_missing_rpc_url() {
        std::env::remove_var("RPC_URL");
        std::env::set_var("PUBLIC_KEY", "my_public_key");
        std::env::set_var("PRIVATE_KEY", "my_private_key");
        std::env::set_var("CONVERSATION_ID", "the_conversation_id");
        std::env::set_var("MESSAGE_SIZE", "100");
        std::env::set_var("MESSAGE_COUNT", "101");

        environment();
    }

    #[test]
    #[should_panic]
    fn test_environment_missing_private_key() {
        std::env::set_var("PUBLIC_KEY", "my_public_key");
        std::env::set_var("RPC_URL", "https://example.com");
        std::env::remove_var("PRIVATE_KEY");
        std::env::set_var("CONVERSATION_ID", "the_conversation_id");
        std::env::set_var("MESSAGE_SIZE", "100");
        std::env::set_var("MESSAGE_COUNT", "101");

        environment();
    }

    #[test]
    #[should_panic]
    fn test_environment_missing_message_count() {
        std::env::set_var("PUBLIC_KEY", "my_public_key");
        std::env::set_var("RPC_URL", "https://example.com");
        std::env::set_var("PRIVATE_KEY", "my_private_key");
        std::env::set_var("CONVERSATION_ID", "the_conversation_id");
        std::env::set_var("MESSAGE_SIZE", "100");
        std::env::remove_var("MESSAGE_COUNT");

        environment();
    }

    #[test]
    #[should_panic]
    fn test_environment_missing_message_size() {
        std::env::set_var("PUBLIC_KEY", "my_public_key");
        std::env::set_var("RPC_URL", "https://example.com");
        std::env::set_var("PRIVATE_KEY", "my_private_key");
        std::env::set_var("CONVERSATION_ID", "the_conversation_id");
        std::env::set_var("MESSAGE_COUNT", "100");
        std::env::remove_var("MESSAGE_SIZE");

        environment();
    }

    #[test]
    #[should_panic]
    fn test_environment_message_size_not_a_number() {
        std::env::set_var("PUBLIC_KEY", "my_public_key");
        std::env::set_var("RPC_URL", "https://example.com");
        std::env::set_var("PRIVATE_KEY", "my_private_key");
        std::env::set_var("CONVERSATION_ID", "the_conversation_id");
        std::env::set_var("MESSAGE_COUNT", "100");
        std::env::set_var("MESSAGE_SIZE", "not_a_number");

        environment();
    }

    #[test]
    #[should_panic]
    fn test_environment_message_count_not_a_number() {
        std::env::set_var("PUBLIC_KEY", "my_public_key");
        std::env::set_var("RPC_URL", "https://example.com");
        std::env::set_var("PRIVATE_KEY", "my_private_key");
        std::env::set_var("CONVERSATION_ID", "the_conversation_id");
        std::env::set_var("MESSAGE_SIZE", "100");
        std::env::set_var("MESSAGE_COUNT", "not_a_number");

        environment();
    }

    #[test]
    #[should_panic]
    fn test_environment_missing_public_key() {
        std::env::set_var("RPC_URL", "https://example.com");
        std::env::set_var("PRIVATE_KEY", "my_private_key");
        std::env::set_var("CONVERSATION_ID", "the_conversation_id");
        std::env::set_var("MESSAGE_SIZE", "100");
        std::env::set_var("MESSAGE_COUNT", "101");
        std::env::remove_var("PUBLIC_KEY");

        environment();
    }

    #[test]
    #[should_panic]
    fn test_environment_missing_conversation_id() {
        std::env::set_var("RPC_URL", "https://example.com");
        std::env::set_var("PRIVATE_KEY", "my_private_key");
        std::env::set_var("PUBLIC_KEY", "my_public_key");
        std::env::set_var("MESSAGE_SIZE", "100");
        std::env::set_var("MESSAGE_COUNT", "101");
        std::env::remove_var("CONVERSATION_ID");

        environment();
    }

    #[test]
    fn test_scram() {
        assert_eq!(scram("12345678901".to_string()), "**********");
        assert_eq!(scram("1234567890".to_string()), "**********");
        assert_eq!(scram("123456789".to_string()), "*********");
        assert_eq!(scram("12345678".to_string()), "********");
        assert_eq!(scram("1234567".to_string()), "*******");
        assert_eq!(scram("123456".to_string()), "******");
        assert_eq!(scram("12345".to_string()), "*****");
        assert_eq!(scram("1234".to_string()), "****");
        assert_eq!(scram("123".to_string()), "***");
        assert_eq!(scram("12".to_string()), "**");
        assert_eq!(scram("1".to_string()), "*");
        assert_eq!(scram("".to_string()), "");
    }
}
