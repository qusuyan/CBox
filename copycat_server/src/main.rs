mod block;
mod composition;
mod node;
mod peers;
mod stage;

use block::ChainType;
use copycat_utils::log::colored_level;
use node::Node;

use std::io::Write;
use tokio::runtime::Builder;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(long, short = 'i')]
    id: u64,

    #[arg(long, short = 'c', value_enum, default_value = "Dummy")]
    chain: ChainType,

    #[arg(long, short = 't')]
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
        let node: Node<String> = match Node::init(id, args.chain).await {
            Ok(node) => node,
            Err(e) => {
                log::error!("Node {id}: failed to start node: {e:?}");
                return;
            }
        };
    })
}
