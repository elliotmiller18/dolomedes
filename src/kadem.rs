use deterministic_rand::rngs::OsRng;
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

enum RPC {
    Ping,
    Store,
    FindNode,
    FindValue,
}

pub struct StartConfig {
    port: u16,
    datadir: std::path::PathBuf,
    config_path: std::path::PathBuf,
}

struct RuntimeConfig {
    port: u16, 
    datadir: std::path::PathBuf,
    signing_key: SigningKey,
    node_id: [u8; 32]
}

impl RuntimeConfig {
    pub fn from_config(path: std::path::PathBuf) -> Self {
        use sha2::Digest;

        let content = std::fs::read_to_string(&path)
            .expect("could not read config file");

        let mut secret_key_hex: Option<String> = None;
        let mut port: Option<u16> = None;
        let mut datadir: Option<std::path::PathBuf> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .expect("invalid config line (missing '=')");

            match key {
                "secret_key" => secret_key_hex = Some(value.to_string()),
                "port" => {
                    port = Some(
                        value
                            .parse::<u16>()
                            .expect("invalid port in config file"),
                    )
                }
                "datadir" => datadir = Some(std::path::PathBuf::from(value)),
                _ => {}
            }
        }

        let secret_key_hex = secret_key_hex.expect("missing secret_key in config file");
        let secret_key_bytes = hex::decode(secret_key_hex)
            .expect("secret_key is not valid hex");
        let secret_key: [u8; 32] = secret_key_bytes
            .as_slice()
            .try_into()
            .expect("secret_key must be 32 bytes");

        let signing_key = SigningKey::from_bytes(&secret_key);
        let verifying_key = signing_key.verifying_key();
        let node_id: [u8; 32] = sha2::Sha256::digest(verifying_key.as_bytes())
            .as_slice()
            .try_into()
            .expect("sha256 output must be 32 bytes");

        RuntimeConfig {
            port: port.expect("missing port in config file"),
            datadir: datadir.expect("missing datadir in config file"),
            signing_key,
            node_id,
        }
    }
}

struct NodeContact {
    ip: std::net::IpAddr,
    //UDP port
    port: u16,
    node_id: [u8; 32],
    verification_key: VerifyingKey,
}

pub struct Kademlia {
    routing_table: Vec<Vec<NodeContact>>,
    kv_store: HashMap<[u8; 32], std::path::PathBuf>,
    config: RuntimeConfig
}

impl Kademlia {
    pub fn new(
        StartConfig {
            port,
            datadir,
            config_path,
        }: StartConfig,
    ) -> Self {
        let mut csprng = OsRng {};
        let signing = SigningKey::generate(&mut csprng);

        let key_hex = hex::encode(signing.as_bytes());
        let absolute_datadir = datadir.canonicalize().unwrap();

        let content = format!(
            "secret_key={}\nport={}\ndatadir={}",
            key_hex,
            port,
            absolute_datadir
                .to_str()
                .expect("invalid characters in datadir path"),
        );

        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&config_path)
            .expect("config file already exists or could not be created")
            .write_all(content.as_bytes())
            .expect("failed to write to config file");

        Self::from_config(config_path)
    }

    pub fn from_config(config_path: std::path::PathBuf) -> Self {
        Kademlia {
            routing_table: Vec::with_capacity(256),
            kv_store: HashMap::new(),
            config: RuntimeConfig::from_config(config_path)
        }
    }
}
