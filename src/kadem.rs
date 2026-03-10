use deterministic_rand::rngs::OsRng;
use ed25519_dalek::SigningKey;
use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::PathBuf;

pub type NodeId = [u8; 32];

pub struct StartConfig {
    pub port: u16,
    pub datadir: std::path::PathBuf,
    pub config_path: std::path::PathBuf,
}

struct RuntimeConfig {
    port: u16,
    datadir: std::path::PathBuf,
    signing_key: SigningKey,
    node_id: NodeId,
}

impl RuntimeConfig {
    pub fn from_config(path: std::path::PathBuf) -> Self {
        use sha2::Digest;
        //TODO: use JSON or something with stricter formatting rules this is a little flaky
        let content = std::fs::read_to_string(&path).expect("could not read config file");

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
                "port" => port = Some(value.parse::<u16>().expect("invalid port in config file")),
                "datadir" => datadir = Some(std::path::PathBuf::from(value)),
                _ => {
                    panic!("unrecognized argument in config file")
                }
            }
        }

        let secret_key_hex = secret_key_hex.expect("missing secret_key in config file");
        let secret_key: [u8; 32] = hex::decode(secret_key_hex)
            .expect("secret_key is not valid hex")
            .as_slice()
            .try_into()
            .expect("secret_key must be 32 bytes");

        let signing_key = SigningKey::from_bytes(&secret_key);
        let verifying_key = signing_key.verifying_key();
        // node id is the sha-2 hash of the verifying key
        let node_id: NodeId = sha2::Sha256::digest(verifying_key.as_bytes())
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

#[derive(Clone)]
pub struct NodeContact {
    //UDP port
    port: u16,
    node_id: NodeId,
    ip: std::net::IpAddr,
}

pub enum FindValueResult {
    Contact(Vec<NodeContact>),
    File(PathBuf),
}

pub struct Kademlia {
    // index zero has a completey different prefix,
    // index one has one matching bit,
    // index two has two, all the way to 256 (which is us)
    routing_table: Vec<VecDeque<NodeContact>>,
    kv_store: HashMap<NodeId, PathBuf>,
    config: RuntimeConfig,
}

impl Kademlia {
    pub const BUCKET_SIZE: usize = 8;
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
        //TODO: do we want to create a dir if it doesn't exist?
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
            routing_table: (0..256).map(|_| VecDeque::with_capacity(8)).collect(),
            //TODO: add a floor to this that tells us what the first element of the routing table
            // with contacts in it is. chances are we're not gonna fill 0-200 in testing and even if
            // this grew to ipfs scale we'd still never fill most of them
            kv_store: HashMap::new(),
            config: RuntimeConfig::from_config(config_path),
        }
    }

    pub fn find_node(&self, node_id: NodeId) -> Vec<NodeContact> {
        assert!(node_id != self.config.node_id, "trying to find ourself?");
        // note: in this function (and elsewhere in this file) further/closer refer to ~~xor distance~~ which is described in the kademlia paper
        // all xor distance is is interpreting the size of a ^ b as the distance from a -> b.
        let routing_index = self.routing_index(node_id);

        let closer_buckets = &self.routing_table[routing_index..self.routing_table.len()];
        let mut closer_contacts: Vec<&NodeContact> =
            closer_buckets.iter().flatten().take(8).collect();

        let found = closer_contacts.len();
        if found < 8 {
            // dont we love asserts >,,<
            assert!(found < 8);
            let remaining = 8 - found;
            let further_buckets = &self.routing_table[0..routing_index];
            closer_contacts.append(
                &mut further_buckets
                    .into_iter()
                    .rev()
                    .flatten()
                    .take(remaining)
                    .collect(),
            );
        }

        //TODO: sort this by xor distance for maximum spec adherence in the case that we need to hit multiple buckets
        closer_contacts.into_iter().cloned().collect()
    }

    pub fn find_value(&self, key: NodeId) -> FindValueResult {
        match self.kv_store.get(&key) {
            Some(path) => FindValueResult::File(path.to_owned()),
            None => FindValueResult::Contact(self.find_node(key)),
        }
    }

    /// update the routing table when we communicate with a
    /// node, confirming that it's alive
    pub fn update_bucket(&mut self, node_id: NodeId) {
        todo!()
    }

    //TODO: store, we will implement PING fully in proto.rs

    /// returns the number of matching leading bits of a node id and our node id
    fn routing_index(&self, id: NodeId) -> usize {
        assert!(id != self.config.node_id, "trying to find routing index of ourselves");
        let mut i = 0;
        loop {
            if id[i] == self.config.node_id[i] {
                i += 1;
                continue;
            }
            let matching_high_bits: usize = (self.config.node_id[i] ^ id[i])
                .leading_zeros()
                .try_into()
                .unwrap();
            break (i * 8) + matching_high_bits;
        }
    }
}
