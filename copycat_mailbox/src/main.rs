use mailbox_utils;
use mailbox_utils::{config, MachineId};

use copycat_protocol::{ChainType, MsgType};

use tokio::runtime::Builder;

use std::io::Write;

use clap::Parser;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
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

    /// The type of blockchain under testing
    #[clap(long, short = 'c', default_value = "dummy")]
    chain: ChainType,
}

fn main() {
    let args = Args::parse();

    env_logger::builder()
        .format(move |buf, record| {
            let mut style = buf.style();
            let level = mailbox_utils::log::colored_level(&mut style, record.level());
            let mut style = buf.style();
            let target = style.set_bold(true).value(record.target());
            writeln!(buf, "{level} {target}: {}", record.args())
        })
        .init();

    let id: MachineId = args.id;
    let machine_config_path = args.machine_config;
    let machine_list = match config::read_machine_config(&machine_config_path) {
        Ok(machines) => machines,
        Err(e) => {
            log::error!("failed to read json config {machine_config_path}: {e}");
            return;
        }
    };

    let nodes = machine_list
        .iter()
        .flat_map(|(_, config::Machine { addr: _, node_list })| node_list.clone())
        .collect();
    let network_config_path = args.network_config;
    let network_config = match config::read_network_config(&network_config_path) {
        Ok(config) => config,
        Err(e) => {
            log::error!("failed to read network config {network_config_path}: {e}");
            return;
        }
    };
    let pipe_info = config::parse_network_config(&nodes, network_config);

    let num_threads = if args.num_threads > 0 {
        args.num_threads
    } else {
        log::warn!("invalid number of threads, using 8 threads instead");
        8
    };

    let chain_type = args.chain;

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(num_threads as usize)
        .thread_name("tokio-worker-replica")
        .build()
        .expect("Creating new runtime failed");

    runtime.block_on(async {
        if let Err(e) = match chain_type {
            ChainType::Dummy => mailbox::init::<MsgType>(id, machine_list, pipe_info).await,
            ChainType::Bitcoin => todo!(),
        } {
            log::error!("Ipc Server failed with error {:?}", e);
        }
    })
}
