use super::{Factory, Library};
use crate::{
    constants::BPS_U256,
    contracts::bindings::i_uniswap_v2_router_02::IUniswapV2Router02,
    errors::{Error, Result},
    utils::{is_native_path, map_native},
    Amount,
};
use ethers_contract::builders::ContractCall;
use ethers_core::types::{Address, U256};
use ethers_providers::Middleware;
use std::sync::Arc;

#[cfg(feature = "addresses")]
use crate::protocol::ProtocolType;
#[cfg(feature = "addresses")]
use ethers_core::types::Chain;

contract_struct! {
    /// A UniswapV2 router.
    pub struct Router<M> {
        /// The router contract.
        contract: IUniswapV2Router02<M>,
    }
}

impl<M: Middleware> Router<M> {
    /// Creates a new instance using the provided address.
    pub fn new(client: Arc<M>, address: Address) -> Self {
        // assert!(protocol.is_v2(), "protocol must be v2");
        let contract = IUniswapV2Router02::new(address, client);
        Self { contract }
    }

    /// Creates a new instance by searching for the required addresses in the [addressbook].
    ///
    /// [addressbook]: crate::contracts::addresses
    #[cfg(feature = "addresses")]
    pub fn new_with_chain(client: Arc<M>, chain: Chain, protocol: ProtocolType) -> Option<Self> {
        // assert!(protocol.is_v2(), "protocol must be v2");
        protocol.try_addresses(chain).1.map(|address| Self::new(client, address))
    }

    /// Generalized add_liquidity function for the various [UniswapV2Router] methods.
    /// Returns the contract call with the necessary parameters set (value, calldata).
    ///
    /// See documentation of [Dex] for more details on arguments.
    ///
    /// Note: this function does not perform many sanity checks and it should be called by using the
    /// [Dex] struct.
    ///
    /// [UniswapV2Router]: https://github.com/Uniswap/v2-periphery/blob/master/contracts/UniswapV2Router01.sol
    /// [Dex]: crate::Dex
    pub fn add_liquidity(
        &self,
        token_a: Address,
        token_b: Address,
        amount_a_desired: U256,
        amount_b_desired: U256,
        amount_a_min: U256,
        amount_b_min: U256,
        to: Address,
        deadline: U256,
    ) -> Result<ContractCall<M, (U256, U256, U256)>> {
        let router = self.contract();
        let (native_a, native_b) = is_native_path(&[token_a, token_b]);

        let call = match (native_a, native_b) {
            (false, true) | (true, false) => {
                let (
                    token,
                    amount_token_min,
                    amount_token_desired,
                    amount_eth_desired,
                    amount_eth_min,
                ) = if native_a {
                    // token_a is ETH
                    (token_b, amount_b_min, amount_b_desired, amount_a_desired, amount_a_min)
                } else {
                    // token_b is ETH
                    (token_a, amount_a_min, amount_a_desired, amount_b_desired, amount_b_min)
                };
                router
                    .add_liquidity_eth(
                        token,
                        amount_token_desired,
                        amount_token_min,
                        amount_eth_min,
                        to,
                        deadline,
                    )
                    .value(amount_eth_desired)
            }
            (false, false) => router.add_liquidity(
                token_a,
                token_b,
                amount_a_desired,
                amount_b_desired,
                amount_a_min,
                amount_b_min,
                to,
                deadline,
            ),
            (true, true) => return Err(Error::IdenticalAddresses),
        };

        Ok(call)
    }

    /// Generalized remove_liquidity function for the various [UniswapV2Router] methods.
    /// Returns the contract call with the necessary parameters set (value, calldata).
    ///
    /// See documentation of [Dex] for more details on arguments.
    ///
    /// Note: this function does not perform many sanity checks and it should be called by using the
    /// [Dex] struct.
    ///
    /// [UniswapV2Router]: https://github.com/Uniswap/v2-periphery/blob/master/contracts/UniswapV2Router01.sol
    /// [Dex]: crate::Dex
    pub fn remove_liquidity(
        &self,
        token_a: Address,
        token_b: Address,
        liquidity: U256,
        amount_a_min: U256,
        amount_b_min: U256,
        to: Address,
        deadline: U256,
    ) -> Result<ContractCall<M, (U256, U256)>> {
        let router = self.contract();
        let (native_a, native_b) = is_native_path(&[token_a, token_b]);

        let call = match (native_a, native_b) {
            (false, true) | (true, false) => {
                let (token, amount_token_min, amount_eth_min) = if native_a {
                    // token_a is ETH
                    (token_b, amount_b_min, amount_a_min)
                } else {
                    // token_b is ETH
                    (token_a, amount_a_min, amount_b_min)
                };
                router.remove_liquidity_eth(
                    token,
                    liquidity,
                    amount_token_min,
                    amount_eth_min,
                    to,
                    deadline,
                )
            }
            (false, false) => router.remove_liquidity(
                token_a,
                token_b,
                liquidity,
                amount_a_min,
                amount_b_min,
                to,
                deadline,
            ),
            (true, true) => return Err(Error::IdenticalAddresses),
        };

        Ok(call)
    }

    /// Generalized swap function for the various [UniswapV2Router] `swap[Exact]XFor[Exact]Y`.
    /// Returns the contract call with the necessary parameters set (value, calldata).
    ///
    /// See documentation of [Dex] for more details on arguments.
    ///
    /// Note: this function does not perform many sanity checks and it should be called by using the
    /// [Dex] struct.
    ///
    /// [UniswapV2Router]: https://github.com/Uniswap/v2-periphery/blob/master/contracts/UniswapV2Router01.sol
    /// [Dex]: crate::Dex
    pub async fn swap(
        &self,
        factory: &Factory<M>,
        amount: Amount,
        slippage_tolerance: f32,
        path: &[Address],
        to: Address,
        deadline: U256,
        weth: Address,
    ) -> Result<ContractCall<M, Vec<U256>>> {
        let router = self.contract();
        let (from_native, to_native) = is_native_path(path);
        let mut path = path.to_vec();
        map_native(&mut path, weth);
        let call = match amount {
            Amount::ExactIn(amount_in) => {
                let amount_out_min = if slippage_tolerance == 100.0 {
                    U256::zero()
                } else {
                    let last_amount_out = *Library::get_amounts_out(factory, amount_in, &path)
                        .await?
                        .last()
                        .expect("path is empty");
                    if slippage_tolerance == 0.0 {
                        last_amount_out
                    } else {
                        let mult = 100.0 - slippage_tolerance;
                        let mult_bps = U256::from((mult * 100.0) as u32);
                        (last_amount_out * mult_bps) / BPS_U256
                    }
                };

                if from_native {
                    router
                        .swap_exact_eth_for_tokens_supporting_fee_on_transfer_tokens(amount_out_min, path, to, deadline)
                        .value(amount_in)
                } else if to_native {
                    router.swap_exact_tokens_for_eth(amount_in, amount_out_min, path, to, deadline)
                } else {
                    router.swap_exact_tokens_for_tokens_supporting_fee_on_transfer_tokens(
                        amount_in,
                        amount_out_min,
                        path,
                        to,
                        deadline,
                    )
                }
            }
            Amount::ExactOut(amount_out) => {
                let amount_in_max = if slippage_tolerance == 100.0 {
                    U256::MAX
                } else {
                    let first_amount_in = *Library::get_amounts_in(factory, amount_out, &path)
                        .await?
                        .first()
                        .expect("path is empty");
                    if slippage_tolerance == 0.0 {
                        first_amount_in
                    } else {
                        let mult = 1.0 / (100.0 - slippage_tolerance);
                        let mult_bps = U256::from((mult * 100.0).round() as u32);
                        (first_amount_in * mult_bps) / BPS_U256
                    }
                };

                if from_native {
                    router
                        .swap_eth_for_exact_tokens(amount_out, path, to, deadline)
                        .value(amount_in_max)
                } else if to_native {
                    router.swap_tokens_for_exact_eth(amount_out, amount_in_max, path, to, deadline)
                } else {
                    router.swap_tokens_for_exact_tokens(
                        amount_out,
                        amount_in_max,
                        path,
                        to,
                        deadline,
                    )
                }
            }
        };

        Ok(call)
    }
}
