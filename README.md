# CBox

CBox is a blockchain performance emulation framework. 
It aims to provide an easy way to understand the performance of a blockchain network at scale. 

CBox makes two major contributions. 
First, CBox proposes a generic SEDA model that can describe most existing blockchain systems. 
The use of SEDA model helps comprehend the performance characteristics of the blockchain being evaluated. 
Second, to enable large-scale performance evaluation, CBox utilizes various emulation techniques to reduce the hardware resource consumption of each validator, thereby allowing multiple validators to be packed on the same physical machine.
The validators communicate through the [*mailbox*](https://github.com/qusuyan/mailbox), a simple network emulation tool that injects WAN data transmission delays at the receiver side. 

We implemented simplified versions of four different blockchain systems: Bitcoin, Diem, Aptos, and Avalanche. 
For study purposes, we also include some variations of these systems. 

The paper will appear in [FC'26](https://fc26.ifca.ai/). 

## Future Directions

- [ ] Cross-machine emulation: since each validator requires much less network bandwidth than available in the data center servers, CBox can emulate larger-scale networks with multiple physical machines. This requires implementing efficient communication channels between physical machines (partly done).
- [ ] Domain-specific scheduler: CBox currently uses Tokio for scheduling, which introduces two major problems: 1. Time is not shared evenly among validators on the same physical machine, and 2. CBox needs to rely on semaphores to limit the computing power available to each validator, which can lead to deadlocks if not used carefully. A custom scheduler is required so that a task is scheduled only if its validator has idle cores and the validators are scheduled in a fair way. 
- [ ] Fine-grained emulation for storage layer: CBox focuses on performance and assumes that the blockchain protocol is correct. This means all correct nodes should stay in sync. That means, in most cases, all correct nodes should access the same state around the same time. With a storage layer shared among correct validators, we can not only save storage capacity but also save storage bandwidth by caching recently accessed states, so that only the first validator will experience disk access while CBox serves reads by subsequent validators from memory (with disk read delay injected). This requires adding a storage layer with a good caching policy.

Note: despite the importance of the storage layer in modern blockchain systems, studying the performance of a blockchain network does not require storage emulation. A simple workaround could be: 1. run the network with a given client trace and record all storage accesses validators experience; 2. run the storage accesses on a real storage layer implementation to record the execution time; 3. replay the same trace and inject the storage access delays measured in step 2 during emulation. 
