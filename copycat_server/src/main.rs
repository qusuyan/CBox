mod composition;
mod config;
mod flowgen;
mod node;
mod peers;
mod stage;

use copycat_protocol::{ChainType, CryptoScheme, DissemPattern};
use copycat_utils::log::colored_level;

use config::Config;
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

    /// Number of user accounts for flow generation
    #[arg(long, short = 'a', default_value = "10000")]
    accounts: usize,

    /// Maximum number of inflight transaction requests
    #[arg(long, short = 'f', default_value = "100000")]
    max_inflight: usize,

    /// Frequency at which transactions are generated
    #[arg(long, short = 'q', value_enum, default_value = "0")]
    frequency: usize,

    /// Blockchain specific configuration string in TOML format
    #[arg(long, default_value_t = String::from(""))]
    config: String,
}

impl CliArgs {
    pub fn validate(&mut self) {
        if self.num_threads < 8 {
            self.num_threads = 8;
        }

        if self.frequency == 0 && self.max_inflight == 0 {
            log::warn!("neither frequency nor max_inflight is set, restore to default");
            self.max_inflight = 100000;
        }
    }
}

pub fn main() {
    let mut args = CliArgs::parse();
    args.validate();

    let id = args.id;
    env_logger::builder()
        .format(move |buf, record| {
            let mut style = buf.style();
            let level = colored_level(&mut style, record.level());
            let mut style = buf.style();
            let target = style.set_bold(true).value(record.target());
            writeln!(buf, "Node{id} {level} {target}: {}", record.args())
        })
        .init();

    // if id == 0 {
    //     console_subscriber::init();
    // }
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(args.num_threads)
        .thread_name("copycat-server-thread")
        .build()
        .expect("Creating new runtime failed");

    let config = if args.config.is_empty() {
        Config::from_str(args.chain, None).unwrap()
    } else {
        match Config::from_str(args.chain, Some(&args.config)) {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Invalid config string ({e:?}), fall back to default");
                Config::from_str(args.chain, None).unwrap()
            }
        }
    };

    let mut stats_file = std::fs::File::create(format!("/tmp/copycat_node_{}.csv", id))
        .expect("stats file creation failed");
    stats_file
        .write(b"Throughput (txn/s), Avg Latency (s)\n")
        .expect("write stats failed");

    runtime.block_on(async {
        let (node, mut executed): (Node, _) =
            match Node::init(id, args.chain, args.dissem_pattern, args.crypto, config).await {
                Ok(node) => node,
                Err(e) => {
                    log::error!("failed to start node: {e:?}");
                    return;
                }
            };

        let mut flow_gen = get_flow_gen(
            id,
            args.accounts,
            args.max_inflight,
            args.frequency,
            args.chain,
            args.crypto,
        );
        let init_txns = flow_gen.setup_txns().await.unwrap();
        for txn in init_txns {
            if let Err(e) = node.send_req(txn).await {
                log::error!("failed to send setup txns: {e}");
                return;
            }
        }
        log::info!("setup txns sent");
        // wait when setup txns are propogated over the network
        tokio::time::sleep(Duration::from_secs(30)).await;
        log::info!("flow generation starts");

        let start_time = Instant::now();
        let mut report_time = start_time + Duration::from_secs(60);

        loop {
            tokio::select! {
                wait_next_req = flow_gen.wait_next() => {
                    if let Err(e) = wait_next_req {
                        log::error!("wait for next available request failed: {e:?}");
                        continue;
                    }

                    let next_req = match flow_gen.next_txn().await {
                        Ok(txn) => txn,
                        Err(e) => {
                            log::error!("get available request failed: {e:?}");
                            continue;
                        }
                    };

                    if let Err(e) = node.send_req(next_req).await {
                        log::error!("sending next request failed: {e:?}");
                        continue;
                    }
                }

                committed_txn = executed.recv() => {
                    let txn = match committed_txn {
                        Some(txn) => txn,
                        None => {
                            log::error!("committed_txn closed unexpectedly");
                            return;
                        }
                    };

                    if let Err(e) = flow_gen.txn_committed(txn).await {
                        log::error!("flow gen failed to record committed transaction: {e:?}");
                        continue;
                    }
                }

                _ = tokio::time::sleep_until(report_time) => {
                    let stats = flow_gen.get_stats();
                    let tput = stats.num_committed / (report_time - start_time).as_secs();
                    log::info!(
                        "Throughput: {} txn/s, Average Latency: {} s",
                        tput,
                        stats.latency
                    );
                    stats_file
                        .write_fmt(format_args!("{},{}\n", tput, stats.latency))
                        .expect("write stats failed");
                    report_time += Duration::from_secs(60);
                }
            }
        }
    })
}
