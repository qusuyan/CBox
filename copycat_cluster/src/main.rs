use mailbox::Mailbox;

use copycat::log::colored_level;
use copycat::Node;
use copycat::{fully_connected_topology, get_topology};
use copycat::{ChainType, Config, CryptoScheme, DissemPattern};
use copycat_flowgen::get_flow_gen;
use std::collections::{HashMap, HashSet};
use std::io::Write;

use tokio::runtime::Builder;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

use clap::Parser;
use sysinfo::System;


// TODO: add parameters to config individual node configs
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct CliArgs {
    /// ID of the physical machine this instance runs on
    #[clap(long, short = 'i')]
    id: u64,

    /// Path to config file containing the list of physical machines and nodes they host
    #[clap(long, short = 'm')]
    machine_config: String,

    /// Path to network config file containing the list of network channel specs
    #[clap(long, short = 'n')]
    network_config: String,

    /// Number of threads
    #[clap(long, short = 't', default_value_t = 8)]
    num_threads: u64,

    /// Max allowed per node concurrency
    #[clap(long, short = 'r')]
    per_node_concurrency: Option<usize>,

    /// Number of concurrent TCP streams between each pair of peers
    #[clap(long, short = 'o', default_value_t = 1)]
    num_conn_per_peer: usize,

    /// The type of blockchain
    #[arg(long, short = 'c', value_enum, default_value = "dummy")]
    chain: ChainType,

    /// Dissemination pattern
    #[arg(long, short = 'd', default_value = "broadcast")]
    dissem_pattern: DissemPattern,

    /// Cryptography scheme
    #[arg(long, short = 'p', value_enum, default_value = "dummy")]
    crypto: CryptoScheme,

    /// Number of clients
    #[arg(long, short = 'l')]
    num_clients: Option<usize>,

    /// Number of user accounts for flow generation
    #[arg(long, short = 'a', default_value = "10000")]
    accounts: usize,

    /// Maximum number of inflight transaction requests, 0 means unlimited
    #[arg(long, short = 'f', default_value = "100000")]
    max_inflight: usize,

    /// Frequency at which transactions are generated, 0 means unlimited
    #[arg(long, short = 'q', value_enum, default_value = "0")]
    frequency: usize,

    /// Number of nodes receiving a transaction, 0 means sending to all nodes
    #[arg(long, short = 's', default_value_t = 1)]
    txn_span: usize,

    /// Blockchain specific configuration string in TOML format
    #[arg(long, default_value_t = String::from(""))]
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
            log::warn!("not enough threads running, setting it to 8");
            self.num_threads = 8;
        }

        // if self.frequency == 0 && self.max_inflight == 0 {
        //     log::warn!("neither frequency nor max_inflight is set, restore to default");
        //     self.max_inflight = 100000;
        // }

        // if self.accounts < 100 {
        //     log::warn!("too few accounts, reset to default");
        //     self.accounts = 10000
        // }

        // if self.disable_txn_dissem && self.txn_span > 0 {
        //     log::warn!("currently does not support sending flow generation to different nodes when transaction dissemination is disabled");
        //     self.txn_span = 0;
        // }

        if matches!(self.chain, ChainType::Avalanche)
            && !matches!(self.dissem_pattern, DissemPattern::Sample)
        {
            log::warn!("Avalanche is based on sampling, using dissem pattern sample instead");
            self.dissem_pattern = DissemPattern::Sample;
        }

        if !matches!(self.dissem_pattern, DissemPattern::Gossip) && !self.topology.is_empty() {
            log::warn!("Topology files are only used for gossipping, but the dissem pattern is {:?}, ignoring topology file...", self.dissem_pattern);
            self.topology = String::from("");
        }
    }
}

fn main() {
    let mut args = CliArgs::parse();
    args.validate();

    let id = args.id;
    env_logger::builder()
        .format(move |buf, record| {
            let mut style = buf.style();
            let level = colored_level(&mut style, record.level());
            let mut style = buf.style();
            let target = style.set_bold(true).value(record.target());
            let ts = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
            writeln!(buf, "Cluster{id} {ts} {level} {target}: {}", record.args())
        })
        .init();

    log::info!("{:?}", args);

    let txn_crypto = args.crypto;
    let p2p_crypto = args.crypto;

    // get mailbox parameters
    let machine_config_path = args.machine_config;
    let machine_list = match mailbox::config::read_machine_config(&machine_config_path) {
        Ok(machines) => machines,
        Err(e) => {
            log::error!("failed to read json config {machine_config_path}: {e}");
            return;
        }
    };
    let local_nodes = machine_list[&id].node_list.clone();

    let nodes = machine_list
        .iter()
        .flat_map(|(_, mailbox::config::Machine { addr: _, node_list })| node_list.clone())
        .collect();
    let network_config_path = args.network_config;
    let network_config = match mailbox::config::read_network_config(&network_config_path) {
        Ok(config) => config,
        Err(e) => {
            log::error!("failed to read network config {network_config_path}: {e}");
            return;
        }
    };
    let pipe_info = mailbox::config::parse_network_config(&nodes, network_config);

    // get node parameters

    let mut topology = if args.topology.is_empty() {
        fully_connected_topology(&local_nodes, &nodes)
    } else {
        match get_topology(&local_nodes, args.topology) {
            Ok(neighbors) => neighbors,
            Err(e) => {
                log::error!("parse network topology failed, fall back to fully connected: {e}");
                fully_connected_topology(&local_nodes, &nodes)
            }
        }
    };
    log::info!("topology: {topology:?}");

    let mut node_config = if args.config.is_empty() {
        Config::from_str(args.chain, None).unwrap()
    } else {
        args.config = args.config.replace('+', "\n");
        match Config::from_str(args.chain, Some(&args.config)) {
            Ok(config) => config,
            Err(e) => {
                log::warn!("Invalid config string ({e:?}), fall back to default");
                Config::from_str(args.chain, None).unwrap()
            }
        }
    };
    let min_neighbors = topology
        .iter()
        .map(|(_, neighbors)| neighbors.len())
        .min()
        .unwrap();
    node_config.validate(min_neighbors);
    log::info!("Node config: {:?}", node_config);

    // get flow generation metrics
    let txn_span = args.txn_span;
    let client_list: Vec<u64> = match args.num_clients {
        Some(clients) => (0..clients as u64).collect(),
        None => (0..local_nodes.len() as u64).collect(),
    };
    let mut req_dsts = HashMap::new();
    for idx in 0..client_list.len() {
        let dsts = if txn_span == 0 {
            local_nodes.clone()
        } else {
            let mut dsts = vec![];
            for offset in 0..txn_span {
                dsts.push(local_nodes[(idx + offset) % local_nodes.len()]);
            }
            dsts
        };
        req_dsts.insert(client_list[idx], dsts);
    }

    let max_inflight = args.max_inflight;
    let frequency = args.frequency;
    let num_accounts = args.accounts;

    // collect system information
    let mut sys = System::new();
    sys.refresh_cpu_usage();

    let mut stats_file = std::fs::File::create(format!("/tmp/copycat_cluster_{}.csv", id))
        .expect("stats file creation failed");
    stats_file
        .write(b"Runtime (s),Throughput (txn/s),Avg Latency (s),Chain Length,Commit Confidence,Avg CPU Usage\n")
        .expect("write stats failed");

    // console_subscriber::init();

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(args.num_threads as usize)
        .thread_name("copycat-cluster-thread")
        .build()
        .expect("Creating new runtime failed");

    runtime.block_on(async {
        // start mailbox
        let _mailbox = match Mailbox::init(id, machine_list, pipe_info, args.num_conn_per_peer).await {
            Ok(mailbox) => mailbox,
            Err(e) => {
                log::error!("Mailbox initialization failed with error {:?}", e);
                std::process::exit(-1);
            }
        };

        // start nodes
        let (combine_tx, mut combine_rx) = mpsc::channel(0x1000000);
        let mut node_map = HashMap::new();
        // let mut node_rt = HashMap::new();
        let mut redirect_handles = HashMap::new();
        for node_id in local_nodes.clone() {
            // let rt = Builder::new_multi_thread()
            //     .enable_all()
            //     .worker_threads(2)
            //     .thread_name(format!("copycat-node-{node_id}-thread"))
            //     .build()
            //     .expect("Creating new runtime failed");
            let result = Node::init(
                node_id,
                args.chain,
                args.dissem_pattern,
                txn_crypto,
                p2p_crypto,
                node_config.clone(),
                !args.disable_txn_dissem,
                topology.remove(&node_id).unwrap_or(HashSet::new()),
                args.per_node_concurrency,
            ).await;
            let (node, mut executed): (Node, _) = match result {
                Ok(node) => node,
                Err(e) => {
                    log::error!("failed to start node: {e:?}");
                    return;
                }
            };
            node_map.insert(node_id, node);
            // node_rt.insert(node_id, rt);

            let tx = combine_tx.clone();
            let redirect_handle = tokio::spawn(async move {
                loop {
                    match executed.recv().await {
                        Some(msg) => {
                            if let Err(e) = tx.send((node_id, msg)).await {
                                log::error!("Node {id} redirecting executed pipe failed: {e:?}")
                            }
                        }
                        None => {
                            log::error!("Node {id} executed pipe closed unexpectedly");
                            return;
                        }
                    };
                }
            });
            redirect_handles.insert(node_id, redirect_handle);
        }

        // run flowgen
        let mut flow_gen = get_flow_gen(
            id, 
            client_list,
            num_accounts,
            max_inflight,
            frequency,
            args.chain,
            args.crypto,
        );
        let init_txns = flow_gen.setup_txns().await.unwrap();

        for (client_id, txn) in init_txns {
            let receiving_node_ids = req_dsts.get(&client_id).unwrap();
            let receiving_nodes = receiving_node_ids.iter().map(|node_id| (node_id, node_map.get(node_id).unwrap()));
            for (id, node) in receiving_nodes {
                if let Err(e) = node.send_req(txn.clone()).await {
                    log::error!("failed to send setup txns to node {id}: {e}");
                    return;
                }
            }
        }

        log::info!("setup txns sent");
        let start_time = Instant::now();
        let report_interval = 60f64;
        let mut report_time = start_time + Duration::from_secs_f64(report_interval);
        // wait when setup txns are propogated over the network
        tokio::time::sleep(Duration::from_secs(10)).await;
        log::info!("flow generation starts");
        let mut txns_sent = 0;
        let mut prev_committed = 0;

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

                    txns_sent += next_req_batch.len();

                    for (client_id, next_req) in next_req_batch.into_iter() {
                        let receiving_node_ids = req_dsts.get(&client_id).unwrap();
                        let receiving_nodes = receiving_node_ids.iter().map(|node_id| (node_id, node_map.get(node_id).unwrap()));
                        for (id, node) in receiving_nodes {
                            if let Err(e) = node.send_req(next_req.clone()).await {
                                log::error!("sending next request to node {id} failed: {e:?}");
                                return;
                            }
                        }
                    }
                }

                committed_txn = combine_rx.recv() => {
                    let (node, (height, txn_batch)) = match committed_txn {
                        Some(txn) => txn,
                        None => {
                            log::error!("committed_txn closed unexpectedly");
                            return;
                        }
                    };

                    if let Err(e) = flow_gen.txn_committed(node, txn_batch, height).await {
                        log::error!("flow gen failed to record committed transaction: {e:?}");
                        continue;
                    }
                }

                _ = tokio::time::sleep_until(report_time) => {
                    let stats = flow_gen.get_stats();
                    let run_time =  (report_time - start_time).as_secs_f64();
                    let newly_committed = stats.num_committed - prev_committed;
                    let tput = newly_committed as f64 / report_interval;

                    sys.refresh_cpu_usage();
                    let (cpu_usage, cpu_count) = sys.cpus()
                        .into_iter()
                        .map(|cpu| (cpu.cpu_usage(), 1))
                        .reduce(|(usage1, count1), (usage2, count2)| (usage1 + usage2, count1 + count2))
                        .expect("no CPU returned");
                    let avg_cpu_usage = cpu_usage / cpu_count as f32;

                    log::info!(
                        "Runtime: {} s, Throughput: {} txn/s, Average Latency: {} s, Chain Length: {}, Commit confidence: {}, CPU Usage: {}",
                        run_time,
                        tput,
                        stats.latency,
                        stats.chain_length,
                        stats.commit_confidence,
                        avg_cpu_usage,
                    );
                    stats_file
                        .write_fmt(format_args!("{},{},{},{},{},{}\n", run_time, tput, stats.latency, stats.chain_length, stats.commit_confidence, avg_cpu_usage))
                        .expect("write stats failed");
                    log::info!("In the last minute: txns_sent: {}, inflight_txns: {}", txns_sent, stats.inflight_txns);
                    txns_sent = 0;
                    prev_committed = stats.num_committed;
                    report_time += Duration::from_secs_f64(report_interval);
                }
            }
        }

        // for (id, handle) in redirect_handles {
        //     if let Err(e) = handle.await {
        //         log::error!("join redirect thread for node {id} failed: {e:?}")
        //     }
        // }

        // if let Err(e) = mailbox.wait().await {
        //     log::error!("Mailbox failed with error {:?}", e);
        // }
    })
}
