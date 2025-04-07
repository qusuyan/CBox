use copycat::{ChainType, SignatureScheme};
use copycat_flowgen::get_flow_gen;
use tokio::time::{Duration, Instant};

#[tokio::main]
async fn main() {
    let num_nodes = 10;
    let exp_time = 60;
    let mut flowgen = get_flow_gen(
        0,
        vec![0],
        1000,
        None,
        1000000,
        0,
        0f64,
        ChainType::Aptos,
        SignatureScheme::DummyECDSA,
    );

    let mut blk_height = 0;
    let mut txns_generated = 0;

    let exp_end_time = Instant::now() + Duration::from_secs(exp_time);
    loop {
        tokio::select! {
            wait_next_req = flowgen.wait_next() => {
                if let Err(e) = wait_next_req {
                    println!("Error waiting for next transaction: {e:?}");
                    continue;
                }

                let next_req_batch = match flowgen.next_txn_batch().await {
                    Ok(txns) => txns,
                    Err(e) => {
                        println!("Error getting available request: {e:?}");
                        continue;
                    }
                };
                txns_generated += next_req_batch.len();
                blk_height += 1;

                for node_id in 0..num_nodes {
                    let ret_req_batch = next_req_batch.iter().map(|(_, txn)| txn.clone() ).collect::<Vec<_>>();
                    if let Err(e) = flowgen.txn_committed(node_id, ret_req_batch.clone(), blk_height).await {
                        println!("Error commiting txn batch: {e:?}");
                    }
                }

            }
            _ = tokio::time::sleep_until(exp_end_time) => {
                break;
            }
        }
    }

    println!("Experiment finished");
    println!("Generated {txns_generated} transactions in {exp_time} seconds");
    println!(
        "Average throughput: {} txns/sec",
        txns_generated as f64 / exp_time as f64
    );
    let stats = flowgen.get_stats();
    println!(
        "Stats: chain_length: {}, committed_count: {}, avg_latency: {}",
        stats.chain_length,
        stats.num_committed,
        stats.latencies.iter().sum::<f64>() / stats.latencies.len() as f64
    );
}
