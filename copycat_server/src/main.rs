mod composition;
mod node;
mod peers;
mod stage;

use copycat_protocol::ChainType;
use copycat_utils::log::colored_level;
use node::Node;

use std::io::Write;
use tokio::runtime::Builder;

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

    // let compose = get_chain_compose(args.chain);

    runtime.block_on(async {
        // TODO
        let (node, mut executed): (Node<String>, _) = match Node::init(id, args.chain).await {
            Ok(node) => node,
            Err(e) => {
                log::error!("Node {id}: failed to start node: {e:?}");
                return;
            }
        };

        let test_msg = (0..50000000).map(|_| id.to_string()).collect::<String>();

        if let Err(e) = node.send_req(test_msg).await {
            log::error!("Node {id}: failed to send txn request: {e:?}");
        }

        loop {
            match executed.recv().await {
                Some(txn) => {
                    log::info!("got committed txn {txn:?}")
                }
                None => {
                    log::error!("Node {id}: failed to recv executed txns");
                }
            }
        }
    })
}
