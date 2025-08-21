#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum SwapDirection {
    /// Input token pc, output token coin
    PC2Coin = 1u64,
    /// Input token coin, output token pc
    Coin2PC = 2u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u64)]
pub enum SwapInType {
    /// swap-base-in
    BaseIn = 1u64,
    /// swap-base-out
    BaseOut = 2u64,
}

pub struct SwapConfig {
    pub(crate) slippage: u64,
    pub(crate) swap_direction: SwapDirection,
}
