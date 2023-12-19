mod composition;
mod flowgen;
mod node;
mod peers;
mod stage;

use copycat_protocol::{ChainType, CryptoScheme, DissemPattern};
use copycat_utils::log::colored_level;

use flowgen::get_flow_gen;
use node::Node;

use std::io::Write;

use tokio::runtime::Builder;
use tokio::time::{Duration, Instant};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    /// ID of the blockchain node
    #[arg(long, short = 'i')]
    id: u64,

    /// The type of blockchain
    #[arg(long, short = 'c', value_enum, default_value = "dummy")]
    chain: ChainType,

    /// Number of executor threads
    #[arg(long, short = 't', default_value = "8")]
    num_threads: usize,

    /// Dissemination pattern
    #[arg(long, short = 'n', default_value = "broadcast")]
    dissem_pattern: DissemPattern,

    /// Cryptography scheme
    #[arg(long, short = 'p', value_enum, default_value = "dummy")]
    crypto: CryptoScheme,
}

impl CliArgs {
    pub fn validate(&mut self) {
        if self.num_threads < 8 {
            self.num_threads = 8;
        }
    }
}

pub fn main() {
    let mut args = CliArgs::parse();
    args.validate();

    env_logger::builder()
        .format(move |buf, record| {
            let mut style = buf.style();
            let level = colored_level(&mut style, record.level());
            let mut style = buf.style();
            let target = style.set_bold(true).value(record.target());
            writeln!(buf, "{level} {target}: {}", record.args())
        })
        .init();

    let id = args.id;
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(args.num_threads)
        .thread_name("tokio-worker-replica")
        .build()
        .expect("Creating new runtime failed");

    runtime.block_on(async {
        let (node, mut executed): (Node, _) =
            match Node::init(id, args.chain, args.dissem_pattern, args.crypto).await {
                Ok(node) => node,
                Err(e) => {
                    log::error!("Node {id}: failed to start node: {e:?}");
                    return;
                }
            };

        let mut flow_gen = get_flow_gen(args.chain);
        let start_time = Instant::now();
        let mut report_time = start_time + Duration::from_secs(60);

        loop {
            tokio::select! {
                wait_next_req = flow_gen.wait_next() => {
                    if let Err(e) = wait_next_req {
                        log::error!("Node {id}: wait for next available request failed: {e:?}");
                        continue;
                    }

                    let next_req = match flow_gen.next_txn().await {
                        Ok(txn) => txn,
                        Err(e) => {
                            log::error!("Node {id}: get available request failed: {e:?}");
                            continue;
                        }
                    };

                    if let Err(e) = node.send_req(next_req).await {
                        log::error!("Node {id}: sending next request failed: {e:?}");
                        continue;
                    }
                }

                committed_txn = executed.recv() => {
                    let txn = match committed_txn {
                        Some(txn) => txn,
                        None => {
                            log::error!("Node {id}: committed_txn closed unexpectedly");
                            continue;
                        }
                    };

                    if let Err(e) = flow_gen.txn_committed(txn).await {
                        log::error!("Node {id}: flow gen failed to record committed transaction: {e:?}");
                        continue;
                    }
                }

                _ = tokio::time::sleep_until(report_time) => {
                    let stats = flow_gen.get_stats();
                    log::info!(
                        "Node {id}: Throughput: {} txn/s, Average Latency: {} s", 
                        stats.num_committed / (report_time - start_time).as_secs(),
                        stats.latency
                    );
                    report_time += Duration::from_secs(60);
                }
            }
        }
    })
}
