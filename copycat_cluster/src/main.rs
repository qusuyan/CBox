use copycat::log::colored_level;
use copycat::{get_report_timer, get_timer_interval, parse_config_file, start_report_timer, Node, NodeId};
use copycat::{fully_connected_topology, get_topology};
use copycat::{ChainType, SignatureScheme, ThresholdSignatureScheme};
use copycat::protocol::MsgType;
use copycat_flowgen::get_flow_gen;

use mailbox::Mailbox;

use std::collections::{HashMap, HashSet};
use std::io::Write;

use tokio::runtime::{Builder, Handle};
use tokio::sync::mpsc;
use tokio::time::Duration;

use clap::Parser;
use sysinfo::{MemoryRefreshKind, System};

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

    /// Path to the config file that contains the configuration of all nodes
    #[arg(long)]
    config: String,

    /// Number of threads
    #[clap(long, short = 't', default_value_t = 8)]
    num_threads: u64,

    /// Number of mailbox workers
    #[clap(long, short = 'w', default_value_t = 8)]
    num_mailbox_workers: usize,

    /// Number of concurrent TCP streams between each pair of peers
    #[clap(long, short = 'o', default_value_t = 1)]
    num_conn_per_peer: usize,

    /// The type of blockchain
    #[arg(long, short = 'c', value_enum, default_value = "dummy")]
    chain: ChainType,

    /// Txn signature scheme
    #[arg(long, short = 'x', value_enum, default_value = "dummy")]
    txn_signature_scheme: SignatureScheme,

    /// p2p signature scheme
    #[arg(long, short = 'p', value_enum, default_value = "dummy")]
    p2p_signature_scheme: SignatureScheme,

    /// Threshold signature scheme
    #[arg(long, short = 'h', value_enum, default_value = "dummy")]
    threshold_signature_scheme: ThresholdSignatureScheme,

    /// Number of clients
    #[arg(long, short = 'l')]
    num_clients: Option<usize>,

    /// Number of user accounts for flow generation
    #[arg(long, short = 'a', default_value = "10000")]
    accounts: usize,

    /// Number of user accounts for flow generation
    #[arg(long, short = 'z')]
    script_size: Option<usize>,

    /// Maximum number of inflight transaction requests, 0 means unlimited
    #[arg(long, short = 'f', default_value = "100000")]
    max_inflight: usize,

    /// Frequency at which transactions are generated, 0 means unlimited
    #[arg(long, short = 'q', value_enum, default_value = "0")]
    frequency: usize,

    /// Probability that a conflict transaction will be generated
    #[arg(long, short = 'r', default_value = "0")]
    conflict_rate: f64,

    /// Number of nodes receiving a transaction, 0 means sending to all nodes
    #[arg(long, short = 's', default_value_t = 1)]
    txn_span: usize,

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

        // if matches!(self.chain, ChainType::Avalanche)
        //     && !matches!(self.dissem_pattern, DissemPattern::Sample)
        // {
        //     log::warn!("Avalanche is based on sampling, using dissem pattern sample instead");
        //     self.dissem_pattern = DissemPattern::Sample;
        // }

        // if !matches!(self.dissem_pattern, DissemPattern::Gossip) && !self.topology.is_empty() {
        //     log::warn!("Topology files are only used for gossipping, but the dissem pattern is {:?}, ignoring topology file...", self.dissem_pattern);
        //     self.topology = String::from("");
        // }
    }
}

fn main() {
    let mut args = CliArgs::parse();
    let threshold_signature_scheme = ThresholdSignatureScheme::Dummy;
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

    let txn_crypto = args.txn_signature_scheme;

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

    let nodes: HashSet<NodeId> = machine_list
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
    let (nic_egress_info, pipe_info, nic_ingress_info) = mailbox::config::parse_network_config(&nodes, network_config);

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

    let mut node_config_map = parse_config_file(&args.config, args.chain).unwrap();
    for id in local_nodes.iter() {
        let node_config = node_config_map.get_mut(id).unwrap();
        node_config.validate(topology.get(id).unwrap(), &nodes);
    };
    log::info!("Node configs: {:?}", node_config_map);

    let majority = nodes.len() / 3 * 2 + 1;
    let mut threshold_signature_quorum = threshold_signature_scheme.to_threshold_signature(
        &nodes,
        majority.try_into().unwrap(),
        0,
    ).expect("failed to generate threshold signature");

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

    // collect system information
    let mut sys = System::new();
    sys.refresh_cpu_usage();
    sys.refresh_memory_specifics(MemoryRefreshKind::new().with_ram());

    let mut stats_file = std::fs::File::create(format!("/tmp/copycat_cluster_{}.csv", id))
        .expect("stats file creation failed");
    stats_file
        .write(b"Runtime (s),Throughput (txn/s),Chain Length,Commit Confidence,Avg CPU Usage,Available Memory\n")
        .expect("write stats failed");
    let mut latency_file = std::fs::File::create(format!("/tmp/copycat_cluster_{}_lat.csv", id))
        .expect("latency file creation failed");
    latency_file
        .write(b"Latency (s)\n")
        .expect("write latency failed");
    // console_subscriber::init();

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .disable_lifo_slot()
        .worker_threads(args.num_threads as usize)
        .thread_name("copycat-cluster-thread")
        .build()
        .expect("Creating new runtime failed");

    runtime.block_on(async {
        // start mailbox
        let _mailbox = match Mailbox::init::<MsgType>(id, machine_list, nic_egress_info, pipe_info, nic_ingress_info, args.num_mailbox_workers, args.num_conn_per_peer).await {
            Ok(mailbox) => mailbox,
            Err(e) => {
                log::error!("Mailbox initialization failed with error {:?}", e);
                std::process::exit(-1);
            }
        };

        // start nodes
        let mut node_map = HashMap::new();
        let (executed_send, mut executed_recv) = mpsc::channel(0x1000000);
        // let mut node_rt = HashMap::new();
        for node_id in local_nodes.clone() {
            // let rt = Builder::new_multi_thread()
            //     .enable_all()
            //     .worker_threads(2)
            //     .thread_name(format!("copycat-node-{node_id}-thread"))
            //     .build()
            //     .expect("Creating new runtime failed");
            let node_config = node_config_map.remove(&node_id).unwrap();
            let p2p_crypto = args.p2p_signature_scheme.gen_p2p_signature(node_id, nodes.iter());
            let threshold_signature = threshold_signature_quorum.remove(&node_id).unwrap();
            let node = match Node::init(
                node_id,
                txn_crypto,
                p2p_crypto,
                threshold_signature,
                node_config.chain_config,
                !args.disable_txn_dissem,
                args.num_mailbox_workers,
                topology.remove(&node_id).unwrap_or(HashSet::new()),
                node_config.max_concurrency,
                executed_send.clone(),
            ).await {
                Ok(node) => node,
                Err(e) => {
                    log::error!("failed to start node: {e:?}");
                    return;
                }
            };
            node_map.insert(node_id, node);
        }

        // run flowgen
        let mut flow_gen = get_flow_gen(
            id, 
            client_list,
            args.accounts,
            args.script_size,
            args.max_inflight,
            args.frequency,
            args.conflict_rate,
            args.chain,
            args.txn_signature_scheme,
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
        let mut report_timer = get_report_timer();
        let report_interval = get_timer_interval().as_secs_f64();
        start_report_timer().await;
        // wait when setup txns are propogated over the network
        let warn_up_time = std::env::var("WARMUP_TIME").map(|str| str.parse::<u64>().expect("invalid warmup time")).unwrap_or(0);
        tokio::time::sleep(Duration::from_secs(warn_up_time)).await;
        log::info!("flow generation starts");
        let mut txns_sent = 0;
        let mut prev_committed = 0;

        let rt_handle = Handle::current();

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

                committed_txn = executed_recv.recv() => {
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

                report_val = report_timer.changed() => {
                    if let Err(e) = report_val {
                        log::error!("Waiting for report timeout failed: {}", e);
                    }
                    let stats = flow_gen.get_stats();
                    let run_time = report_timer.borrow().as_secs_f64().round() as usize;
                    let newly_committed = stats.num_committed - prev_committed;
                    let tput = newly_committed as f64 / report_interval;

                    sys.refresh_cpu_usage();
                    sys.refresh_memory_specifics(MemoryRefreshKind::new().with_ram());
                    let (cpu_usage, cpu_count) = sys.cpus()
                        .into_iter()
                        .map(|cpu| (cpu.cpu_usage(), 1))
                        .reduce(|(usage1, count1), (usage2, count2)| (usage1 + usage2, count1 + count2))
                        .expect("no CPU returned");
                    let avg_cpu_usage = cpu_usage / cpu_count as f32;
                    let mem_available = sys.available_memory();

                    let rt_metrics = rt_handle.metrics();
                    let active_tasks = rt_metrics.active_tasks_count();
                    let avg_queue_depth = (0..rt_metrics.num_workers()).map(|id| rt_metrics.worker_local_queue_depth(id)).sum::<usize>() / rt_metrics.num_workers();
                    let avg_poll_count = (0..rt_metrics.num_workers()).map(|id| rt_metrics.worker_poll_count(id)).sum::<u64>() / rt_metrics.num_workers() as u64;
                    let avg_overflow_count = (0..rt_metrics.num_workers()).map(|id| rt_metrics.worker_overflow_count(id)).sum::<u64>() / rt_metrics.num_workers() as u64;
                    let avg_steal_count = (0..rt_metrics.num_workers()).map(|id| rt_metrics.worker_steal_count(id)).sum::<u64>() / rt_metrics.num_workers() as u64;

                    log::info!(
                        "Runtime: {} s, Throughput: {} txn/s, Chain Length: {}, Commit Confidence: {}, CPU Usage: {}, Memory Available: {}",
                        run_time,
                        tput,
                        stats.chain_length,
                        stats.commit_confidence,
                        avg_cpu_usage,
                        mem_available,
                    );
                    stats_file
                        .write_fmt(format_args!("{},{},{},{},{},{}\n", run_time, tput, stats.chain_length, stats.commit_confidence, avg_cpu_usage, mem_available))
                        .expect("write stats failed");
                    for lat in stats.latencies {
                        latency_file.write_fmt(format_args!("{}\n", lat))
                        .expect("write latency failed");
                    }
                    log::info!("In the last minute: txns_sent: {}, inflight_txns: {}", txns_sent, stats.inflight_txns);
                    log::info!("Cumulatively: active tasks: {} avg queue depth: {}, avg poll count: {}, avg overflow count: {}, avg steal count: {}", active_tasks, avg_queue_depth, avg_poll_count, avg_overflow_count, avg_steal_count);
                    txns_sent = 0;
                    prev_committed = stats.num_committed;

                    for node in node_map.values() {
                        node.report();
                    }
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
