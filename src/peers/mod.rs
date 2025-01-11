#[cfg(not(test))]
mod peers;
#[cfg(not(test))]
pub use peers::PeerMessenger;

#[cfg(test)]
mod mock;
#[cfg(test)]
pub use mock::PeerMessenger;
