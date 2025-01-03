use copycat::log::colored_level;
use copycat::parse_config_file;
use copycat::{get_neighbors, get_report_timer, start_report_timer};
use copycat::{ChainType, CryptoScheme, Node};
use copycat_flowgen::get_flow_gen;

use std::collections::HashSet;
use std::io::Write;

use tokio::runtime::Builder;
use tokio::time::Duration;

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

    /// Number of mailbox workers
    #[clap(long, short = 'w', default_value_t = 8)]
    num_mailbox_workers: usize,

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

    /// Probability that a conflict transaction will be generated
    #[arg(long, short = 'r', default_value = "0")]
    conflict_rate: f64,

    /// Path to validator configuration
    #[arg(long)]
    config: String,

    /// Network topology
    #[arg(long, default_value_t = String::from(""))]
    topology: String,

    /// If we should disseminate the transactions
    #[arg(long, default_value_t = false)]
    disable_txn_dissem: bool,
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

    let config = parse_config_file(&args.config, args.chain)
        .unwrap()
        .remove(&id)
        .unwrap();
    log::info!("Node config: {:?}", config);

    let neighbors = if args.topology.is_empty() {
        HashSet::new()
    } else {
        match get_neighbors(id, args.topology) {
            Ok(neighbors) => neighbors,
            Err(e) => {
                log::error!("parse network topology failed: {e}");
                HashSet::new()
            }
        }
    };
    log::info!("neighbors: {neighbors:?}");

    let mut stats_file = std::fs::File::create(format!("/tmp/copycat_node_{}.csv", id))
        .expect("stats file creation failed");
    stats_file
        .write(b"Throughput (txn/s)\n")
        .expect("write stats failed");
    let mut latency_file = std::fs::File::create(format!("/tmp/copycat_node_{}_lat.csv", id))
        .expect("latency file creation failed");
    latency_file
        .write(b"Latency (s)\n")
        .expect("write latency failed");

    runtime.block_on(async {
        let (node, mut executed): (Node, _) = match Node::init(
            id,
            args.num_mailbox_workers,
            args.chain,
            args.crypto,
            args.crypto,
            config.chain_config,
            !args.disable_txn_dissem,
            neighbors,
            config.max_concurrency,
        )
        .await
        {
            Ok(node) => node,
            Err(e) => {
                log::error!("failed to start node: {e:?}");
                return;
            }
        };

        let mut flow_gen = get_flow_gen(
            id,
            vec![id],
            args.accounts,
            args.max_inflight,
            args.frequency,
            args.conflict_rate,
            args.chain,
            args.crypto,
        );
        let init_txns = flow_gen.setup_txns().await.unwrap();
        for (_, txn) in init_txns {
            if let Err(e) = node.send_req(txn).await {
                log::error!("failed to send setup txns: {e}");
                return;
            }
        }
        log::info!("setup txns sent");
        let mut report_timer = get_report_timer();
        start_report_timer().await;
        // wait when setup txns are propogated over the network
        tokio::time::sleep(Duration::from_secs(10)).await;
        log::info!("flow generation starts");

        loop {
            tokio::select! {
                wait_next_req = flow_gen.wait_next() => {
                    if let Err(e) = wait_next_req {
                        log::error!("wait for next available request failed: {e:?}");
                        continue;
                    }

                    let next_req_batch = match flow_gen.next_txn_batch().await {
                        Ok(txns) => txns,
                        Err(e) => {
                            log::error!("get available request failed: {e:?}");
                            continue;
                        }
                    };

                    for (_, next_req) in next_req_batch.into_iter() {
                        if let Err(e) = node.send_req(next_req).await {
                            log::error!("sending next request failed: {e:?}");
                            continue;
                        }
                    }
                }

                committed_txns = executed.recv() => {
                    let (height, txns) = match committed_txns {
                        Some(txns) => txns,
                        None => {
                            log::error!("executed pipe closed unexpectedly");
                            return;
                        }
                    };

                    if let Err(e) = flow_gen.txn_committed(id, txns, height).await {
                        log::error!("flow gen failed to record committed transaction: {e:?}");
                        continue;
                    }
                }

                report_val = report_timer.changed() => {
                    if let Err(e) = report_val {
                        log::error!("Waiting for report timeout failed: {}", e);
                    }

                    let stats = flow_gen.get_stats();
                    let tput = stats.num_committed as f64 / report_timer.borrow().as_secs_f64();
                    log::info!(
                        "Throughput: {} txn/s",
                        tput,
                    );
                    stats_file
                        .write_fmt(format_args!("{}\n", tput))
                        .expect("write stats failed");
                    for lat in stats.latencies {
                        latency_file.write_fmt(format_args!("{}\n", lat))
                        .expect("write latency failed");
                    }
                }
            }
        }
    })
}
