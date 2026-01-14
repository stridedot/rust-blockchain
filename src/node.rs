use anyhow::Result;
use std::{net::SocketAddr, sync::RwLock};

#[derive(Clone)]
pub struct Node {
    addr: String,
}

impl Node {
    fn new(addr: String) -> Node {
        Node { addr }
    }

    pub fn get_addr(&self) -> String {
        self.addr.clone()
    }

    pub fn parse_socket_addr(&self) -> Result<SocketAddr> {
        Ok(self.addr.parse()?)
    }
}

pub struct Nodes {
    inner: RwLock<Vec<Node>>,
}

impl Nodes {
    pub fn new() -> Self {
        Nodes {
            inner: RwLock::new(vec![]),
        }
    }

    pub fn get_nodes(&self) -> Vec<Node> {
        self.inner.read().expect("failed to read nodes").to_vec()
    }

    pub fn add_node(&self, addr: String) {
        let mut inner = self.inner.write().expect("failed to write nodes");
        if inner.iter().any(|x| x.get_addr().eq(&addr)) {
            return;
        }
        inner.push(Node::new(addr));
    }

    pub fn evict_node(&self, addr: &str) {
        let mut inner = self.inner.write().expect("failed to write nodes");
        if let Some(idx) = inner.iter().position(|x| x.get_addr().eq(addr)) {
            inner.remove(idx);
        }
    }

    pub fn first(&self) -> Option<Node> {
        self.inner
            .read()
            .expect("failed to read nodes")
            .first()
            .cloned()
    }

    pub fn node_is_known(&self, addr: &str) -> bool {
        self.inner
            .read()
            .expect("failed to read nodes")
            .iter()
            .any(|x| x.get_addr().eq(addr))
    }
}
