mod stage;
mod composition;
mod node;

use node::Node;
use composition::SystemCompose;
use stage::txn_validation::TxnValidationType;
use tokio::runtime::Builder;

use copycat_utils::log::colored_level;
use std::io::Write;

use clap::Parser;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct CliArgs {
    #[arg(short, long)]
    id: u64,
}

pub fn main() {
    let args = CliArgs::parse();

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
    let num_threads: usize = 8;
    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(num_threads)
        .thread_name("tokio-worker-replica")
        .build()
        .expect("Creating new runtime failed");

    let compose = SystemCompose{ txn_validation_stage: TxnValidationType::DUMMY };

    runtime.block_on(async {
        // TODO
        let node: Node<String> = match Node::init(id, compose).await {
            Ok(node) => node,
            Err(e) => {
                log::error!("Node {id}: failed to start node: {e:?}");
                return;
            }
        };
    })
}
